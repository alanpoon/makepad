//! PNG export functionality for captured texture pixels.
//!
//! This module provides functions to save captured pixels to PNG files
//! or encode them to PNG bytes in memory.

use std::io::BufWriter;
use std::path::Path;
use std::fs::File;

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
use crate::makepad_platform::{CapturedPixels, CapturedPixelFormat};

/// Save captured pixels to a PNG file.
///
/// The pixels will be automatically converted from BGRA to RGBA if needed.
///
/// # Arguments
/// * `pixels` - The captured pixel data to save
/// * `path` - The path where the PNG file should be written
///
/// # Returns
/// * `Ok(())` on success
/// * `Err(std::io::Error)` if file creation or PNG encoding fails
///
/// # Example
/// ```ignore
/// if let Some(pixels) = texture.capture_pixels(cx) {
///     save_to_png(&pixels, "screenshot.png").unwrap();
/// }
/// ```
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
pub fn save_to_png(
    pixels: &CapturedPixels,
    path: impl AsRef<Path>,
) -> Result<(), std::io::Error> {
    // Convert to RGBA if needed
    let rgba_pixels = if pixels.format == CapturedPixelFormat::BGRAu8 {
        pixels.to_rgba()
    } else {
        pixels.clone()
    };

    let file = File::create(path)?;
    let writer = BufWriter::new(file);

    let mut encoder = png::Encoder::new(writer, rgba_pixels.width as u32, rgba_pixels.height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut png_writer = encoder.write_header()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    png_writer.write_image_data(&rgba_pixels.data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}

/// Encode captured pixels to PNG bytes in memory.
///
/// The pixels will be automatically converted from BGRA to RGBA if needed.
///
/// # Arguments
/// * `pixels` - The captured pixel data to encode
///
/// # Returns
/// * `Ok(Vec<u8>)` containing the PNG-encoded bytes on success
/// * `Err(std::io::Error)` if PNG encoding fails
///
/// # Example
/// ```ignore
/// if let Some(pixels) = texture.capture_pixels(cx) {
///     let png_bytes = encode_to_png_bytes(&pixels).unwrap();
///     // Use png_bytes for network transfer, clipboard, etc.
/// }
/// ```
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
pub fn encode_to_png_bytes(pixels: &CapturedPixels) -> Result<Vec<u8>, std::io::Error> {
    // Convert to RGBA if needed
    let rgba_pixels = if pixels.format == CapturedPixelFormat::BGRAu8 {
        pixels.to_rgba()
    } else {
        pixels.clone()
    };

    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, rgba_pixels.width as u32, rgba_pixels.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut png_writer = encoder.write_header()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        png_writer.write_image_data(&rgba_pixels.data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    }
    Ok(output)
}
