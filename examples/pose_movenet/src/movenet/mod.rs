//! MoveNet single-pose estimation on top of `makepad-ggml`.
//!
//! A model directory holds two files:
//!   * `movenet.json`    — the layer list, written by `tools/convert_movenet.py`
//!   * `movenet.safetensors` — the weights, in ggml's kernel layout
//!
//! [`Estimator::load`] builds the ggml graph once; [`Estimator::estimate`]
//! runs it per frame and returns a [`Pose`] in normalized source coordinates.

pub mod decode;
pub mod graph;
pub mod preprocess;
pub mod spec;
pub mod weights;

pub use decode::{DecodeParams, Keypoint, Pose, KEYPOINT_NAMES, NUM_KEYPOINTS, SKELETON};
pub use preprocess::Letterbox;

use makepad_ggml::backend::metal::{
    prepare_graph, BufferStorageMode, MetalGraphSession, MetalGraphTensorWrite, MetalRuntime,
};
use makepad_ggml::{Context, Graph, InitParams, TensorId};
use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub enum MoveNetError {
    Io(String),
    Spec(String),
    Weights(String),
    MissingTensor(String),
    Graph(String),
    Backend(String),
    Decode(String),
}

impl MoveNetError {
    /// Annotate a graph error with the layer that produced it, which is the
    /// only way to find a bad entry in a several-hundred-layer spec.
    fn with_layer(self, index: usize, op: &str, output: &str) -> Self {
        match self {
            Self::Graph(msg) => Self::Graph(format!("layer {index} ({op} -> {output}): {msg}")),
            Self::MissingTensor(name) => Self::Graph(format!(
                "layer {index} ({op} -> {output}): weights have no tensor named {name}"
            )),
            other => other,
        }
    }
}

impl fmt::Display for MoveNetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(m) => write!(f, "io error: {m}"),
            Self::Spec(m) => write!(f, "model spec error: {m}"),
            Self::Weights(m) => write!(f, "weights error: {m}"),
            Self::MissingTensor(m) => write!(f, "weights have no tensor named {m}"),
            Self::Graph(m) => write!(f, "graph error: {m}"),
            Self::Backend(m) => write!(f, "backend error: {m}"),
            Self::Decode(m) => write!(f, "decode error: {m}"),
        }
    }
}

impl std::error::Error for MoveNetError {}

pub struct Estimator {
    ctx: Context,
    session: MetalGraphSession,
    input: TensorId,
    heads: [TensorId; 4],
    input_size: usize,
    input_scale: f32,
    input_bias: f32,
    grid_width: usize,
    grid_height: usize,
    name: String,
    decode_params: DecodeParams,
}

impl Estimator {
    /// Load `movenet.json` + `movenet.safetensors` from a directory.
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, MoveNetError> {
        let dir = dir.as_ref();
        let spec_path = dir.join("movenet.json");
        let weights_path = dir.join("movenet.safetensors");

        let spec_text = std::fs::read_to_string(&spec_path).map_err(|e| {
            MoveNetError::Io(format!("cannot read {}: {e}", spec_path.display()))
        })?;
        let weight_bytes = std::fs::read(&weights_path).map_err(|e| {
            MoveNetError::Io(format!("cannot read {}: {e}", weights_path.display()))
        })?;

        let spec = spec::ModelSpec::from_json(&spec_text)?;
        let weights = weights::Weights::from_bytes(&weight_bytes)?;
        Self::from_parts(&spec, &weights)
    }

