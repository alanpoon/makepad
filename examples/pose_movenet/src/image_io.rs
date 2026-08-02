//! JPEG/PNG decoding to plain RGB8, which is what the estimator wants.

use makepad_zune_core::colorspace::ColorSpace;
use makepad_zune_core::options::DecoderOptions;
use makepad_zune_jpeg::JpegDecoder;
use makepad_zune_png::PngDecoder;
use std::io::Cursor;

pub struct DecodedImage {
    pub width: usize,
    pub height: usize,
    /// `width * height * 3`, row-major.
    pub rgb: Vec<u8>,
}

pub fn decode(bytes: &[u8]) -> Result<DecodedImage, String> {
    if bytes.len() > 8 && bytes[0..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        decode_png(bytes)
    } else if bytes.len() > 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        decode_jpeg(bytes)
    } else {
        Err("unrecognized image: expected a JPEG or PNG".to_string())
    }
}

fn decode_jpeg(bytes: &[u8]) -> Result<DecodedImage, String> {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = JpegDecoder::new_with_options(Cursor::new(bytes), options);
    decoder
        .decode_headers()
        .map_err(|e| format!("jpeg headers unreadable: {e}"))?;
    let info = decoder
        .info()
        .ok_or_else(|| "jpeg has no image info".to_string())?;
    let colorspace = decoder.output_colorspace();
    let pixels = decoder
        .decode()
        .map_err(|e| format!("jpeg decode failed: {e}"))?;
    to_rgb(pixels, info.width as usize, info.height as usize, colorspace)
}

fn decode_png(bytes: &[u8]) -> Result<DecodedImage, String> {
    let options = DecoderOptions::default().png_set_strip_to_8bit(true);
    let mut decoder = PngDecoder::new_with_options(Cursor::new(bytes), options);
    decoder
        .decode_headers()
        .map_err(|e| format!("png headers unreadable: {e}"))?;
    let info = decoder
        .info()
        .cloned()
        .ok_or_else(|| "png has no image info".to_string())?;
    let colorspace = decoder.colorspace();
    let pixels = decoder
        .decode_raw()
        .map_err(|e| format!("png decode failed: {e}"))?;
    to_rgb(pixels, info.width as usize, info.height as usize, colorspace)
}

fn to_rgb(
    pixels: Vec<u8>,
    width: usize,
    height: usize,
    colorspace: Option<ColorSpace>,
) -> Result<DecodedImage, String> {
    let pixel_count = width
        .checked_mul(height)
        .ok_or_else(|| "image dimensions overflow".to_string())?;
    if pixel_count == 0 {
        return Err("image is empty".to_string());
    }
    let channels = match colorspace {
        Some(cs) => cs.num_components(),
        None => pixels.len() / pixel_count,
    };
    if pixels.len() < pixel_count * channels {
        return Err(format!(
            "decoded {} bytes, expected {} for {width}x{height}x{channels}",
            pixels.len(),
            pixel_count * channels
        ));
    }

    let rgb = match channels {
        3 => pixels,
        4 => pixels
            .chunks_exact(4)
            .flat_map(|px| [px[0], px[1], px[2]])
            .collect(),
        1 => pixels.iter().flat_map(|v| [*v, *v, *v]).collect(),
        2 => pixels
            .chunks_exact(2)
            .flat_map(|px| [px[0], px[0], px[0]])
            .collect(),
        other => return Err(format!("unsupported channel count {other}")),
    };

    Ok(DecodedImage {
        width,
        height,
        rgb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_formats() {
        assert!(decode(b"not an image at all").is_err());
    }

    #[test]
    fn expands_gray_and_drops_alpha() {
        let gray = to_rgb(vec![7, 9], 2, 1, Some(ColorSpace::Luma)).unwrap();
        assert_eq!(gray.rgb, vec![7, 7, 7, 9, 9, 9]);

        let rgba = to_rgb(
            vec![1, 2, 3, 255, 4, 5, 6, 128],
            2,
            1,
            Some(ColorSpace::RGBA),
        )
        .unwrap();
        assert_eq!(rgba.rgb, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn rejects_truncated_pixel_data() {
        assert!(to_rgb(vec![1, 2, 3], 4, 4, Some(ColorSpace::RGB)).is_err());
    }
}
