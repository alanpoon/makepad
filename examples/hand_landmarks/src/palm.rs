//! MediaPipe palm detection: the stage that finds hands before the landmark
//! model runs on each one.
//!
//! The model is an SSD over 2016 fixed anchors, emitting per anchor a box, 7
//! palm keypoints and a score. Decoding follows MediaPipe's
//! `TensorsToDetectionsCalculator` configuration for palm detection.

use makepad_ggml::backend::metal::{
    prepare_graph, BufferStorageMode, MetalGraphSession, MetalGraphTensorWrite, MetalRuntime,
};
use makepad_ggml::{Context, Graph, InitParams, TensorId};
use makepad_nn_graph::{graph, preprocess, spec, weights, Letterbox, NnError};
use std::path::Path;

/// Palm keypoints the detector emits, used to orient the hand.
pub const NUM_PALM_KEYPOINTS: usize = 7;
/// Keypoint 0 is the wrist center, 2 is the middle-finger knuckle; the vector
/// between them gives the hand's rotation.
pub const WRIST_KEYPOINT: usize = 0;
pub const MIDDLE_KNUCKLE_KEYPOINT: usize = 2;

/// Anchor grids for the 192x192 model: 24x24 with 2 anchors per cell, then
/// 12x12 with 6, which is where 2016 comes from.
const ANCHOR_GRIDS: [(usize, usize); 2] = [(24, 2), (12, 6)];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// One detected palm, in normalized source-image coordinates.
#[derive(Clone, Debug)]
pub struct Palm {
    pub score: f32,
    /// Axis-aligned box: center and size.
    pub center: Point,
    pub width: f32,
    pub height: f32,
    pub keypoints: [Point; NUM_PALM_KEYPOINTS],
}

impl Palm {
    pub fn left(&self) -> f32 {
        self.center.x - self.width * 0.5
    }
    pub fn top(&self) -> f32 {
        self.center.y - self.height * 0.5
    }
    pub fn right(&self) -> f32 {
        self.center.x + self.width * 0.5
    }
    pub fn bottom(&self) -> f32 {
        self.center.y + self.height * 0.5
    }
}

/// One anchor: a center, in normalized model space. All palm-detection
/// anchors are unit-sized, so only the center varies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub x: f32,
    pub y: f32,
}

/// Reproduces MediaPipe's `SsdAnchorsCalculator` output for this model: one
/// anchor per (cell, repeat), cells in row-major order, coarser grids last.
pub fn build_anchors() -> Vec<Anchor> {
    let mut anchors = Vec::new();
    for (grid, per_cell) in ANCHOR_GRIDS {
        for y in 0..grid {
            for x in 0..grid {
                let cx = (x as f32 + 0.5) / grid as f32;
                let cy = (y as f32 + 0.5) / grid as f32;
                for _ in 0..per_cell {
                    anchors.push(Anchor { x: cx, y: cy });
                }
            }
        }
    }
    anchors
}

/// Thresholds for turning raw model output into detections.
#[derive(Clone, Copy, Debug)]
pub struct DetectParams {
    pub min_score: f32,
    /// Boxes overlapping a kept box by more than this are suppressed.
    pub iou_threshold: f32,
    pub max_hands: usize,
}

impl Default for DetectParams {
    fn default() -> Self {
        Self {
            min_score: 0.5,
            iou_threshold: 0.3,
            max_hands: 2,
        }
    }
}

pub struct PalmDetector {
    ctx: Context,
    session: MetalGraphSession,
    input: TensorId,
    outputs: [TensorId; 2],
    input_size: usize,
    input_scale: f32,
    input_bias: f32,
    anchors: Vec<Anchor>,
    pub params: DetectParams,
}

impl PalmDetector {
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, NnError> {
        let dir = dir.as_ref();
        let spec_path = dir.join("palm_detector.json");
        let weights_path = dir.join("palm_detector.safetensors");
        let spec_text = std::fs::read_to_string(&spec_path)
            .map_err(|e| NnError::Io(format!("cannot read {}: {e}", spec_path.display())))?;
        let weight_bytes = std::fs::read(&weights_path)
            .map_err(|e| NnError::Io(format!("cannot read {}: {e}", weights_path.display())))?;
        let spec = spec::ModelSpec::from_json(&spec_text)?;
        let weights = weights::Weights::from_bytes(&weight_bytes)?;
        Self::from_parts(&spec, &weights)
    }

