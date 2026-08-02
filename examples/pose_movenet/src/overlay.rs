//! Draws the skeleton straight into the RGBA pixels that get uploaded as a
//! texture, so the result is one flat image with no overlay widget to keep in
//! sync with the picture underneath.

use crate::movenet::{Pose, SKELETON};

/// A mutable RGBA8 canvas, row-major.
pub struct Canvas<'a> {
    pub pixels: &'a mut [u8],
    pub width: usize,
    pub height: usize,
}

impl<'a> Canvas<'a> {
    pub fn blend(&mut self, x: i64, y: i64, color: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }
        let alpha = alpha.clamp(0.0, 1.0);
        let i = (y as usize * self.width + x as usize) * 4;
        for c in 0..3 {
            let dst = self.pixels[i + c] as f32;
            let src = color[c] as f32;
            self.pixels[i + c] = (dst + (src - dst) * alpha).round().clamp(0.0, 255.0) as u8;
        }
        self.pixels[i + 3] = 255;
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: [u8; 3], thickness: f32) {
        let steps = ((x1 - x0).abs().max((y1 - y0).abs()) * 2.0).ceil().max(1.0) as usize;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            self.disc(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, thickness * 0.5, color);
        }
    }

    /// Anti-aliased filled circle; also used to stamp thick lines.
    pub fn disc(&mut self, cx: f32, cy: f32, radius: f32, color: [u8; 3]) {
        let r = radius.max(0.5);
        let min_x = (cx - r - 1.0).floor() as i64;
        let max_x = (cx + r + 1.0).ceil() as i64;
        let min_y = (cy - r - 1.0).floor() as i64;
        let max_y = (cy + r + 1.0).ceil() as i64;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let d = (dx * dx + dy * dy).sqrt();
                // one pixel of feathering at the rim
                let alpha = (r - d + 0.5).clamp(0.0, 1.0);
                if alpha > 0.0 {
                    self.blend(x, y, color, alpha);
                }
            }
        }
    }
}

/// Skeleton colors: warm for the right side, cool for the left, so a mirrored
/// pose is obvious at a glance.
fn bone_color(a: usize, b: usize) -> [u8; 3] {
    let right = |i: usize| matches!(i, 2 | 4 | 6 | 8 | 10 | 12 | 14 | 16);
    let left = |i: usize| matches!(i, 1 | 3 | 5 | 7 | 9 | 11 | 13 | 15);
    if right(a) && right(b) {
        [255, 156, 64]
    } else if left(a) && left(b) {
        [86, 180, 255]
    } else {
        [200, 220, 240]
    }
}

/// Draw `pose` (normalized coordinates) onto the canvas. Joints scoring below
/// `min_score` are skipped, along with any bone that touches them.
pub fn draw_pose(canvas: &mut Canvas, pose: &Pose, min_score: f32) {
    let w = canvas.width as f32;
    let h = canvas.height as f32;
    let scale = (w.min(h) / 256.0).max(1.0);

    for (a, b) in SKELETON {
        let ka = pose.keypoints[a];
        let kb = pose.keypoints[b];
        if ka.score < min_score || kb.score < min_score {
            continue;
        }
        canvas.line(
            ka.x * w,
            ka.y * h,
            kb.x * w,
            kb.y * h,
            bone_color(a, b),
            3.0 * scale,
        );
    }

    for kp in pose.keypoints.iter() {
        if kp.score < min_score {
            continue;
        }
        // dark halo first so joints stay readable on light backgrounds
        canvas.disc(kp.x * w, kp.y * h, 4.0 * scale, [20, 24, 32]);
        canvas.disc(kp.x * w, kp.y * h, 2.5 * scale, [255, 255, 255]);
    }
}

/// Pack RGBA8 into the BGRA u32 texels a makepad texture wants.
pub fn rgba8_to_bgra_u32(pixels: &[u8]) -> Vec<u32> {
    pixels
        .chunks_exact(4)
        .map(|px| {
            (px[3] as u32) << 24 | (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movenet::Keypoint;

    fn canvas(w: usize, h: usize) -> Vec<u8> {
        vec![0u8; w * h * 4]
    }

    #[test]
    fn disc_marks_its_center_and_leaves_the_corner_alone() {
        let (w, h) = (16, 16);
        let mut px = canvas(w, h);
        let mut c = Canvas {
            pixels: &mut px,
            width: w,
            height: h,
        };
        c.disc(8.0, 8.0, 3.0, [255, 255, 255]);

        let at = |x: usize, y: usize| px[(y * w + x) * 4];
        assert_eq!(at(8, 8), 255);
        assert_eq!(at(0, 0), 0);
    }

    #[test]
    fn line_touches_both_ends() {
        let (w, h) = (32, 32);
        let mut px = canvas(w, h);
        let mut c = Canvas {
            pixels: &mut px,
            width: w,
            height: h,
        };
        c.line(4.0, 4.0, 28.0, 28.0, [255, 0, 0], 2.0);

        let red = |x: usize, y: usize| px[(y * w + x) * 4];
        assert!(red(4, 4) > 0);
        assert!(red(16, 16) > 0);
        assert!(red(27, 27) > 0);
        assert_eq!(red(28, 4), 0);
    }

    #[test]
    fn writes_are_clipped_to_the_canvas() {
        let (w, h) = (8, 8);
        let mut px = canvas(w, h);
        let mut c = Canvas {
            pixels: &mut px,
            width: w,
            height: h,
        };
        // entirely outside; must not panic or wrap around
        c.disc(-20.0, -20.0, 3.0, [255, 255, 255]);
        c.disc(100.0, 100.0, 3.0, [255, 255, 255]);
        assert!(px.iter().all(|v| *v == 0));
    }

    #[test]
    fn low_scoring_joints_are_not_drawn() {
        let (w, h) = (32, 32);
        let mut px = canvas(w, h);
        let mut pose = Pose::default();
        for kp in pose.keypoints.iter_mut() {
            *kp = Keypoint {
                x: 0.5,
                y: 0.5,
                score: 0.05,
            };
        }
        let mut c = Canvas {
            pixels: &mut px,
            width: w,
            height: h,
        };
        draw_pose(&mut c, &pose, 0.2);
        assert!(px.iter().all(|v| *v == 0));

        for kp in pose.keypoints.iter_mut() {
            kp.score = 0.9;
        }
        let mut c = Canvas {
            pixels: &mut px,
            width: w,
            height: h,
        };
        draw_pose(&mut c, &pose, 0.2);
        assert!(px.iter().any(|v| *v > 0));
    }

    #[test]
    fn packs_bgra_texels() {
        let px = [10u8, 20, 30, 255];
        let packed = rgba8_to_bgra_u32(&px);
        assert_eq!(packed, vec![0xFF_0A_14_1E]);
    }
}
