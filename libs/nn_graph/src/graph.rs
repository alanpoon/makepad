//! Turns a [`ModelSpec`] plus its weights into a ggml graph.
//!
//! Tensor layout follows ggml's convention where `ne[0]` varies fastest:
//! activations are `[W, H, C, N]` (planar CHW) and convolution weights are
//! `[KW, KH, IC, OC]`, which is byte-identical to a row-major OIHW array.
//! Depthwise weights are `[KW, KH, 1, C]`, i.e. row-major `[C, 1, KH, KW]`.
//! `tools/convert_movenet.py` transposes TensorFlow's HWIO into that order.

use makepad_ggml::{BufferUsage, Context, Op, PoolOp, ScaleMode, TensorId, TensorType, UnaryOp};
use std::collections::HashMap;

use crate::spec::{LayerSpec, ModelSpec};
use crate::weights::Weights;
use crate::NnError;

/// The model input plus every tensor the spec named as an output.
#[derive(Debug)]
pub struct BuiltGraph {
    pub input: TensorId,
    /// Role name -> tensor id, mirroring `ModelSpec::outputs`.
    pub outputs: HashMap<String, TensorId>,
}

impl BuiltGraph {
    pub fn output(&self, role: &str) -> Result<TensorId, NnError> {
        self.outputs
            .get(role)
            .copied()
            .ok_or_else(|| NnError::Spec(format!("model has no output named {role}")))
    }
}

pub fn build(
    ctx: &mut Context,
    spec: &ModelSpec,
    weights: &Weights,
) -> Result<BuiltGraph, NnError> {
    let input = ctx
        .new_tensor_4d(
            TensorType::F32,
            spec.input_width,
            spec.input_height,
            3,
            1,
            BufferUsage::Activations,
        )
        .map_err(NnError::Graph)?;
    ctx.set_tensor_name(input, "image").ok();

    let mut values: HashMap<String, TensorId> = HashMap::new();
    values.insert("image".to_string(), input);

    for (index, layer) in spec.layers.iter().enumerate() {
        let id = build_layer(ctx, layer, weights, &values)
            .map_err(|e| e.with_layer(index, &layer.op, &layer.output))?;
        values.insert(layer.output.clone(), id);
    }

    let mut outputs = HashMap::new();
    for (role, name) in &spec.outputs {
        let id = values.get(name).copied().ok_or_else(|| {
            NnError::Graph(format!("output {role} names tensor {name}, which was never produced"))
        })?;
        outputs.insert(role.clone(), id);
    }

    Ok(BuiltGraph { input, outputs })
}