    pub fn from_parts(
        spec: &spec::ModelSpec,
        weights: &weights::Weights,
    ) -> Result<Self, NnError> {
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

        let mut ctx = Context::new(InitParams {
            mem_size: measured + measured / 8 + (8 << 20),
            mem_buffer: None,
            no_alloc: false,
        });
        let built = graph::build(&mut ctx, spec, weights)?;
        // out0 = boxes + keypoints, out1 = scores
        let outputs = [built.output("out0")?, built.output("out1")?];

        let mut g = Graph::new();
        for out in outputs {
            g.build_forward_expand(&ctx, out).map_err(NnError::Graph)?;
        }

        let runtime = MetalRuntime::new()
            .map_err(|e| NnError::Backend(format!("no Metal runtime: {e}")))?;
        let prepared = prepare_graph(&ctx, &g, runtime.features()).map_err(NnError::Backend)?;
        let session = MetalGraphSession::from_runtime(
            runtime,
            &ctx,
            &prepared,
            BufferStorageMode::Shared,
            BufferStorageMode::Shared,
        )
        .map_err(NnError::Backend)?;

        let anchors = build_anchors();
        let boxes = ctx
            .tensor(outputs[0])
            .ok_or_else(|| NnError::Graph("box output vanished".to_string()))?;
        if boxes.ne[1] as usize != anchors.len() {
            return Err(NnError::Spec(format!(
                "model has {} anchors but the anchor layout produces {}",
                boxes.ne[1],
                anchors.len()
            )));
        }

        Ok(Self {
            ctx,
            session,
            input: built.input,
            outputs,
            input_size: spec.input_width as usize,
            input_scale: spec.input_scale.unwrap_or(1.0),
            input_bias: spec.input_bias.unwrap_or(0.0),
            anchors,
            params: DetectParams::default(),
        })
    }

    pub fn input_size(&self) -> usize {
        self.input_size
    }

    /// Find palms in `rgb`, returning them in normalized source coordinates,
    /// strongest first.
    pub fn detect(&self, rgb: &[u8], width: usize, height: usize) -> Result<Vec<Palm>, NnError> {
        let (planes, letterbox) = preprocess::letterbox_rgb8(
            rgb,
            width,
            height,
            self.input_size,
            self.input_scale,
            self.input_bias,
        )
        .map_err(NnError::Input)?;

        let bytes: Vec<u8> = planes.iter().flat_map(|v| v.to_le_bytes()).collect();
        let execution = self
            .session
            .execute(
                &self.ctx,
                &[MetalGraphTensorWrite {
                    tensor_id: self.input,
                    bytes: &bytes,
                }],
                &self.outputs,
            )
            .map_err(NnError::Backend)?;

        let read = |id: TensorId, what: &str| -> Result<Vec<f32>, NnError> {
            let raw = execution
                .outputs
                .get(&id)
                .ok_or_else(|| NnError::Backend(format!("{what} was not returned")))?;
            Ok(raw
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect())
        };

        let boxes = read(self.outputs[0], "boxes")?;
        let scores = read(self.outputs[1], "scores")?;

        let mut palms = decode(
            &boxes,
            &scores,
            &self.anchors,
            self.input_size as f32,
            self.params,
        )?;
        palms = non_max_suppression(palms, self.params.iou_threshold, self.params.max_hands);

        // model space -> source image
        for palm in palms.iter_mut() {
            to_source(palm, &letterbox, width, height);
        }
        Ok(palms)
    }
}

