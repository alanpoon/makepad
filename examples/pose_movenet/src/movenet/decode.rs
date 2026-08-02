//! MoveNet single-pose decoding.
//!
//! The four heads are read on the CPU: pick the person center, regress a
//! coarse position for each of the 17 keypoints from that center, refine each
//! one with a distance-weighted argmax over its heatmap channel, then add the
//! sub-pixel offset at the winning cell.
//!
//! Head tensors arrive in ggml layout `[W, H, C, 1]`, i.e. planar: channel `c`
//! at `(x, y)` lives at `data[(c * H + y) * W + x]`.

/// COCO keypoint order, which is the order MoveNet emits.
pub const KEYPOINT_NAMES: [&str; 17] = [
    "nose",
    "left_eye",
    "right_eye",
    "left_ear",
    "right_ear",
    "left_shoulder",
    "right_shoulder",
    "left_elbow",
    "right_elbow",
    "left_wrist",
    "right_wrist",
    "left_hip",
    "right_hip",
    "left_knee",
    "right_knee",
    "left_ankle",
    "right_ankle",
];

pub const NUM_KEYPOINTS: usize = 17;

/// Bone connections used to draw the skeleton.
pub const SKELETON: [(usize, usize); 18] = [
    (0, 1),
    (0, 2),
    (1, 3),
    (2, 4),
    (0, 5),
    (0, 6),
    (5, 7),
    (7, 9),
    (6, 8),
    (8, 10),
    (5, 6),
    (5, 11),
    (6, 12),
    (11, 12),
    (11, 13),
    (13, 15),
    (12, 14),
    (14, 16),
];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Keypoint {
    /// Normalized to the model input square, 0..1, top-left origin.
    pub y: f32,
    pub x: f32,
    /// Heatmap confidence at the winning cell.
    pub score: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Pose {
    pub keypoints: [Keypoint; NUM_KEYPOINTS],
    /// Score of the selected person center.
    pub center_score: f32,
}

impl Pose {
    pub fn name(index: usize) -> &'static str {
        KEYPOINT_NAMES[index]
    }
}

/// Raw head tensors, planar, all sharing one grid size.
pub struct Heads<'a> {
    pub width: usize,
    pub height: usize,
    /// 17 channels.
    pub heatmap: &'a [f32],
    /// 1 channel.
    pub center: &'a [f32],
    /// 34 channels, (y, x) pairs, grid units, relative to the center cell.
    pub regress: &'a [f32],
    /// 34 channels, (y, x) pairs, grid units, relative to the winning cell.
    pub offset: &'a [f32],
}

/// Weighting constants read out of the published MoveNet graphs.
///
/// The model bakes its center weighting in as a constant `1/(d + 1.8)` map
/// around `grid/2`, and applies `heatmap / (d + 1.8)` when refining each
/// keypoint. Both are reproduced here rather than approximated.
#[derive(Clone, Copy, Debug)]
pub struct DecodeParams {
    /// Added to the distance when weighting the center heatmap, so cells near
    /// the frame center win ties.
    pub center_distance_bias: f32,
    /// Added to the distance when weighting a keypoint heatmap by how far the
    /// cell sits from the regressed position.
    pub keypoint_distance_bias: f32,
}

impl Default for DecodeParams {
    fn default() -> Self {
        Self {
            center_distance_bias: 1.8,
            keypoint_distance_bias: 1.8,
        }
    }
}

impl<'a> Heads<'a> {
    fn cells(&self) -> usize {
        self.width * self.height
    }

    fn at(&self, plane: &[f32], channel: usize, x: usize, y: usize) -> f32 {
        plane[(channel * self.height + y) * self.width + x]
    }

    fn validate(&self) -> Result<(), String> {
        let cells = self.cells();
        if cells == 0 {
            return Err("head grid is empty".to_string());
        }
        for (name, plane, channels) in [
            ("heatmap", self.heatmap, NUM_KEYPOINTS),
            ("center", self.center, 1),
            ("regress", self.regress, NUM_KEYPOINTS * 2),
            ("offset", self.offset, NUM_KEYPOINTS * 2),
        ] {
            if plane.len() != cells * channels {
                return Err(format!(
                    "{name} head holds {} values, expected {} ({}x{}x{})",
                    plane.len(),
                    cells * channels,
                    self.width,
                    self.height,
                    channels
                ));
            }
        }
        Ok(())
    }
}

pub fn decode(heads: &Heads, params: DecodeParams) -> Result<Pose, String> {
    heads.validate()?;

    let (cx, cy, center_score) = find_center(heads, params);

    let mut pose = Pose {
        center_score,
        ..Default::default()
    };

    for joint in 0..NUM_KEYPOINTS {
        // coarse position regressed from the person center, in grid units
        let ty = cy as f32 + heads.at(heads.regress, joint * 2, cx, cy);
        let tx = cx as f32 + heads.at(heads.regress, joint * 2 + 1, cx, cy);

        // refine: the heatmap peak nearest that coarse position wins
        let mut best = f32::NEG_INFINITY;
        let (mut bx, mut by) = (0usize, 0usize);
        for y in 0..heads.height {
            for x in 0..heads.width {
                let dy = y as f32 - ty;
                let dx = x as f32 - tx;
                let distance = (dy * dy + dx * dx).sqrt();
                let weighted = heads.at(heads.heatmap, joint, x, y)
                    / (params.keypoint_distance_bias + distance);
                if weighted > best {
                    best = weighted;
                    bx = x;
                    by = y;
                }
            }
        }

        let oy = heads.at(heads.offset, joint * 2, bx, by);
        let ox = heads.at(heads.offset, joint * 2 + 1, bx, by);
        pose.keypoints[joint] = Keypoint {
            y: (by as f32 + oy) / heads.height as f32,
            x: (bx as f32 + ox) / heads.width as f32,
            score: heads.at(heads.heatmap, joint, bx, by),
        };
    }

    Ok(pose)
}

