//! MediaPipe hand landmark detection: 21 landmarks per hand.
//!
//! This runs the landmark stage of MediaPipe's Hand Landmarker. It expects the
//! hand to fill most of the frame — the palm-detection stage that normally
//! crops and rotates a region of interest is not implemented yet (see the
//! README), so point it at a hand shot or pass a crop rectangle.

use makepad_ggml::backend::metal::{
    prepare_graph, BufferStorageMode, MetalGraphSession, MetalGraphTensorWrite, MetalRuntime,
};
use makepad_ggml::{Context, Graph, InitParams, TensorId};
use makepad_nn_graph::{graph, preprocess, spec, weights, Letterbox, NnError};
use std::path::Path;

pub const NUM_LANDMARKS: usize = 21;

/// MediaPipe landmark order: wrist, then thumb → pinky, 4 joints each,
/// running from the knuckle out to the tip.
pub const LANDMARK_NAMES: [&str; NUM_LANDMARKS] = [
    "wrist",
    "thumb_cmc",
    "thumb_mcp",
    "thumb_ip",
    "thumb_tip",
    "index_mcp",
    "index_pip",
    "index_dip",
    "index_tip",
    "middle_mcp",
    "middle_pip",
    "middle_dip",
    "middle_tip",
    "ring_mcp",
    "ring_pip",
    "ring_dip",
    "ring_tip",
    "pinky_mcp",
    "pinky_pip",
    "pinky_dip",
    "pinky_tip",
];

/// Bones: the palm outline plus one chain per finger.
pub const CONNECTIONS: [(usize, usize); 21] = [
    // thumb
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 4),
    // index
    (0, 5),
    (5, 6),
    (6, 7),
    (7, 8),
    // middle
    (9, 10),
    (10, 11),
    (11, 12),
    // ring
    (13, 14),
    (14, 15),
    (15, 16),
    // pinky
    (0, 17),
    (17, 18),
    (18, 19),
    (19, 20),
    // knuckle line across the palm
    (5, 9),
    (9, 13),
    (13, 17),
];