/// Raw SSD output -> detections in normalized *model* space.
pub fn decode(
    boxes: &[f32],
    scores: &[f32],
    anchors: &[Anchor],
    scale: f32,
    params: DetectParams,
) -> Result<Vec<Palm>, NnError> {
    let stride = 4 + NUM_PALM_KEYPOINTS * 2;
    if boxes.len() < anchors.len() * stride || scores.len() < anchors.len() {
        return Err(NnError::Input(format!(
            "detector returned {} box values and {} scores for {} anchors",
            boxes.len(),
            scores.len(),
            anchors.len()
        )));
    }

    let mut out = Vec::new();
    for (i, anchor) in anchors.iter().enumerate() {
        // MediaPipe clips the logit before the sigmoid to avoid overflow
        let logit = scores[i].clamp(-100.0, 100.0);
        let score = 1.0 / (1.0 + (-logit).exp());
        if score < params.min_score {
            continue;
        }

        let b = &boxes[i * stride..(i + 1) * stride];
        // anchors are unit sized, so the raw values only need the scale
        let cx = b[0] / scale + anchor.x;
        let cy = b[1] / scale + anchor.y;
        let w = b[2] / scale;
        let h = b[3] / scale;

        let mut keypoints = [Point::default(); NUM_PALM_KEYPOINTS];
        for (k, kp) in keypoints.iter_mut().enumerate() {
            *kp = Point {
                x: b[4 + k * 2] / scale + anchor.x,
                y: b[4 + k * 2 + 1] / scale + anchor.y,
            };
        }

        out.push(Palm {
            score,
            center: Point { x: cx, y: cy },
            width: w,
            height: h,
            keypoints,
        });
    }

    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}

