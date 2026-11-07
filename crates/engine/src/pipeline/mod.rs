mod decode;
mod detect;

pub use decode::{
    decode_image, decode_image_with_format, MAX_IMAGE_ALLOC, MAX_IMAGE_HEIGHT, MAX_IMAGE_PIXELS,
    MAX_IMAGE_WIDTH, MAX_INPUT_FILE_SIZE, MAX_REASONABLE_MEMORY_ESTIMATE,
};
pub use detect::detect_format;

use crate::codec::encode_image_with_options;
use crate::types::{ConversionOptions, ConversionResult, ImageFormat};
use image::{GenericImageView, ImageDecoder, ImageReader};
use jxl_oxide::integration::JxlDecoder;
use std::io::Cursor;

pub fn estimate_memory_usage(data: &[u8]) -> usize {
    let format = detect_format(data);
    let dims = match format {
        ImageFormat::Jxl => JxlDecoder::new(Cursor::new(data))
            .ok()
            .map(|d| d.dimensions()),
        ImageFormat::Avif => oxideav_avif::info(data).ok().map(|i| (i.width, i.height)),
        _ => ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_dimensions().ok()),
    };

    if let Some((w, h)) = dims {
        let pixels = (w as usize).saturating_mul(h as usize);
        data.len().saturating_add(pixels.saturating_mul(16))
    } else {
        data.len().saturating_mul(4)
    }
}

pub fn estimate_memory_from_header(header: &[u8], file_size: usize) -> Option<usize> {
    let format = detect_format(header);
    let dims = match format {
        ImageFormat::Jxl => JxlDecoder::new(Cursor::new(header))
            .ok()
            .map(|d| d.dimensions()),
        ImageFormat::Avif => oxideav_avif::info(header).ok().map(|i| (i.width, i.height)),
        _ => ImageReader::new(Cursor::new(header))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_dimensions().ok()),
    };

    dims.map(|(w, h)| {
        let pixels = (w as usize).saturating_mul(h as usize);
        file_size.saturating_add(pixels.saturating_mul(16))
    })
}

pub fn convert(
    data: &[u8],
    options: &ConversionOptions,
) -> Result<(Vec<u8>, ConversionResult), String> {
    if data.len() > MAX_INPUT_FILE_SIZE {
        return Err(format!(
            "Input file size ({} MB) exceeds maximum allowed limit ({} MB)",
            data.len() / (1024 * 1024),
            MAX_INPUT_FILE_SIZE / (1024 * 1024)
        ));
    }

    let start = std::time::Instant::now();
    let input_size = data.len();

    let input_format = detect_format(data);
    let img = decode::decode_image_with_format(data, input_format)?;

    let (width, height) = img.dimensions();

    let output_format = options.format.unwrap_or(if input_format.is_supported() {
        input_format
    } else {
        ImageFormat::Png
    });

    if !output_format.is_supported() {
        return Err("Unsupported output format".to_string());
    }

    let output = encode_image_with_options(
        &img,
        output_format,
        options.quality,
        &options.jpeg,
        &options.avif,
        &options.png,
        &options.webp,
        &options.jxl,
        &options.gif,
        &options.bmp,
    )?;

    let output_size = output.len();
    let saved_bytes = input_size as i64 - output_size as i64;
    let saved_percent = if input_size > 0 {
        (saved_bytes as f32 / input_size as f32) * 100.0
    } else {
        0.0
    };

    let result = ConversionResult {
        input_format,
        output_format,
        width,
        height,
        input_size_bytes: input_size,
        output_size_bytes: output_size,
        saved_bytes,
        saved_percent,
        duration_ms: start.elapsed().as_millis().max(1) as u32,
    };

    Ok((output, result))
}