/// Highest center-heatmap cell, biased towards the middle of the frame so a
/// bystander at the edge does not steal the pose.
fn find_center(heads: &Heads, params: DecodeParams) -> (usize, usize, f32) {
    // the model's baked weight map is centered on grid/2, not (grid-1)/2
    let mid_x = heads.width as f32 * 0.5;
    let mid_y = heads.height as f32 * 0.5;

    let mut best = f32::NEG_INFINITY;
    let (mut bx, mut by) = (0usize, 0usize);
    for y in 0..heads.height {
        for x in 0..heads.width {
            let dy = y as f32 - mid_y;
            let dx = x as f32 - mid_x;
            let distance = (dy * dy + dx * dx).sqrt();
            let weighted =
                heads.at(heads.center, 0, x, y) / (params.center_distance_bias + distance);
            if weighted > best {
                best = weighted;
                bx = x;
                by = y;
            }
        }
    }
    (bx, by, heads.at(heads.center, 0, bx, by))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Planes {
        heatmap: Vec<f32>,
        center: Vec<f32>,
        regress: Vec<f32>,
        offset: Vec<f32>,
    }

    fn planes(w: usize, h: usize) -> Planes {
        Planes {
            heatmap: vec![0.0; w * h * NUM_KEYPOINTS],
            center: vec![0.0; w * h],
            regress: vec![0.0; w * h * NUM_KEYPOINTS * 2],
            offset: vec![0.0; w * h * NUM_KEYPOINTS * 2],
        }
    }

    fn set(plane: &mut [f32], w: usize, h: usize, c: usize, x: usize, y: usize, v: f32) {
        plane[(c * h + y) * w + x] = v;
    }

    #[test]
    fn reads_a_single_peak_with_its_offset() {
        let (w, h) = (8usize, 8usize);
        let mut p = planes(w, h);

        // one person centered at cell (4, 4)
        set(&mut p.center, w, h, 0, 4, 4, 0.9);
        // every joint regresses to (6, 2) and peaks there
        for joint in 0..NUM_KEYPOINTS {
            set(&mut p.regress, w, h, joint * 2, 4, 4, 2.0); // dy: 4 -> 6
            set(&mut p.regress, w, h, joint * 2 + 1, 4, 4, -2.0); // dx: 4 -> 2
            set(&mut p.heatmap, w, h, joint, 2, 6, 0.8);
            set(&mut p.offset, w, h, joint * 2, 2, 6, 0.5);
            set(&mut p.offset, w, h, joint * 2 + 1, 2, 6, 0.25);
        }

        let heads = Heads {
            width: w,
            height: h,
            heatmap: &p.heatmap,
            center: &p.center,
            regress: &p.regress,
            offset: &p.offset,
        };
        let pose = decode(&heads, DecodeParams::default()).unwrap();

        assert!((pose.center_score - 0.9).abs() < 1e-6);
        for kp in pose.keypoints {
            // (6 + 0.5) / 8 and (2 + 0.25) / 8
            assert!((kp.y - 0.8125).abs() < 1e-6, "y was {}", kp.y);
            assert!((kp.x - 0.28125).abs() < 1e-6, "x was {}", kp.x);
            assert!((kp.score - 0.8).abs() < 1e-6);
        }
    }

    #[test]
    fn prefers_the_center_person_over_an_edge_person() {
        let (w, h) = (16usize, 16usize);
        let mut p = planes(w, h);
        // edge candidate scores higher raw, but sits far from the middle
        set(&mut p.center, w, h, 0, 0, 0, 0.80);
        set(&mut p.center, w, h, 0, 8, 8, 0.55);

        let heads = Heads {
            width: w,
            height: h,
            heatmap: &p.heatmap,
            center: &p.center,
            regress: &p.regress,
            offset: &p.offset,
        };
        let (x, y, score) = find_center(&heads, DecodeParams::default());
        assert_eq!((x, y), (8, 8));
        assert!((score - 0.55).abs() < 1e-6);
    }

    #[test]
    fn picks_the_peak_nearest_the_regressed_position() {
        let (w, h) = (16usize, 16usize);
        let mut p = planes(w, h);
        set(&mut p.center, w, h, 0, 8, 8, 1.0);

        // joint 0 regresses to (9, 9); a stronger but distant peak must lose
        set(&mut p.regress, w, h, 0, 8, 8, 1.0);
        set(&mut p.regress, w, h, 1, 8, 8, 1.0);
        set(&mut p.heatmap, w, h, 0, 9, 9, 0.6);
        set(&mut p.heatmap, w, h, 0, 15, 0, 0.9);

        let heads = Heads {
            width: w,
            height: h,
            heatmap: &p.heatmap,
            center: &p.center,
            regress: &p.regress,
            offset: &p.offset,
        };
        let pose = decode(&heads, DecodeParams::default()).unwrap();
        assert!((pose.keypoints[0].y - 9.0 / 16.0).abs() < 1e-6);
        assert!((pose.keypoints[0].x - 9.0 / 16.0).abs() < 1e-6);
        assert!((pose.keypoints[0].score - 0.6).abs() < 1e-6);
    }

    #[test]
    fn rejects_mismatched_head_sizes() {
        let (w, h) = (4usize, 4usize);
        let p = planes(w, h);
        let heads = Heads {
            width: w,
            height: h,
            heatmap: &p.heatmap[..w * h], // one channel instead of 17
            center: &p.center,
            regress: &p.regress,
            offset: &p.offset,
        };
        assert!(decode(&heads, DecodeParams::default()).is_err());
    }
}