pub fn iou(a: &Palm, b: &Palm) -> f32 {
    let x0 = a.left().max(b.left());
    let y0 = a.top().max(b.top());
    let x1 = a.right().min(b.right());
    let y1 = a.bottom().min(b.bottom());
    let inter = (x1 - x0).max(0.0) * (y1 - y0).max(0.0);
    let union = a.width * a.height + b.width * b.height - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Plain greedy NMS over score-sorted detections.
pub fn non_max_suppression(sorted: Vec<Palm>, iou_threshold: f32, max_out: usize) -> Vec<Palm> {
    let mut kept: Vec<Palm> = Vec::new();
    for candidate in sorted {
        if kept.len() >= max_out {
            break;
        }
        if kept.iter().any(|k| iou(k, &candidate) > iou_threshold) {
            continue;
        }
        kept.push(candidate);
    }
    kept
}

fn to_source(palm: &mut Palm, letterbox: &Letterbox, width: usize, height: usize) {
    let map = |p: Point| -> Point {
        let (x, y) = letterbox.to_source_normalized(p.x, p.y, width, height);
        Point { x, y }
    };
    // sizes scale by the letterbox factor, in each axis' own units
    let sx = letterbox.size as f32 / (letterbox.scale * width as f32);
    let sy = letterbox.size as f32 / (letterbox.scale * height as f32);
    palm.center = map(palm.center);
    palm.width *= sx;
    palm.height *= sy;
    for kp in palm.keypoints.iter_mut() {
        *kp = map(*kp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_layout_matches_the_model() {
        let anchors = build_anchors();
        assert_eq!(anchors.len(), 2016);
        // first cell of the 24x24 grid, repeated twice
        assert!((anchors[0].x - 0.5 / 24.0).abs() < 1e-6);
        assert_eq!(anchors[0], anchors[1]);
        // second cell steps one column across
        assert!((anchors[2].x - 1.5 / 24.0).abs() < 1e-6);
        // the 12x12 grid starts after 24*24*2 entries, with 6 per cell
        let coarse = &anchors[1152..];
        assert_eq!(coarse.len(), 864);
        assert!((coarse[0].x - 0.5 / 12.0).abs() < 1e-6);
        assert_eq!(coarse[0], coarse[5]);
        assert!((coarse[6].x - 1.5 / 12.0).abs() < 1e-6);
    }

    fn raw_for(anchor_index: usize, anchors: &[Anchor], scale: f32) -> (Vec<f32>, Vec<f32>) {
        let stride = 4 + NUM_PALM_KEYPOINTS * 2;
        let mut boxes = vec![0.0f32; anchors.len() * stride];
        let mut scores = vec![-50.0f32; anchors.len()];
        let b = &mut boxes[anchor_index * stride..(anchor_index + 1) * stride];
        b[0] = 0.0; // centered on the anchor
        b[1] = 0.0;
        b[2] = 0.25 * scale; // quarter-frame wide
        b[3] = 0.5 * scale;
        // wrist below the center, middle knuckle above it
        b[4 + WRIST_KEYPOINT * 2 + 1] = 0.1 * scale;
        b[4 + MIDDLE_KNUCKLE_KEYPOINT * 2 + 1] = -0.1 * scale;
        scores[anchor_index] = 5.0; // sigmoid(5) ~ 0.993
        (boxes, scores)
    }

    #[test]
    fn decodes_a_box_relative_to_its_anchor() {
        let anchors = build_anchors();
        let scale = 192.0;
        let idx = 700;
        let (boxes, scores) = raw_for(idx, &anchors, scale);

        let palms = decode(&boxes, &scores, &anchors, scale, DetectParams::default()).unwrap();
        assert_eq!(palms.len(), 1);
        let p = &palms[0];
        assert!(p.score > 0.99);
        assert!((p.center.x - anchors[idx].x).abs() < 1e-6);
        assert!((p.width - 0.25).abs() < 1e-6);
        assert!((p.height - 0.5).abs() < 1e-6);
        assert!((p.keypoints[WRIST_KEYPOINT].y - (anchors[idx].y + 0.1)).abs() < 1e-5);
        assert!((p.keypoints[MIDDLE_KNUCKLE_KEYPOINT].y - (anchors[idx].y - 0.1)).abs() < 1e-5);
    }

    #[test]
    fn drops_everything_below_the_score_threshold() {
        let anchors = build_anchors();
        let stride = 4 + NUM_PALM_KEYPOINTS * 2;
        let boxes = vec![0.0f32; anchors.len() * stride];
        let scores = vec![-1.0f32; anchors.len()]; // sigmoid(-1) = 0.27
        let params = DetectParams::default();
        assert!(decode(&boxes, &scores, &anchors, 192.0, params).unwrap().is_empty());

        let loose = DetectParams { min_score: 0.2, ..params };
        assert_eq!(
            decode(&boxes, &scores, &anchors, 192.0, loose).unwrap().len(),
            anchors.len()
        );
    }

    fn palm_at(x: f32, y: f32, size: f32, score: f32) -> Palm {
        Palm {
            score,
            center: Point { x, y },
            width: size,
            height: size,
            keypoints: [Point::default(); NUM_PALM_KEYPOINTS],
        }
    }

    #[test]
    fn suppresses_overlapping_boxes_but_keeps_separate_hands() {
        let input = vec![
            palm_at(0.3, 0.5, 0.2, 0.9),
            palm_at(0.31, 0.51, 0.2, 0.8), // almost the same box
            palm_at(0.8, 0.5, 0.2, 0.7),   // a second hand
        ];
        let kept = non_max_suppression(input, 0.3, 2);
        assert_eq!(kept.len(), 2);
        assert!((kept[0].center.x - 0.3).abs() < 1e-6);
        assert!((kept[1].center.x - 0.8).abs() < 1e-6);
    }

    #[test]
    fn respects_the_hand_limit() {
        let input = (0..5)
            .map(|i| palm_at(0.1 + i as f32 * 0.2, 0.5, 0.05, 0.9 - i as f32 * 0.1))
            .collect();
        assert_eq!(non_max_suppression(input, 0.3, 2).len(), 2);
    }

    #[test]
    fn iou_is_zero_for_disjoint_boxes_and_one_for_identical() {
        let a = palm_at(0.2, 0.2, 0.1, 1.0);
        let b = palm_at(0.8, 0.8, 0.1, 1.0);
        assert!(iou(&a, &b).abs() < 1e-6);
        assert!((iou(&a, &a) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn rejects_short_output_tensors() {
        let anchors = build_anchors();
        assert!(decode(&[0.0; 10], &[0.0; 10], &anchors, 192.0, DetectParams::default()).is_err());
    }
}
