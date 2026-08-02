//! The rotated square crop that sits between palm detection and the landmark
//! model.
//!
//! MediaPipe orients the crop so the hand points "up": the vector from the
//! wrist keypoint to the middle-finger knuckle is rotated onto the -y axis,
//! the box is squared off on its long side, expanded 2.6x, and shifted along
//! the hand's own axis. Feeding the landmark model an upright, tight hand is
//! what lifts its presence score from ~0.3 to >0.95.

use crate::palm::{Palm, Point, MIDDLE_KNUCKLE_KEYPOINT, WRIST_KEYPOINT};

/// MediaPipe's `RectTransformationCalculator` settings for hand landmarks.
const SCALE: f32 = 2.6;
const SHIFT_Y: f32 = -0.5;

/// A rotated square in normalized image coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotatedRect {
    pub center: Point,
    /// Side length, as a fraction of image width for x and height for y — the
    /// rect is square in *pixels*, not in normalized units.
    pub size_px: f32,
    /// Clockwise rotation in radians, 0 meaning the hand points straight up.
    pub rotation: f32,
}

/// Build the landmark-model ROI for one detection.
pub fn roi_for(palm: &Palm, image_width: usize, image_height: usize) -> RotatedRect {
    let (iw, ih) = (image_width as f32, image_height as f32);

    // rotation that puts the wrist -> knuckle vector along -y
    let wrist = palm.keypoints[WRIST_KEYPOINT];
    let knuckle = palm.keypoints[MIDDLE_KNUCKLE_KEYPOINT];
    let dx = (knuckle.x - wrist.x) * iw;
    let dy = (knuckle.y - wrist.y) * ih;
    let rotation = std::f32::consts::FRAC_PI_2 - (-dy).atan2(dx);

    // square off on the long side, in pixels
    let w_px = palm.width * iw;
    let h_px = palm.height * ih;
    let long_px = w_px.max(h_px);
    let size_px = long_px * SCALE;

    // shift the center along the rect's own axes, then back to normalized
    let (sin_r, cos_r) = rotation.sin_cos();
    let shift_px = long_px * SHIFT_Y;
    let cx_px = palm.center.x * iw - shift_px * sin_r;
    let cy_px = palm.center.y * ih + shift_px * cos_r;

    RotatedRect {
        center: Point {
            x: cx_px / iw,
            y: cy_px / ih,
        },
        size_px,
        rotation,
    }
}

/// Map a point inside the crop (normalized 0..1, origin top-left) back to
/// normalized source-image coordinates.
pub fn crop_to_source(
    rect: &RotatedRect,
    x: f32,
    y: f32,
    image_width: usize,
    image_height: usize,
) -> Point {
    let (iw, ih) = (image_width as f32, image_height as f32);
    let (sin_r, cos_r) = rect.rotation.sin_cos();
    // crop-local offsets in pixels, measured from the rect center
    let lx = (x - 0.5) * rect.size_px;
    let ly = (y - 0.5) * rect.size_px;
    let px = rect.center.x * iw + lx * cos_r - ly * sin_r;
    let py = rect.center.y * ih + lx * sin_r + ly * cos_r;
    Point {
        x: px / iw,
        y: py / ih,
    }
}

/// Sample the rotated crop into a planar CHW f32 buffer of `size*size*3`,
/// which is what the landmark model's graph input expects. Pixels outside the
/// image read as black, matching the padding MediaPipe uses.
pub fn crop_rgb8(
    rgb: &[u8],
    width: usize,
    height: usize,
    rect: &RotatedRect,
    size: usize,
    scale_value: f32,
    bias_value: f32,
) -> Result<Vec<f32>, String> {
    if width == 0 || height == 0 || size == 0 {
        return Err("crop source or target is empty".to_string());
    }
    if rgb.len() < width * height * 3 {
        return Err(format!(
            "source holds {} bytes, expected {}",
            rgb.len(),
            width * height * 3
        ));
    }

    let mut out = vec![0.0f32; size * size * 3];
    let plane = size * size;
    let (sin_r, cos_r) = rect.rotation.sin_cos();
    let cx = rect.center.x * width as f32;
    let cy = rect.center.y * height as f32;
    let step = rect.size_px / size as f32;

    for v in 0..size {
        let ly = (v as f32 + 0.5 - size as f32 * 0.5) * step;
        for u in 0..size {
            let lx = (u as f32 + 0.5 - size as f32 * 0.5) * step;
            let sx = cx + lx * cos_r - ly * sin_r;
            let sy = cy + lx * sin_r + ly * cos_r;
            if sx < -0.5 || sy < -0.5 || sx > width as f32 - 0.5 || sy > height as f32 - 0.5 {
                continue;
            }
            let (r, g, b) = sample(rgb, width, height, sx, sy);
            let dst = v * size + u;
            out[dst] = r * scale_value + bias_value;
            out[plane + dst] = g * scale_value + bias_value;
            out[2 * plane + dst] = b * scale_value + bias_value;
        }
    }
    Ok(out)
}