/// Index of each fingertip, for callers that only care about tips.
pub const FINGERTIPS: [usize; 5] = [4, 8, 12, 16, 20];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Landmark {
    /// Normalized to the source image, 0..1, top-left origin.
    pub x: f32,
    pub y: f32,
    /// Depth relative to the wrist, in the same units as `x`. Negative is
    /// towards the camera. This is the model's rough depth, not metric.
    pub z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Handedness {
    Left,
    Right,
}

#[derive(Clone, Debug)]
pub struct Hand {
    pub landmarks: [Landmark; NUM_LANDMARKS],
    /// Landmarks in the model's world space, roughly metric, origin at the
    /// hand's approximate center.
    pub world: [Landmark; NUM_LANDMARKS],
    /// Confidence that a hand is present at all, 0..1.
    pub presence: f32,
    /// Which hand this is, with the score the model gave it.
    pub handedness: Handedness,
    pub handedness_score: f32,
}

impl Hand {
    pub fn fingertips(&self) -> [Landmark; 5] {
        FINGERTIPS.map(|i| self.landmarks[i])
    }
}

pub struct HandLandmarker {
    ctx: Context,
    session: MetalGraphSession,
    input: TensorId,
    outputs: [TensorId; 4],
    input_size: usize,
    input_scale: f32,
    input_bias: f32,
    name: String,
}

impl HandLandmarker {
    /// Load `hand_landmarks.json` + `hand_landmarks.safetensors` from a
    /// directory.
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, NnError> {
        let dir = dir.as_ref();
        let spec_path = dir.join("hand_landmarks.json");
        let weights_path = dir.join("hand_landmarks.safetensors");
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
        // size the arena by building once with no backing store
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

        // out0 = landmarks, out1 = presence, out2 = handedness, out3 = world
        let outputs = [
            built.output("out0")?,
            built.output("out1")?,
            built.output("out2")?,
            built.output("out3")?,
        ];

        let mut g = Graph::new();
        for out in outputs {
            g.build_forward_expand(&ctx, out).map_err(NnError::Graph)?;
        }

        let runtime = MetalRuntime::new().map_err(|e| {
            NnError::Backend(format!("no Metal runtime: {e} (this example needs Metal)"))
        })?;
        let prepared = prepare_graph(&ctx, &g, runtime.features()).map_err(NnError::Backend)?;
        let session = MetalGraphSession::from_runtime(
            runtime,
            &ctx,
            &prepared,
            BufferStorageMode::Shared,
            BufferStorageMode::Shared,
        )
        .map_err(NnError::Backend)?;

        Ok(Self {
            ctx,
            session,
            input: built.input,
            outputs,
            input_size: spec.input_width as usize,
            input_scale: spec.input_scale.unwrap_or(1.0),
            input_bias: spec.input_bias.unwrap_or(0.0),
            name: spec.name.clone(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn input_size(&self) -> usize {
        self.input_size
    }

    pub fn input_scale(&self) -> f32 {
        self.input_scale
    }

    pub fn input_bias(&self) -> f32 {
        self.input_bias
    }

    /// Run the model on an already-prepared planar CHW buffer — the rotated
    /// ROI crop from [`crate::roi`]. Coordinates come back in crop space
    /// (0..1 across the crop), for the caller to map home.
    pub fn detect_planar(&self, planes: &[f32]) -> Result<Hand, NnError> {
        let expected = self.input_size * self.input_size * 3;
        if planes.len() != expected {
            return Err(NnError::Input(format!(
                "crop holds {} values, expected {expected}",
                planes.len()
            )));
        }
        let bytes: Vec<u8> = planes.iter().flat_map(|v| v.to_le_bytes()).collect();
        let (raw, presence, handedness, world) = self.run(&bytes)?;

        // identity letterbox: crop space is already the model square
        let identity = Letterbox {
            size: self.input_size,
            scale: 1.0,
            pad_x: 0.0,
            pad_y: 0.0,
        };
        decode(
            &raw,
            &world,
            presence,
            handedness,
            &identity,
            self.input_size,
            self.input_size,
            self.input_size,
        )
    }

    #[allow(clippy::type_complexity)]
    fn run(&self, bytes: &[u8]) -> Result<(Vec<f32>, f32, f32, Vec<f32>), NnError> {
        let execution = self
            .session
            .execute(
                &self.ctx,
                &[MetalGraphTensorWrite {
                    tensor_id: self.input,
                    bytes,
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

        Ok((
            read(self.outputs[0], "landmarks")?,
            read(self.outputs[1], "presence")?.first().copied().unwrap_or(0.0),
            read(self.outputs[2], "handedness")?.first().copied().unwrap_or(0.5),
            read(self.outputs[3], "world landmarks")?,
        ))
    }

    /// Run the model over a whole frame, letterboxed. Without a preceding
    /// palm detection this sees far more than a hand, so prefer
    /// [`Self::detect_planar`] with an ROI crop.
    pub fn detect(&self, rgb: &[u8], width: usize, height: usize) -> Result<Hand, NnError> {
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
        let (raw_landmarks, presence, handedness, raw_world) = self.run(&bytes)?;

        decode(
            &raw_landmarks,
            &raw_world,
            presence,
            handedness,
            &letterbox,
            self.input_size,
            width,
            height,
        )
    }
}

/// Turn the flat model outputs into a [`Hand`].
///
/// Image landmarks arrive as 21 (x, y, z) triples in model-input pixels;
/// world landmarks are already in a hand-centered metric space.
#[allow(clippy::too_many_arguments)]
pub fn decode(
    raw_landmarks: &[f32],
    raw_world: &[f32],
    presence: f32,
    handedness_score: f32,
    letterbox: &Letterbox,
    input_size: usize,
    width: usize,
    height: usize,
) -> Result<Hand, NnError> {
    if raw_landmarks.len() < NUM_LANDMARKS * 3 {
        return Err(NnError::Input(format!(
            "landmark output holds {} values, expected {}",
            raw_landmarks.len(),
            NUM_LANDMARKS * 3
        )));
    }

    let mut landmarks = [Landmark::default(); NUM_LANDMARKS];
    for (i, lm) in landmarks.iter_mut().enumerate() {
        // model pixels -> normalized model space -> source image
        let mx = raw_landmarks[i * 3] / input_size as f32;
        let my = raw_landmarks[i * 3 + 1] / input_size as f32;
        let (x, y) = letterbox.to_source_normalized(mx, my, width, height);
        *lm = Landmark {
            x,
            y,
            // z is in model pixels like x, so it only needs the letterbox
            // scale and the source width to become a normalized depth
            z: raw_landmarks[i * 3 + 2] / (letterbox.scale * width as f32),
        };
    }

    let mut world = [Landmark::default(); NUM_LANDMARKS];
    if raw_world.len() >= NUM_LANDMARKS * 3 {
        for (i, lm) in world.iter_mut().enumerate() {
            *lm = Landmark {
                x: raw_world[i * 3],
                y: raw_world[i * 3 + 1],
                z: raw_world[i * 3 + 2],
            };
        }
    }

    // the handedness output is P(right hand) from the model's point of view
    let (handedness, score) = if handedness_score >= 0.5 {
        (Handedness::Right, handedness_score)
    } else {
        (Handedness::Left, 1.0 - handedness_score)
    };

    Ok(Hand {
        landmarks,
        world,
        presence,
        handedness,
        handedness_score: score,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_letterbox(size: usize) -> Letterbox {
        Letterbox {
            size,
            scale: 1.0,
            pad_x: 0.0,
            pad_y: 0.0,
        }
    }

    #[test]
    fn maps_model_pixels_onto_the_source_image() {
        let size = 224usize;
        let mut raw = vec![0.0f32; NUM_LANDMARKS * 3];
        // wrist at the middle of the model square, index tip at 3/4 across
        raw[0] = 112.0;
        raw[1] = 112.0;
        raw[8 * 3] = 168.0;
        raw[8 * 3 + 1] = 56.0;

        let hand = decode(
            &raw,
            &[],
            0.9,
            0.8,
            &identity_letterbox(size),
            size,
            size,
            size,
        )
        .unwrap();

        assert!((hand.landmarks[0].x - 0.5).abs() < 1e-5);
        assert!((hand.landmarks[0].y - 0.5).abs() < 1e-5);
        assert!((hand.landmarks[8].x - 0.75).abs() < 1e-5);
        assert!((hand.landmarks[8].y - 0.25).abs() < 1e-5);
        assert!((hand.presence - 0.9).abs() < 1e-6);

        // z keeps x's units: 22.4 model px on a 224 wide frame is 0.1
        let mut deep = vec![0.0f32; NUM_LANDMARKS * 3];
        deep[2] = 22.4;
        let hand = decode(&deep, &[], 1.0, 0.5, &identity_letterbox(size), size, size, size).unwrap();
        assert!((hand.landmarks[0].z - 0.1).abs() < 1e-5, "z was {}", hand.landmarks[0].z);
    }

    #[test]
    fn reads_handedness_from_either_side_of_the_threshold() {
        let raw = vec![0.0f32; NUM_LANDMARKS * 3];
        let lb = identity_letterbox(224);

        let right = decode(&raw, &[], 1.0, 0.93, &lb, 224, 224, 224).unwrap();
        assert_eq!(right.handedness, Handedness::Right);
        assert!((right.handedness_score - 0.93).abs() < 1e-6);

        let left = decode(&raw, &[], 1.0, 0.11, &lb, 224, 224, 224).unwrap();
        assert_eq!(left.handedness, Handedness::Left);
        assert!((left.handedness_score - 0.89).abs() < 1e-6);
    }

    #[test]
    fn keeps_world_landmarks_unscaled() {
        let raw = vec![0.0f32; NUM_LANDMARKS * 3];
        let mut world = vec![0.0f32; NUM_LANDMARKS * 3];
        world[12] = 0.05;
        world[13] = -0.02;
        world[14] = 0.01;

        let hand = decode(&raw, &world, 1.0, 0.5, &identity_letterbox(224), 224, 224, 224).unwrap();
        assert!((hand.world[4].x - 0.05).abs() < 1e-6);
        assert!((hand.world[4].y + 0.02).abs() < 1e-6);
        assert!((hand.world[4].z - 0.01).abs() < 1e-6);
    }

    #[test]
    fn rejects_a_short_landmark_vector() {
        assert!(decode(&[0.0; 10], &[], 1.0, 0.5, &identity_letterbox(224), 224, 8, 8).is_err());
    }

    #[test]
    fn fingertips_are_the_five_tip_indices() {
        let mut raw = vec![0.0f32; NUM_LANDMARKS * 3];
        for (n, tip) in FINGERTIPS.iter().enumerate() {
            raw[tip * 3] = (n as f32 + 1.0) * 20.0;
        }
        let hand = decode(&raw, &[], 1.0, 0.5, &identity_letterbox(224), 224, 224, 224).unwrap();
        let tips = hand.fingertips();
        for (n, tip) in tips.iter().enumerate() {
            assert!((tip.x - (n as f32 + 1.0) * 20.0 / 224.0).abs() < 1e-5);
        }
    }
}