    pub fn from_parts(
        spec: &spec::ModelSpec,
        weights: &weights::Weights,
    ) -> Result<Self, MoveNetError> {
        if spec.input_width != spec.input_height {
            return Err(MoveNetError::Spec(format!(
                "model input must be square, got {}x{}",
                spec.input_width, spec.input_height
            )));
        }

        // Size the arena by building the graph once with no backing store and
        // adding up what the tensors would need. Guessing instead either wastes
        // hundreds of MB or dies partway through a deep model.
        let mut measure = Context::new(InitParams {
            mem_size: 0,
            mem_buffer: None,
            no_alloc: true,
        });
        graph::build(&mut measure, spec, weights)?;
        let measured: usize = measure
            .tensors()
            .iter()
            .map(|t| makepad_ggml::ggml_pad(t.nbytes(), makepad_ggml::GGML_MEM_ALIGN))
            .sum();
        drop(measure);

        // headroom for alignment padding and the graph's own bookkeeping
        let mem_size = measured + (measured / 8) + (8 << 20);

        let mut ctx = Context::new(InitParams {
            mem_size,
            mem_buffer: None,
            no_alloc: false,
        });

        let built = graph::build(&mut ctx, spec, weights)?;

        let mut g = Graph::new();
        for out in [
            built.heatmap,
            built.center,
            built.regress,
            built.offset,
        ] {
            g.build_forward_expand(&ctx, out)
                .map_err(MoveNetError::Graph)?;
        }

        let runtime = MetalRuntime::new().map_err(|e| {
            MoveNetError::Backend(format!(
                "no Metal runtime: {e} (this example needs a Metal-capable machine)"
            ))
        })?;
        let prepared =
            prepare_graph(&ctx, &g, runtime.features()).map_err(MoveNetError::Backend)?;
        let session = MetalGraphSession::from_runtime(
            runtime,
            &ctx,
            &prepared,
            BufferStorageMode::Shared,
            BufferStorageMode::Shared,
        )
        .map_err(MoveNetError::Backend)?;

        let head = ctx
            .tensor(built.heatmap)
            .ok_or_else(|| MoveNetError::Graph("heatmap head vanished".to_string()))?;
        let grid_width = head.ne[0] as usize;
        let grid_height = head.ne[1] as usize;
        if head.ne[2] as usize != NUM_KEYPOINTS {
            return Err(MoveNetError::Spec(format!(
                "heatmap head has {} channels, expected {NUM_KEYPOINTS}",
                head.ne[2]
            )));
        }

        Ok(Self {
            ctx,
            session,
            input: built.input,
            heads: [built.heatmap, built.center, built.regress, built.offset],
            input_size: spec.input_width as usize,
            input_scale: spec.input_scale.unwrap_or(1.0),
            input_bias: spec.input_bias.unwrap_or(0.0),
            grid_width,
            grid_height,
            name: spec.name.clone(),
            decode_params: DecodeParams::default(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn input_size(&self) -> usize {
        self.input_size
    }

    pub fn grid(&self) -> (usize, usize) {
        (self.grid_width, self.grid_height)
    }

    pub fn set_decode_params(&mut self, params: DecodeParams) {
        self.decode_params = params;
    }

    /// Run one frame. `rgb` is 8-bit RGB, row-major, `width * height * 3`.
    /// Keypoints come back normalized to the source image, not the padded
    /// model square.
    pub fn estimate(
        &self,
        rgb: &[u8],
        width: usize,
        height: usize,
    ) -> Result<Pose, MoveNetError> {
        let (planes, letterbox) = preprocess::letterbox_rgb8(
            rgb,
            width,
            height,
            self.input_size,
            self.input_scale,
            self.input_bias,
        )
        .map_err(MoveNetError::Decode)?;

        let mut bytes = Vec::with_capacity(planes.len() * 4);
        for v in &planes {
            bytes.extend_from_slice(&v.to_le_bytes());
        }

        let execution = self
            .session
            .execute(
                &self.ctx,
                &[MetalGraphTensorWrite {
                    tensor_id: self.input,
                    bytes: &bytes,
                }],
                &self.heads,
            )
            .map_err(MoveNetError::Backend)?;

        let plane = |id: TensorId, what: &str| -> Result<Vec<f32>, MoveNetError> {
            let raw = execution
                .outputs
                .get(&id)
                .ok_or_else(|| MoveNetError::Backend(format!("{what} head was not returned")))?;
            Ok(raw
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect())
        };

        let heatmap = plane(self.heads[0], "heatmap")?;
        let center = plane(self.heads[1], "center")?;
        let regress = plane(self.heads[2], "regress")?;
        let offset = plane(self.heads[3], "offset")?;

        let heads = decode::Heads {
            width: self.grid_width,
            height: self.grid_height,
            heatmap: &heatmap,
            center: &center,
            regress: &regress,
            offset: &offset,
        };
        let mut pose = decode::decode(&heads, self.decode_params).map_err(MoveNetError::Decode)?;

        // model space -> source image space, undoing the letterbox
        for kp in pose.keypoints.iter_mut() {
            let (x, y) = letterbox.to_source_normalized(kp.x, kp.y, width, height);
            kp.x = x;
            kp.y = y;
        }
        Ok(pose)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny model in the same shape as the real thing: one strided conv
    /// backbone layer feeding four heads. It exercises spec parsing, weight
    /// upload, graph construction, Metal execution and decoding without
    /// needing the multi-megabyte MoveNet download.
    fn tiny_spec(input: i64) -> String {
        let head = |name: &str, activation: &str| {
            format!(
                r#"{{"op":"conv2d","inputs":["trunk"],"output":"{name}","weight":"{name}.w","bias":"{name}.b","activation":"{activation}"}}"#
            )
        };
        format!(
            r#"{{
                "name": "tiny",
                "input_width": {input},
                "input_height": {input},
                "layers": [
                    {{"op":"conv2d","inputs":["image"],"output":"stem",
                      "weight":"stem.w","bias":"stem.b","stride":[2,2],"pad":[1,1],
                      "activation":"relu6"}},
                    {{"op":"dwconv2d","inputs":["stem"],"output":"dw",
                      "weight":"dw.w","bias":"dw.b","stride":[2,2],"pad":[1,1],
                      "activation":"relu6"}},
                    {{"op":"conv2d","inputs":["dw"],"output":"trunk",
                      "weight":"trunk.w","bias":"trunk.b","activation":"relu"}},
                    {},
                    {},
                    {},
                    {}
                ],
                "outputs": {{"heatmap":"heatmap","center":"center",
                             "regress":"regress","offset":"offset"}}
            }}"#,
            head("heatmap", "sigmoid"),
            head("center", "sigmoid"),
            head("regress", "none"),
            head("offset", "none"),
        )
    }

    fn tiny_weights() -> weights::Weights {
        // [OC, IC, KH, KW] kernels and [OC] biases, filled with a smooth ramp
        let ramp = |n: usize| -> Vec<f32> {
            (0..n).map(|i| ((i % 17) as f32 - 8.0) * 0.01).collect()
        };
        let entries: Vec<(&str, Vec<usize>, Vec<f32>)> = vec![
            ("stem.w", vec![8, 3, 3, 3], ramp(8 * 3 * 3 * 3)),
            ("stem.b", vec![8], ramp(8)),
            ("dw.w", vec![8, 1, 3, 3], ramp(8 * 9)),
            ("dw.b", vec![8], ramp(8)),
            ("trunk.w", vec![16, 8, 1, 1], ramp(16 * 8)),
            ("trunk.b", vec![16], ramp(16)),
            ("heatmap.w", vec![17, 16, 1, 1], ramp(17 * 16)),
            ("heatmap.b", vec![17], ramp(17)),
            ("center.w", vec![1, 16, 1, 1], ramp(16)),
            ("center.b", vec![1], ramp(1)),
            ("regress.w", vec![34, 16, 1, 1], ramp(34 * 16)),
            ("regress.b", vec![34], ramp(34)),
            ("offset.w", vec![34, 16, 1, 1], ramp(34 * 16)),
            ("offset.b", vec![34], ramp(34)),
        ];

        let mut header = String::from("{");
        let mut blob: Vec<u8> = Vec::new();
        for (i, (name, shape, values)) in entries.iter().enumerate() {
            let start = blob.len();
            for v in values {
                blob.extend_from_slice(&v.to_le_bytes());
            }
            if i > 0 {
                header.push(',');
            }
            let shape_json = shape
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(",");
            header.push_str(&format!(
                "\"{name}\":{{\"dtype\":\"F32\",\"shape\":[{shape_json}],\"data_offsets\":[{start},{}]}}",
                blob.len()
            ));
        }
        header.push('}');

        let mut file = Vec::new();
        file.extend_from_slice(&(header.len() as u64).to_le_bytes());
        file.extend_from_slice(header.as_bytes());
        file.extend_from_slice(&blob);
        weights::Weights::from_bytes(&file).unwrap()
    }

    #[test]
    fn builds_and_runs_a_tiny_model_end_to_end() {
        if !MetalRuntime::is_available() {
            return;
        }
        let spec = spec::ModelSpec::from_json(&tiny_spec(64)).unwrap();
        let estimator = Estimator::from_parts(&spec, &tiny_weights()).unwrap();

        // 64 -> stride 2 -> 32 -> stride 2 -> 16
        assert_eq!(estimator.grid(), (16, 16));
        assert_eq!(estimator.input_size(), 64);

        let (w, h) = (96usize, 48usize);
        let mut rgb = vec![0u8; w * h * 3];
        for (i, px) in rgb.chunks_exact_mut(3).enumerate() {
            px[0] = (i % 255) as u8;
            px[1] = ((i / 3) % 255) as u8;
            px[2] = 200;
        }

        let pose = estimator.estimate(&rgb, w, h).unwrap();
        assert_eq!(pose.keypoints.len(), NUM_KEYPOINTS);
        for kp in pose.keypoints {
            assert!(kp.score.is_finite());
            assert!(kp.x.is_finite() && kp.y.is_finite());
        }
    }

    #[test]
    fn reports_the_layer_that_references_a_missing_tensor() {
        let spec = spec::ModelSpec::from_json(&tiny_spec(64)).unwrap();
        let mut ctx = Context::new(InitParams {
            mem_size: 1 << 22,
            mem_buffer: None,
            no_alloc: false,
        });
        let empty = weights::Weights::default();
        let err = graph::build(&mut ctx, &spec, &empty).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("layer 0"), "unhelpful error: {text}");
        assert!(text.contains("stem.w"), "unhelpful error: {text}");
    }
}

/// Runs the converted model in `model/` over `assets/pose.jpg` when both are
/// present. Skipped when they are not, so a fresh checkout still tests green.
#[cfg(test)]
mod real_model_tests {
    use super::*;

    fn example_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn finds_a_person_in_the_bundled_photo() {
        let dir = example_dir();
        let model = dir.join("model");
        let photo = dir.join("assets/pose.jpg");
        if !model.join("movenet.json").exists() || !photo.exists() {
            eprintln!("skipping: no converted model or photo");
            return;
        }
        if !MetalRuntime::is_available() {
            return;
        }

        let estimator = Estimator::load(&model).unwrap();
        assert_eq!(estimator.input_size(), 192);
        assert_eq!(estimator.grid(), (48, 48));

        let bytes = std::fs::read(&photo).unwrap();
        let img = crate::image_io::decode(&bytes).unwrap();
        let pose = estimator.estimate(&img.rgb, img.width, img.height).unwrap();

        for (i, kp) in pose.keypoints.iter().enumerate() {
            eprintln!(
                "{:>14}  score {:.3}  at ({:.3}, {:.3})",
                KEYPOINT_NAMES[i], kp.score, kp.x, kp.y
            );
        }
        eprintln!("center score {:.3}", pose.center_score);

        // sanity, not accuracy: the heads must produce probabilities and
        // coordinates that land on the picture
        for kp in pose.keypoints.iter() {
            assert!((0.0..=1.0).contains(&kp.score), "score {} out of range", kp.score);
            assert!(kp.x > -0.5 && kp.x < 1.5, "x {} way off frame", kp.x);
            assert!(kp.y > -0.5 && kp.y < 1.5, "y {} way off frame", kp.y);
        }
        let confident = pose.keypoints.iter().filter(|k| k.score > 0.3).count();
        assert!(
            confident >= 10,
            "only {confident}/17 joints above 0.3 on a clear single-person photo"
        );
    }
}