fn sample(rgb: &[u8], width: usize, height: usize, x: f32, y: f32) -> (f32, f32, f32) {
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;
    let clamp = |v: f32, max: usize| -> usize { v.max(0.0).min(max as f32 - 1.0) as usize };
    let (x0i, y0i) = (clamp(x0, width), clamp(y0, height));
    let (x1i, y1i) = (clamp(x0 + 1.0, width), clamp(y0 + 1.0, height));
    let texel = |xi: usize, yi: usize| {
        let i = (yi * width + xi) * 3;
        (
            rgb[i] as f32 / 255.0,
            rgb[i + 1] as f32 / 255.0,
            rgb[i + 2] as f32 / 255.0,
        )
    };
    let (r00, g00, b00) = texel(x0i, y0i);
    let (r10, g10, b10) = texel(x1i, y0i);
    let (r01, g01, b01) = texel(x0i, y1i);
    let (r11, g11, b11) = texel(x1i, y1i);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    (
        lerp(lerp(r00, r10, fx), lerp(r01, r11, fx), fy),
        lerp(lerp(g00, g10, fx), lerp(g01, g11, fx), fy),
        lerp(lerp(b00, b10, fx), lerp(b01, b11, fx), fy),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palm::NUM_PALM_KEYPOINTS;

    fn palm_pointing_up() -> Palm {
        let mut keypoints = [Point::default(); NUM_PALM_KEYPOINTS];
        keypoints[WRIST_KEYPOINT] = Point { x: 0.5, y: 0.6 };
        keypoints[MIDDLE_KNUCKLE_KEYPOINT] = Point { x: 0.5, y: 0.4 };
        Palm {
            score: 0.9,
            center: Point { x: 0.5, y: 0.5 },
            width: 0.2,
            height: 0.2,
            keypoints,
        }
    }

    #[test]
    fn an_upright_hand_needs_no_rotation() {
        let palm = palm_pointing_up();
        let rect = roi_for(&palm, 200, 200);
        assert!(rect.rotation.abs() < 1e-5, "rotation was {}", rect.rotation);
        // 0.2 of 200px, squared and scaled by 2.6
        assert!((rect.size_px - 0.2 * 200.0 * SCALE).abs() < 1e-3);
        // shifted "up" the hand, which is -y here
        assert!(rect.center.y < palm.center.y);
    }

    #[test]
    fn a_sideways_hand_rotates_a_quarter_turn() {
        let mut palm = palm_pointing_up();
        // knuckle to the right of the wrist: the hand points +x
        palm.keypoints[WRIST_KEYPOINT] = Point { x: 0.4, y: 0.5 };
        palm.keypoints[MIDDLE_KNUCKLE_KEYPOINT] = Point { x: 0.6, y: 0.5 };
        let rect = roi_for(&palm, 200, 200);
        assert!(
            (rect.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5,
            "rotation was {}",
            rect.rotation
        );
        // the shift now moves the center along +x
        assert!(rect.center.x > palm.center.x);
    }

    #[test]
    fn crop_coordinates_round_trip_through_the_rotation() {
        let palm = palm_pointing_up();
        let rect = roi_for(&palm, 300, 300);
        // the crop center is the rect center whatever the rotation
        let c = crop_to_source(&rect, 0.5, 0.5, 300, 300);
        assert!((c.x - rect.center.x).abs() < 1e-6);
        assert!((c.y - rect.center.y).abs() < 1e-6);

        // a point up the crop is up the image when the hand is upright
        let up = crop_to_source(&rect, 0.5, 0.0, 300, 300);
        assert!(up.y < c.y);
    }

    #[test]
    fn rotated_crops_map_back_along_the_rotated_axis() {
        let mut palm = palm_pointing_up();
        palm.keypoints[WRIST_KEYPOINT] = Point { x: 0.4, y: 0.5 };
        palm.keypoints[MIDDLE_KNUCKLE_KEYPOINT] = Point { x: 0.6, y: 0.5 };
        let rect = roi_for(&palm, 300, 300);

        // "up" in the crop is +x in the image for a hand pointing right
        let up = crop_to_source(&rect, 0.5, 0.0, 300, 300);
        assert!(up.x > rect.center.x, "up.x {} center {}", up.x, rect.center.x);
        assert!((up.y - rect.center.y).abs() < 1e-5);
    }

    #[test]
    fn crop_samples_the_expected_region() {
        // a 4x4 image, white in the top-left quadrant only
        let (w, h) = (4usize, 4usize);
        let mut rgb = vec![0u8; w * h * 3];
        for y in 0..2 {
            for x in 0..2 {
                let i = (y * w + x) * 3;
                rgb[i] = 255;
                rgb[i + 1] = 255;
                rgb[i + 2] = 255;
            }
        }
        let rect = RotatedRect {
            center: Point { x: 0.25, y: 0.25 },
            size_px: 2.0,
            rotation: 0.0,
        };
        let planes = crop_rgb8(&rgb, w, h, &rect, 2, 1.0, 0.0).unwrap();
        assert_eq!(planes.len(), 2 * 2 * 3);
        // the rect is centered on the quadrant corner, so the crop's own
        // top-left samples deep inside the white square and the bottom-right
        // straddles three black neighbours
        assert!(planes[0] > 0.9, "top-left was {}", planes[0]);
        assert!(planes[1] < 0.6 && planes[2] < 0.6);
        assert!(planes[3] < 0.3, "bottom-right was {}", planes[3]);
    }

    #[test]
    fn crop_pads_outside_the_image() {
        let (w, h) = (4usize, 4usize);
        let rgb = vec![255u8; w * h * 3];
        let rect = RotatedRect {
            center: Point { x: -1.0, y: -1.0 },
            size_px: 2.0,
            rotation: 0.0,
        };
        let planes = crop_rgb8(&rgb, w, h, &rect, 4, 1.0, 0.0).unwrap();
        assert!(planes.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn rejects_a_short_source() {
        let rect = RotatedRect {
            center: Point { x: 0.5, y: 0.5 },
            size_px: 2.0,
            rotation: 0.0,
        };
        assert!(crop_rgb8(&[0, 0, 0], 4, 4, &rect, 2, 1.0, 0.0).is_err());
    }
}
