//! Fits an arbitrary RGB image into the square model input.
//!
//! MoveNet expects a square frame, so the image is scaled to fit and centered
//! on a padded canvas. The [`Letterbox`] returned records that transform so
//! decoded keypoints can be mapped back onto the original pixels.

/// The transform applied to get from source pixels into model input space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Letterbox {
    /// Model input edge length in pixels.
    pub size: usize,
    /// Source pixels are multiplied by this to reach model space.
    pub scale: f32,
    /// Left/top padding inside the model square, in model pixels.
    pub pad_x: f32,
    pub pad_y: f32,
}

impl Letterbox {
    /// Map a normalized model-space coordinate (0..1) back to source pixels.
    pub fn to_source(&self, x: f32, y: f32) -> (f32, f32) {
        let mx = x * self.size as f32;
        let my = y * self.size as f32;
        ((mx - self.pad_x) / self.scale, (my - self.pad_y) / self.scale)
    }

    /// Map a normalized model-space coordinate to normalized source coords.
    pub fn to_source_normalized(
        &self,
        x: f32,
        y: f32,
        source_width: usize,
        source_height: usize,
    ) -> (f32, f32) {
        let (sx, sy) = self.to_source(x, y);
        (sx / source_width as f32, sy / source_height as f32)
    }
}

/// Scale and pad `rgb` (8-bit, 3 channels, row-major) into a planar CHW f32
/// buffer of `size * size * 3`, ready to be written into the graph input.
pub fn letterbox_rgb8(
    rgb: &[u8],
    width: usize,
    height: usize,
    size: usize,
    scale_value: f32,
    bias_value: f32,
) -> Result<(Vec<f32>, Letterbox), String> {
    if width == 0 || height == 0 {
        return Err("source image is empty".to_string());
    }
    if rgb.len() < width * height * 3 {
        return Err(format!(
            "source image holds {} bytes, expected at least {}",
            rgb.len(),
            width * height * 3
        ));
    }
    if size == 0 {
        return Err("model input size is zero".to_string());
    }

    let scale = (size as f32 / width as f32).min(size as f32 / height as f32);
    let scaled_w = width as f32 * scale;
    let scaled_h = height as f32 * scale;
    let pad_x = (size as f32 - scaled_w) * 0.5;
    let pad_y = (size as f32 - scaled_h) * 0.5;

    let mut out = vec![0.0f32; size * size * 3];
    let plane = size * size;

    // When minifying, one destination pixel covers many source pixels, so
    // point-sampling aliases badly (a 1200px photo into a 192px square throws
    // away 97% of the pixels). Average over the source footprint instead --
    // this is what PIL/TensorFlow do, and the model is trained on that.
    let footprint = (1.0 / scale).max(1.0);
    let half = (footprint * 0.5).floor() as i32;

    for my in 0..size {
        // center of the destination pixel, mapped back into source pixels
        let sy = (my as f32 + 0.5 - pad_y) / scale - 0.5;
        for mx in 0..size {
            let sx = (mx as f32 + 0.5 - pad_x) / scale - 0.5;
            if sx < -0.5 || sy < -0.5 || sx > width as f32 - 0.5 || sy > height as f32 - 0.5 {
                continue; // padding stays at the neutral value
            }
            let (r, g, b) = if half > 0 {
                sample_box(rgb, width, height, sx, sy, half)
            } else {
                sample_bilinear(rgb, width, height, sx, sy)
            };
            let dst = my * size + mx;
            out[dst] = r * scale_value + bias_value;
            out[plane + dst] = g * scale_value + bias_value;
            out[2 * plane + dst] = b * scale_value + bias_value;
        }
    }

    Ok((
        out,
        Letterbox {
            size,
            scale,
            pad_x,
            pad_y,
        },
    ))
}

/// Mean of the source pixels covering one destination pixel.
fn sample_box(
    rgb: &[u8],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    half: i32,
) -> (f32, f32, f32) {
    let cx = x.round() as i32;
    let cy = y.round() as i32;
    let (mut r, mut g, mut b) = (0.0f32, 0.0f32, 0.0f32);
    let mut n = 0.0f32;
    for dy in -half..=half {
        let py = (cy + dy).clamp(0, height as i32 - 1) as usize;
        for dx in -half..=half {
            let px = (cx + dx).clamp(0, width as i32 - 1) as usize;
            let i = (py * width + px) * 3;
            r += rgb[i] as f32;
            g += rgb[i + 1] as f32;
            b += rgb[i + 2] as f32;
            n += 1.0;
        }
    }
    (r / n / 255.0, g / n / 255.0, b / n / 255.0)
}

fn sample_bilinear(rgb: &[u8], width: usize, height: usize, x: f32, y: f32) -> (f32, f32, f32) {
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;

    let clamp = |v: f32, max: usize| -> usize { v.max(0.0).min(max as f32 - 1.0) as usize };
    let x0i = clamp(x0, width);
    let y0i = clamp(y0, height);
    let x1i = clamp(x0 + 1.0, width);
    let y1i = clamp(y0 + 1.0, height);

    let texel = |xi: usize, yi: usize| -> (f32, f32, f32) {
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

    #[test]
    fn square_input_needs_no_padding() {
        let rgb = vec![255u8; 4 * 4 * 3];
        let (planes, lb) = letterbox_rgb8(&rgb, 4, 4, 4, 1.0, 0.0).unwrap();
        assert_eq!(lb.scale, 1.0);
        assert_eq!((lb.pad_x, lb.pad_y), (0.0, 0.0));
        assert_eq!(planes.len(), 4 * 4 * 3);
        assert!(planes.iter().all(|v| (*v - 1.0).abs() < 1e-6));
    }

    #[test]
    fn wide_input_is_padded_top_and_bottom() {
        // 8x4 source into a 8x8 model square: scale 1, 2px of padding each side
        let rgb = vec![128u8; 8 * 4 * 3];
        let (planes, lb) = letterbox_rgb8(&rgb, 8, 4, 8, 1.0, 0.0).unwrap();
        assert_eq!(lb.scale, 1.0);
        assert_eq!(lb.pad_y, 2.0);
        assert_eq!(lb.pad_x, 0.0);

        // top row is padding, middle row is image
        assert_eq!(planes[0], 0.0);
        assert!((planes[8 * 3] - 128.0 / 255.0).abs() < 1e-3);
    }

    #[test]
    fn maps_model_coordinates_back_through_the_padding() {
        let rgb = vec![0u8; 8 * 4 * 3];
        let (_, lb) = letterbox_rgb8(&rgb, 8, 4, 8, 1.0, 0.0).unwrap();

        // model-space center is the source center
        let (sx, sy) = lb.to_source(0.5, 0.5);
        assert!((sx - 4.0).abs() < 1e-4);
        assert!((sy - 2.0).abs() < 1e-4);

        // the top of the image content sits below the padding
        let (_, top) = lb.to_source(0.5, 2.0 / 8.0);
        assert!((top - 0.0).abs() < 1e-4);

        let (nx, ny) = lb.to_source_normalized(0.5, 0.5, 8, 4);
        assert!((nx - 0.5).abs() < 1e-4);
        assert!((ny - 0.5).abs() < 1e-4);
    }

    #[test]
    fn scales_pixel_values() {
        let rgb = vec![255u8; 3];
        let (planes, _) = letterbox_rgb8(&rgb, 1, 1, 1, 2.0, -1.0).unwrap();
        assert!((planes[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rejects_short_buffers() {
        assert!(letterbox_rgb8(&[0, 0], 4, 4, 4, 1.0, 0.0).is_err());
    }
}
