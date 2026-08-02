//! Turns a [`ModelSpec`] plus its weights into a ggml graph.
//!
//! Tensor layout follows ggml's convention where `ne[0]` varies fastest:
//! activations are `[W, H, C, N]` (planar CHW) and convolution weights are
//! `[KW, KH, IC, OC]`, which is byte-identical to a row-major OIHW array.
//! Depthwise weights are `[KW, KH, 1, C]`, i.e. row-major `[C, 1, KH, KW]`.
//! `tools/convert_movenet.py` transposes TensorFlow's HWIO into that order.

use makepad_ggml::{BufferUsage, Context, Op, ScaleMode, TensorId, TensorType, UnaryOp};
use std::collections::HashMap;

use super::spec::{LayerSpec, ModelSpec};
use super::weights::Weights;
use super::MoveNetError;

/// The model input plus the four head outputs, as ids into the context.
#[derive(Debug)]
pub struct BuiltGraph {
    pub input: TensorId,
    pub heatmap: TensorId,
    pub center: TensorId,
    pub regress: TensorId,
    pub offset: TensorId,
}

pub fn build(
    ctx: &mut Context,
    spec: &ModelSpec,
    weights: &Weights,
) -> Result<BuiltGraph, MoveNetError> {
    let input = ctx
        .new_tensor_4d(
            TensorType::F32,
            spec.input_width,
            spec.input_height,
            3,
            1,
            BufferUsage::Activations,
        )
        .map_err(MoveNetError::Graph)?;
    ctx.set_tensor_name(input, "image").ok();

    let mut values: HashMap<String, TensorId> = HashMap::new();
    values.insert("image".to_string(), input);

    for (index, layer) in spec.layers.iter().enumerate() {
        let id = build_layer(ctx, layer, weights, &values)
            .map_err(|e| e.with_layer(index, &layer.op, &layer.output))?;
        values.insert(layer.output.clone(), id);
    }

    let lookup = |name: &str| -> Result<TensorId, MoveNetError> {
        values
            .get(name)
            .copied()
            .ok_or_else(|| MoveNetError::Graph(format!("output tensor {name} was never produced")))
    };

    Ok(BuiltGraph {
        input,
        heatmap: lookup(&spec.outputs.heatmap)?,
        center: lookup(&spec.outputs.center)?,
        regress: lookup(&spec.outputs.regress)?,
        offset: lookup(&spec.outputs.offset)?,
    })
}

fn build_layer(
    ctx: &mut Context,
    layer: &LayerSpec,
    weights: &Weights,
    values: &HashMap<String, TensorId>,
) -> Result<TensorId, MoveNetError> {
    let input = |n: usize| -> Result<TensorId, MoveNetError> {
        let name = layer.inputs.get(n).ok_or_else(|| {
            MoveNetError::Graph(format!("expects at least {} input(s)", n + 1))
        })?;
        values
            .get(name)
            .copied()
            .ok_or_else(|| MoveNetError::Graph(format!("input tensor {name} is not defined yet")))
    };

    let out = match layer.op.as_str() {
        "conv2d" | "dwconv2d" => {
            let depthwise = layer.op == "dwconv2d";
            let mut src = input(0)?;
            let kernel = load_kernel(ctx, layer, weights, depthwise)?;
            let (s0, s1) = layer.stride_xy();
            let (d0, d1) = layer.dilation_xy();

            // ggml's conv pads evenly; TF's SAME does not. When the two
            // disagree, pad explicitly first and convolve with no padding,
            // which keeps both the output size and the pixel grid exact.
            let [pl, pr, pt, pb] = layer.pad_sides();
            let (p0, p1) = if layer.pad_is_symmetric() {
                (pl, pt)
            } else {
                src = ctx
                    .pad_ext(
                        src,
                        pl as i64,
                        pr as i64,
                        pt as i64,
                        pb as i64,
                        0,
                        0,
                        0,
                        0,
                        BufferUsage::Activations,
                    )
                    .map_err(MoveNetError::Graph)?;
                (0, 0)
            };
            let conv = if depthwise {
                ctx.conv_2d_dw(kernel, src, s0, s1, p0, p1, d0, d1, BufferUsage::Activations)
            } else {
                ctx.conv_2d(kernel, src, s0, s1, p0, p1, d0, d1, BufferUsage::Activations)
            }
            .map_err(MoveNetError::Graph)?;

            match layer.bias.as_deref() {
                Some(bias_name) => add_channel_bias(ctx, conv, bias_name, weights)?,
                None => conv,
            }
        }
        "add" => binary(ctx, Op::Add, input(0)?, input(1)?)?,
        "sub" => binary(ctx, Op::Sub, input(0)?, input(1)?)?,
        "mul" => binary(ctx, Op::Mul, input(0)?, input(1)?)?,
        "div" => binary(ctx, Op::Div, input(0)?, input(1)?)?,
        // a literal from the weights file, e.g. the per-channel mean the
        // model subtracts from the input
        "constant" => load_constant(ctx, layer, weights)?,
        "concat" => {
            // default to the channel dimension, which is dim 2 in ggml order
            let dim = layer.dim.unwrap_or(2);
            let mut acc = input(0)?;
            for n in 1..layer.inputs.len() {
                acc = ctx
                    .concat(acc, input(n)?, dim, BufferUsage::Activations)
                    .map_err(MoveNetError::Graph)?;
            }
            acc
        }
        "upsample" => {
            let factor = layer.factor.unwrap_or(2);
            let mode = match layer.mode.as_deref() {
                None | Some("nearest") => ScaleMode::Nearest,
                Some("bilinear") => ScaleMode::Bilinear,
                Some("bicubic") => ScaleMode::Bicubic,
                Some(other) => {
                    return Err(MoveNetError::Graph(format!("unknown upsample mode {other}")))
                }
            };
            ctx.upscale(
                input(0)?,
                factor,
                mode,
                false,
                false,
                BufferUsage::Activations,
            )
            .map_err(MoveNetError::Graph)?
        }
        "activation" => input(0)?,
        other => return Err(MoveNetError::Graph(format!("unknown op {other}"))),
    };

    apply_activation(ctx, out, layer.activation.as_deref())
}