fn build_layer(
    ctx: &mut Context,
    layer: &LayerSpec,
    weights: &Weights,
    values: &HashMap<String, TensorId>,
) -> Result<TensorId, NnError> {
    let input = |n: usize| -> Result<TensorId, NnError> {
        let name = layer.inputs.get(n).ok_or_else(|| {
            NnError::Graph(format!("expects at least {} input(s)", n + 1))
        })?;
        values
            .get(name)
            .copied()
            .ok_or_else(|| NnError::Graph(format!("input tensor {name} is not defined yet")))
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
                    .map_err(NnError::Graph)?;
                (0, 0)
            };
            let conv = if depthwise {
                ctx.conv_2d_dw(kernel, src, s0, s1, p0, p1, d0, d1, BufferUsage::Activations)
            } else {
                ctx.conv_2d(kernel, src, s0, s1, p0, p1, d0, d1, BufferUsage::Activations)
            }
            .map_err(NnError::Graph)?;

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
                    .map_err(NnError::Graph)?;
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
                    return Err(NnError::Graph(format!("unknown upsample mode {other}")))
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
            .map_err(NnError::Graph)?
        }
        "activation" => input(0)?,
        // PReLU: relu(x) - slope * relu(-x), which needs no dedicated op
        "prelu" => {
            let x = input(0)?;
            let slope = load_named_constant(ctx, layer.slope.as_deref(), weights)?;
            let pos = ctx
                .unary(x, UnaryOp::Relu, BufferUsage::Activations)
                .map_err(NnError::Graph)?;
            let neg_x = ctx
                .scale(x, -1.0, BufferUsage::Activations)
                .map_err(NnError::Graph)?;
            let neg = ctx
                .unary(neg_x, UnaryOp::Relu, BufferUsage::Activations)
                .map_err(NnError::Graph)?;
            let scaled = binary(ctx, Op::Mul, neg, slope)?;
            binary(ctx, Op::Sub, pos, scaled)?
        }
        "maxpool2d" | "avgpool2d" => {
            let (kw, kh) = pair(&layer.window, 2);
            let (s0, s1) = layer.stride_xy();
            // the pool kernel applies padding itself, but only symmetrically;
            // materialize a padded copy when the two sides differ
            let [pl, pr, pt, pb] = layer.pad_sides();
            let (mut src, p0, p1) = if layer.pad_is_symmetric() {
                (input(0)?, pl, pt)
            } else {
                (input(0)?, 0, 0)
            };
            if !layer.pad_is_symmetric() {
                src = ctx
                    .pad_ext(
                        src, pl as i64, pr as i64, pt as i64, pb as i64, 0, 0, 0, 0,
                        BufferUsage::Activations,
                    )
                    .map_err(NnError::Graph)?;
            }
            let op = if layer.op == "maxpool2d" {
                PoolOp::Max
            } else {
                PoolOp::Avg
            };
            ctx.pool_2d(src, op, kw, kh, s0, s1, p0, p1, BufferUsage::Activations)
                .map_err(NnError::Graph)?
        }
        "pad" => {
            let [pl, pr, pt, pb] = layer.pad_sides();
            let (cb, ca) = pair(&layer.pad_channels, 0);
            ctx.pad_ext(
                input(0)?, pl as i64, pr as i64, pt as i64, pb as i64, cb as i64, ca as i64,
                0, 0, BufferUsage::Activations,
            )
            .map_err(NnError::Graph)?
        }
        // TFLite reshapes flatten NHWC, where the channel varies fastest.
        // Activations here are planar [W, H, C, N], so a spatial tensor has to
        // be permuted to channel-fastest before the shape can be reinterpreted
        // -- otherwise the values land in the wrong order.
        "reshape" => {
            let shape = layer
                .shape
                .as_ref()
                .ok_or_else(|| NnError::Graph("reshape has no shape".to_string()))?;
            let ne = nhwc_to_ggml(shape);
            let src = input(0)?;
            let src_ne = ctx
                .tensor(src)
                .ok_or_else(|| NnError::Graph("reshape input vanished".to_string()))?
                .ne;

            let spatial = src_ne[2] > 1 && (src_ne[0] > 1 || src_ne[1] > 1);
            let ordered = if spatial {
                // [W, H, C, N] -> [C, W, H, N], which is NHWC in memory
                ctx.permute(src, [1, 2, 0, 3]).map_err(NnError::Graph)?
            } else {
                src
            };
            let flat = ctx.cont(ordered).map_err(NnError::Graph)?;
            ctx.reshape(flat, &ne).map_err(NnError::Graph)?
        }
        // TFLite's fully-connected: out = in * W^T + b, with W as [OUT, IN]
        "fully_connected" => {
            let src = input(0)?;
            let name = layer
                .weight
                .as_deref()
                .ok_or_else(|| NnError::Graph("fully_connected has no weight".to_string()))?;
            let w = weights.get(name)?;
            if w.shape.len() != 2 {
                return Err(NnError::Graph(format!(
                    "{name} must be 2d [OUT, IN], got {:?}",
                    w.shape
                )));
            }
            let (out_dim, in_dim) = (w.shape[0] as i64, w.shape[1] as i64);
            let weight = ctx
                .new_tensor_4d(TensorType::F32, in_dim, out_dim, 1, 1, BufferUsage::Weights)
                .map_err(NnError::Graph)?;
            ctx.set_tensor_name(weight, name).ok();
            write_f32(ctx, weight, &w.data)?;

            let flat = ctx
                .cont(src)
                .map_err(NnError::Graph)?;
            let flat = ctx.reshape(flat, &[in_dim, 1, 1, 1]).map_err(NnError::Graph)?;
            let out = ctx
                .mul_mat(weight, flat, BufferUsage::Activations)
                .map_err(NnError::Graph)?;
            match layer.bias.as_deref() {
                Some(bias_name) => {
                    let b = weights.get(bias_name)?;
                    let bias = ctx
                        .new_tensor_4d(TensorType::F32, out_dim, 1, 1, 1, BufferUsage::Weights)
                        .map_err(NnError::Graph)?;
                    ctx.set_tensor_name(bias, bias_name).ok();
                    write_f32(ctx, bias, &b.data)?;
                    binary(ctx, Op::Add, out, bias)?
                }
                None => out,
            }
        }
        // global average over the spatial dims, as MEAN over H and W
        "mean" => {
            let src = input(0)?;
            let ne = ctx
                .tensor(src)
                .ok_or_else(|| NnError::Graph("mean input vanished".to_string()))?
                .ne;
            let (w, h, c) = (ne[0], ne[1], ne[2]);
            // [W, H, C, N] -> [W*H, C, N] so sum_rows collapses the plane
            let flat = ctx
                .cont(src)
                .map_err(NnError::Graph)?;
            let flat = ctx
                .reshape(flat, &[w * h, c, ne[3], 1])
                .map_err(NnError::Graph)?;
            let sum = ctx
                .sum_rows(flat, BufferUsage::Activations)
                .map_err(NnError::Graph)?;
            let mean = ctx
                .scale(sum, 1.0 / (w * h) as f32, BufferUsage::Activations)
                .map_err(NnError::Graph)?;
            ctx.reshape(mean, &[1, 1, c, ne[3]]).map_err(NnError::Graph)?
        }
        other => return Err(NnError::Graph(format!("unknown op {other}"))),
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
) -> Result<TensorId, NnError> {
    let a_ne = ctx
        .tensor(a)
        .ok_or_else(|| NnError::Graph("missing left operand".to_string()))?
        .ne;
    let b_ne = ctx
        .tensor(b)
        .ok_or_else(|| NnError::Graph("missing right operand".to_string()))?
        .ne;

    let b = if a_ne == b_ne {
        b
    } else {
        if b_ne.iter().zip(a_ne.iter()).any(|(b, a)| *b != 1 && b != a) {
            return Err(NnError::Graph(format!(
                "cannot broadcast {b_ne:?} onto {a_ne:?}"
            )));
        }
        ctx.repeat(b, a, BufferUsage::Activations)
            .map_err(NnError::Graph)?
    };

    ctx.binary_like_a(op, a, b, BufferUsage::Activations)
        .map_err(NnError::Graph)
}

