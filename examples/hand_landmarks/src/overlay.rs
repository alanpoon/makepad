//! Draws the hand skeleton into the RGBA pixels that get uploaded as a
//! texture, so image and landmarks are one flat picture.

use crate::hand::{Hand, Handedness, CONNECTIONS, FINGERTIPS};

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

    pub fn disc(&mut self, cx: f32, cy: f32, radius: f32, color: [u8; 3]) {
        let r = radius.max(0.5);
        for y in (cy - r - 1.0).floor() as i64..=(cy + r + 1.0).ceil() as i64 {
            for x in (cx - r - 1.0).floor() as i64..=(cx + r + 1.0).ceil() as i64 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let alpha = (r - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
                if alpha > 0.0 {
                    self.blend(x, y, color, alpha);
                }
            }
        }
    }
}

/// One hue per finger so the chains stay readable when they overlap.
fn finger_color(landmark: usize) -> [u8; 3] {
    match landmark {
        0 => [235, 235, 235],        // wrist
        1..=4 => [255, 122, 92],     // thumb
        5..=8 => [255, 196, 71],     // index
        9..=12 => [126, 217, 87],    // middle
        13..=16 => [86, 180, 255],   // ring
        _ => [201, 137, 255],        // pinky
    }
}

pub fn draw_hand(canvas: &mut Canvas, hand: &Hand) {
    let w = canvas.width as f32;
    let h = canvas.height as f32;
    let scale = (w.min(h) / 512.0).max(1.0);

    for (a, b) in CONNECTIONS {
        let (pa, pb) = (hand.landmarks[a], hand.landmarks[b]);
        canvas.line(
            pa.x * w,
            pa.y * h,
            pb.x * w,
            pb.y * h,
            finger_color(b.max(a)),
            3.0 * scale,
        );
    }

    for (i, lm) in hand.landmarks.iter().enumerate() {
        let tip = FINGERTIPS.contains(&i);
        let radius = if i == 0 {
            5.0
        } else if tip {
            4.5
        } else {
            3.0
        } * scale;
        canvas.disc(lm.x * w, lm.y * h, radius + 1.5 * scale, [18, 20, 26]);
        canvas.disc(lm.x * w, lm.y * h, radius, finger_color(i));
    }
}

/// A left/right tag drawn as a small bar under the wrist: warm for right,
/// cool for left, length scaled by confidence.
pub fn draw_handedness(canvas: &mut Canvas, hand: &Hand) {
    let w = canvas.width as f32;
    let h = canvas.height as f32;
    let wrist = hand.landmarks[0];
    let color = match hand.handedness {
        Handedness::Right => [255, 156, 64],
        Handedness::Left => [86, 180, 255],
    };
    let len = 40.0 * (w.min(h) / 512.0).max(1.0) * hand.handedness_score.clamp(0.0, 1.0);
    let y = wrist.y * h + 14.0;
    canvas.line(wrist.x * w - len * 0.5, y, wrist.x * w + len * 0.5, y, color, 6.0);
}

pub fn rgba8_to_bgra_u32(pixels: &[u8]) -> Vec<u32> {
    pixels
        .chunks_exact(4)
        .map(|px| (px[3] as u32) << 24 | (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hand::{Landmark, NUM_LANDMARKS};

    fn hand_at(x: f32, y: f32) -> Hand {
        Hand {
            landmarks: [Landmark { x, y, z: 0.0 }; NUM_LANDMARKS],
            world: [Landmark::default(); NUM_LANDMARKS],
            presence: 1.0,
            handedness: Handedness::Right,
            handedness_score: 1.0,
        }
    }

    #[test]
    fn draws_inside_the_canvas() {
        let (w, h) = (64usize, 64usize);
        let mut px = vec![0u8; w * h * 4];
        let mut c = Canvas {
            pixels: &mut px,
            width: w,
            height: h,
        };
        draw_hand(&mut c, &hand_at(0.5, 0.5));
        let lit = px.chunks_exact(4).filter(|p| p[0] > 0 || p[1] > 0 || p[2] > 0).count();
        assert!(lit > 0, "nothing was drawn");
        // the corner is far from a hand drawn at the middle
        assert_eq!(px[0], 0);
    }

    #[test]
    fn landmarks_outside_the_frame_do_not_panic() {
        let (w, h) = (32usize, 32usize);
        let mut px = vec![0u8; w * h * 4];
        let mut c = Canvas {
            pixels: &mut px,
            width: w,
            height: h,
        };
        draw_hand(&mut c, &hand_at(-3.0, 5.0));
        draw_handedness(&mut c, &hand_at(-3.0, 5.0));
    }

    #[test]
    fn each_finger_gets_its_own_color() {
        let colors: Vec<[u8; 3]> = [4usize, 8, 12, 16, 20].iter().map(|i| finger_color(*i)).collect();
        for i in 0..colors.len() {
            for j in i + 1..colors.len() {
                assert_ne!(colors[i], colors[j], "fingers {i} and {j} share a color");
            }
        }
    }

    #[test]
    fn packs_bgra_texels() {
        assert_eq!(rgba8_to_bgra_u32(&[10, 20, 30, 255]), vec![0xFF_0A_14_1E]);
    }
}
