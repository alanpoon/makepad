//! The model description that `tools/convert_movenet.py` writes next to the
//! weights.
//!
//! The topology is data, not code: the converter walks the real MoveNet graph
//! and emits the layer list, so this example does not hard-code an assumed
//! MobileNetV2/FPN arrangement that may not match the model you downloaded.

use makepad_micro_serde::*;

#[derive(Clone, Debug, DeJson)]
pub struct ModelSpec {
    pub name: String,
    /// Square model input, e.g. 192 for Lightning, 256 for Thunder.
    pub input_width: i64,
    pub input_height: i64,
    /// Pixel scaling applied after decoding to [0,1]: `value * scale + bias`.
    /// MoveNet's TFLite graphs take [0,1] floats, so both default to identity.
    pub input_scale: Option<f32>,
    pub input_bias: Option<f32>,
    pub layers: Vec<LayerSpec>,
    pub outputs: OutputSpec,
}

/// The four MoveNet prediction heads, named by the tensor each produces.
#[derive(Clone, Debug, DeJson)]
pub struct OutputSpec {
    /// Per-keypoint heatmap, 17 channels, already through sigmoid.
    pub heatmap: String,
    /// Person-center heatmap, 1 channel, already through sigmoid.
    pub center: String,
    /// Center-to-keypoint regression, 34 channels, (y, x) pairs in grid units.
    pub regress: String,
    /// Sub-pixel refinement at the keypoint cell, 34 channels, (y, x) pairs.
    pub offset: String,
}

#[derive(Clone, Debug, DeJson)]
pub struct LayerSpec {
    /// One of: conv2d, dwconv2d, add, mul, concat, upsample, activation.
    pub op: String,
    pub inputs: Vec<String>,
    pub output: String,
    pub weight: Option<String>,
    pub bias: Option<String>,
    /// `[x, y]`, defaults to `[1, 1]`.
    pub stride: Option<Vec<i32>>,
    /// `[x, y]`, defaults to `[0, 0]`. Only used when `pad_lrtb` is absent.
    pub pad: Option<Vec<i32>>,
    /// `[left, right, top, bottom]`. TensorFlow's SAME padding puts the odd
    /// pixel on the right/bottom, which a single symmetric value cannot
    /// express — and getting it wrong changes the output size.
    pub pad_lrtb: Option<Vec<i32>>,
    /// `[x, y]`, defaults to `[1, 1]`.
    pub dilation: Option<Vec<i32>>,
    /// relu, relu6, sigmoid, hardswish, or none.
    pub activation: Option<String>,
    /// upsample only: integer scale factor.
    pub factor: Option<i64>,
    /// upsample only: nearest (default) or bilinear.
    pub mode: Option<String>,
    /// concat only: dimension index in ggml order, 2 = channels.
    pub dim: Option<usize>,
}

impl LayerSpec {
    pub fn stride_xy(&self) -> (i32, i32) {
        pair(&self.stride, 1)
    }

    pub fn pad_xy(&self) -> (i32, i32) {
        pair(&self.pad, 0)
    }

    /// `[left, right, top, bottom]`, falling back to the symmetric `pad`.
    pub fn pad_sides(&self) -> [i32; 4] {
        match &self.pad_lrtb {
            Some(v) if v.len() == 4 => [v[0], v[1], v[2], v[3]],
            _ => {
                let (x, y) = self.pad_xy();
                [x, x, y, y]
            }
        }
    }

    pub fn pad_is_symmetric(&self) -> bool {
        let [l, r, t, b] = self.pad_sides();
        l == r && t == b
    }

    pub fn dilation_xy(&self) -> (i32, i32) {
        pair(&self.dilation, 1)
    }
}

fn pair(value: &Option<Vec<i32>>, default: i32) -> (i32, i32) {
    match value {
        Some(v) if v.len() == 2 => (v[0], v[1]),
        Some(v) if v.len() == 1 => (v[0], v[0]),
        _ => (default, default),
    }
}

impl ModelSpec {
    pub fn from_json(text: &str) -> Result<Self, super::MoveNetError> {
        DeJson::deserialize_json(text)
            .map_err(|e| super::MoveNetError::Spec(format!("model spec is not valid json: {e:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_minimal_spec() {
        let spec = ModelSpec::from_json(
            r#"{
                "name": "test",
                "input_width": 192,
                "input_height": 192,
                "layers": [
                    {"op":"conv2d","inputs":["image"],"output":"stem",
                     "weight":"stem.w","bias":"stem.b","stride":[2,2],
                     "pad":[1,1],"activation":"relu6"}
                ],
                "outputs": {"heatmap":"h","center":"c","regress":"r","offset":"o"}
            }"#,
        )
        .unwrap();

        assert_eq!(spec.name, "test");
        assert_eq!(spec.input_width, 192);
        assert_eq!(spec.layers.len(), 1);
        let layer = &spec.layers[0];
        assert_eq!(layer.stride_xy(), (2, 2));
        assert_eq!(layer.pad_xy(), (1, 1));
        // absent in the json, so it falls back to no dilation
        assert_eq!(layer.dilation_xy(), (1, 1));
        assert_eq!(layer.activation.as_deref(), Some("relu6"));
        assert_eq!(spec.outputs.heatmap, "h");
    }
}