/// Elementwise op that broadcasts `b` up to `a`'s shape when it is smaller —
/// the model's normalization constants are scalars or one value per channel.
fn binary(
    ctx: &mut Context,
    op: Op,
    a: TensorId,
    b: TensorId,
) -> Result<TensorId, MoveNetError> {
    let a_ne = ctx
        .tensor(a)
        .ok_or_else(|| MoveNetError::Graph("missing left operand".to_string()))?
        .ne;
    let b_ne = ctx
        .tensor(b)
        .ok_or_else(|| MoveNetError::Graph("missing right operand".to_string()))?
        .ne;

    let b = if a_ne == b_ne {
        b
    } else {
        if b_ne.iter().zip(a_ne.iter()).any(|(b, a)| *b != 1 && b != a) {
            return Err(MoveNetError::Graph(format!(
                "cannot broadcast {b_ne:?} onto {a_ne:?}"
            )));
        }
        ctx.repeat(b, a, BufferUsage::Activations)
            .map_err(MoveNetError::Graph)?
    };

    ctx.binary_like_a(op, a, b, BufferUsage::Activations)
        .map_err(MoveNetError::Graph)
}

/// Materialize a constant from the weights file. Shapes arrive in the
/// converter's NHWC order and are mapped onto ggml's `[W, H, C, N]`.
fn load_constant(
    ctx: &mut Context,
    layer: &LayerSpec,
    weights: &Weights,
) -> Result<TensorId, MoveNetError> {
    let name = layer
        .weight
        .as_deref()
        .ok_or_else(|| MoveNetError::Graph("constant has no weight name".to_string()))?;
    let tensor = weights.get(name)?;

    let ne = match tensor.shape.len() {
        0 => [1, 1, 1, 1],
        1 => [1, 1, tensor.shape[0] as i64, 1],
        4 => [
            tensor.shape[2] as i64,
            tensor.shape[1] as i64,
            tensor.shape[3] as i64,
            tensor.shape[0] as i64,
        ],
        other => {
            return Err(MoveNetError::Graph(format!(
                "constant {name} has {other} dimensions, expected 0, 1 or 4"
            )))
        }
    };

    let id = ctx
        .new_tensor_4d(TensorType::F32, ne[0], ne[1], ne[2], ne[3], BufferUsage::Weights)
        .map_err(MoveNetError::Graph)?;
    ctx.set_tensor_name(id, name).ok();
    write_f32(ctx, id, &tensor.data)?;
    Ok(id)
}