/// NHWC shape from the converter -> ggml `[W, H, C, N]`.
fn nhwc_to_ggml(shape: &[i64]) -> Vec<i64> {
    match shape.len() {
        1 => vec![shape[0], 1, 1, 1],
        2 => vec![shape[1], 1, 1, shape[0]],
        3 => vec![shape[2], shape[1], 1, shape[0]],
        4 => vec![shape[3], shape[2], shape[1], shape[0]],
        _ => vec![shape.iter().product(), 1, 1, 1],
    }
}

fn pair(value: &Option<Vec<i32>>, default: i32) -> (i32, i32) {
    match value {
        Some(v) if v.len() == 2 => (v[0], v[1]),
        Some(v) if v.len() == 1 => (v[0], v[0]),
        _ => (default, default),
    }
}

/// Upload a 1d constant (a PReLU slope) as `[1, 1, C, 1]` for broadcasting.
fn load_named_constant(
    ctx: &mut Context,
    name: Option<&str>,
    weights: &Weights,
) -> Result<TensorId, NnError> {
    let name = name.ok_or_else(|| NnError::Graph("missing constant name".to_string()))?;
    let t = weights.get(name)?;
    let channels = *t.shape.last().unwrap_or(&t.data.len()) as i64;
    let id = ctx
        .new_tensor_4d(TensorType::F32, 1, 1, channels, 1, BufferUsage::Weights)
        .map_err(NnError::Graph)?;
    ctx.set_tensor_name(id, name).ok();
    write_f32(ctx, id, &t.data)?;
    Ok(id)
}