fn apply_activation(
    ctx: &mut Context,
    src: TensorId,
    activation: Option<&str>,
) -> Result<TensorId, MoveNetError> {
    let out = match activation {
        None | Some("") | Some("none") | Some("linear") => src,
        // relu6 is a clamp, which avoids a second pass for the upper bound
        Some("relu6") => ctx
            .clamp(src, 0.0, 6.0, BufferUsage::Activations)
            .map_err(MoveNetError::Graph)?,
        Some("relu") => ctx
            .unary(src, UnaryOp::Relu, BufferUsage::Activations)
            .map_err(MoveNetError::Graph)?,
        Some("sigmoid") => ctx
            .unary(src, UnaryOp::Sigmoid, BufferUsage::Activations)
            .map_err(MoveNetError::Graph)?,
        Some("hardswish") => ctx
            .unary(src, UnaryOp::Hardswish, BufferUsage::Activations)
            .map_err(MoveNetError::Graph)?,
        Some("tanh") => ctx
            .unary(src, UnaryOp::Tanh, BufferUsage::Activations)
            .map_err(MoveNetError::Graph)?,
        Some(other) => {
            return Err(MoveNetError::Graph(format!(
                "unknown activation {other}"
            )))
        }
    };
    Ok(out)
}

/// Upload a convolution kernel, checking the converter produced the layout
/// ggml expects before the bytes land in a tensor that claims that shape.
fn load_kernel(
    ctx: &mut Context,
    layer: &LayerSpec,
    weights: &Weights,
    depthwise: bool,
) -> Result<TensorId, MoveNetError> {
    let name = layer
        .weight
        .as_deref()
        .ok_or_else(|| MoveNetError::Graph("convolution has no weight name".to_string()))?;
    let tensor = weights.get(name)?;
    if tensor.shape.len() != 4 {
        return Err(MoveNetError::Graph(format!(
            "{name} must be 4d [OC, IC, KH, KW], got {:?}",
            tensor.shape
        )));
    }
    let (oc, ic, kh, kw) = (
        tensor.shape[0] as i64,
        tensor.shape[1] as i64,
        tensor.shape[2] as i64,
        tensor.shape[3] as i64,
    );
    if depthwise && ic != 1 {
        return Err(MoveNetError::Graph(format!(
            "depthwise {name} must have shape [C, 1, KH, KW], got {:?}",
            tensor.shape
        )));
    }

    // row-major [OC, IC, KH, KW] is the same bytes as ggml [KW, KH, IC, OC]
    let (ne2, ne3) = if depthwise { (1, oc) } else { (ic, oc) };
    let id = ctx
        .new_tensor_4d(TensorType::F32, kw, kh, ne2, ne3, BufferUsage::Weights)
        .map_err(MoveNetError::Graph)?;
    ctx.set_tensor_name(id, name).ok();
    write_f32(ctx, id, &tensor.data)?;
    Ok(id)
}

fn add_channel_bias(
    ctx: &mut Context,
    conv: TensorId,
    bias_name: &str,
    weights: &Weights,
) -> Result<TensorId, MoveNetError> {
    let bias = weights.get(bias_name)?;
    let channels = ctx
        .tensor(conv)
        .ok_or_else(|| MoveNetError::Graph("convolution output vanished".to_string()))?
        .ne[2];
    if bias.elements() as i64 != channels {
        return Err(MoveNetError::Graph(format!(
            "{bias_name} holds {} values but the convolution produces {} channels",
            bias.elements(),
            channels
        )));
    }

    let vector = ctx
        .new_tensor_4d(TensorType::F32, 1, 1, channels, 1, BufferUsage::Weights)
        .map_err(MoveNetError::Graph)?;
    ctx.set_tensor_name(vector, bias_name).ok();
    write_f32(ctx, vector, &bias.data)?;

    let broadcast = ctx
        .repeat(vector, conv, BufferUsage::Activations)
        .map_err(MoveNetError::Graph)?;
    ctx.binary_like_a(Op::Add, conv, broadcast, BufferUsage::Activations)
        .map_err(MoveNetError::Graph)
}

fn write_f32(ctx: &mut Context, id: TensorId, values: &[f32]) -> Result<(), MoveNetError> {
    // the sizing pass builds the same graph with no backing arena; there is
    // nothing to write into yet and the shapes are all that matter
    if ctx.get_no_alloc() {
        return Ok(());
    }
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for v in values {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    ctx.write_tensor_data(id, &bytes)
        .map_err(MoveNetError::Graph)
}