/// Materialize a constant from the weights file. Shapes arrive in the
/// converter's NHWC order and are mapped onto ggml's `[W, H, C, N]`.
fn load_constant(
    ctx: &mut Context,
    layer: &LayerSpec,
    weights: &Weights,
) -> Result<TensorId, NnError> {
    let name = layer
        .weight
        .as_deref()
        .ok_or_else(|| NnError::Graph("constant has no weight name".to_string()))?;
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
            return Err(NnError::Graph(format!(
                "constant {name} has {other} dimensions, expected 0, 1 or 4"
            )))
        }
    };

    let id = ctx
        .new_tensor_4d(TensorType::F32, ne[0], ne[1], ne[2], ne[3], BufferUsage::Weights)
        .map_err(NnError::Graph)?;
    ctx.set_tensor_name(id, name).ok();
    write_f32(ctx, id, &tensor.data)?;
    Ok(id)
}

fn apply_activation(
    ctx: &mut Context,
    src: TensorId,
    activation: Option<&str>,
) -> Result<TensorId, NnError> {
    let out = match activation {
        None | Some("") | Some("none") | Some("linear") => src,
        // relu6 is a clamp, which avoids a second pass for the upper bound
        Some("relu6") => ctx
            .clamp(src, 0.0, 6.0, BufferUsage::Activations)
            .map_err(NnError::Graph)?,
        Some("relu") => ctx
            .unary(src, UnaryOp::Relu, BufferUsage::Activations)
            .map_err(NnError::Graph)?,
        Some("sigmoid") => ctx
            .unary(src, UnaryOp::Sigmoid, BufferUsage::Activations)
            .map_err(NnError::Graph)?,
        Some("hardswish") => ctx
            .unary(src, UnaryOp::Hardswish, BufferUsage::Activations)
            .map_err(NnError::Graph)?,
        Some("tanh") => ctx
            .unary(src, UnaryOp::Tanh, BufferUsage::Activations)
            .map_err(NnError::Graph)?,
        Some(other) => {
            return Err(NnError::Graph(format!(
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
) -> Result<TensorId, NnError> {
    let name = layer
        .weight
        .as_deref()
        .ok_or_else(|| NnError::Graph("convolution has no weight name".to_string()))?;
    let tensor = weights.get(name)?;
    if tensor.shape.len() != 4 {
        return Err(NnError::Graph(format!(
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
        return Err(NnError::Graph(format!(
            "depthwise {name} must have shape [C, 1, KH, KW], got {:?}",
            tensor.shape
        )));
    }

    // row-major [OC, IC, KH, KW] is the same bytes as ggml [KW, KH, IC, OC]
    let (ne2, ne3) = if depthwise { (1, oc) } else { (ic, oc) };
    let id = ctx
        .new_tensor_4d(TensorType::F32, kw, kh, ne2, ne3, BufferUsage::Weights)
        .map_err(NnError::Graph)?;
    ctx.set_tensor_name(id, name).ok();
    write_f32(ctx, id, &tensor.data)?;
    Ok(id)
}

fn add_channel_bias(
    ctx: &mut Context,
    conv: TensorId,
    bias_name: &str,
    weights: &Weights,
) -> Result<TensorId, NnError> {
    let bias = weights.get(bias_name)?;
    let channels = ctx
        .tensor(conv)
        .ok_or_else(|| NnError::Graph("convolution output vanished".to_string()))?
        .ne[2];
    if bias.elements() as i64 != channels {
        return Err(NnError::Graph(format!(
            "{bias_name} holds {} values but the convolution produces {} channels",
            bias.elements(),
            channels
        )));
    }

    let vector = ctx
        .new_tensor_4d(TensorType::F32, 1, 1, channels, 1, BufferUsage::Weights)
        .map_err(NnError::Graph)?;
    ctx.set_tensor_name(vector, bias_name).ok();
    write_f32(ctx, vector, &bias.data)?;

    let broadcast = ctx
        .repeat(vector, conv, BufferUsage::Activations)
        .map_err(NnError::Graph)?;
    ctx.binary_like_a(Op::Add, conv, broadcast, BufferUsage::Activations)
        .map_err(NnError::Graph)
}

fn write_f32(ctx: &mut Context, id: TensorId, values: &[f32]) -> Result<(), NnError> {
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
        .map_err(NnError::Graph)
}
