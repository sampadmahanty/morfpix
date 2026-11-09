mod decode;
mod detect;

pub use decode::{
    decode_image, decode_image_with_format, validate_dimensions, MAX_IMAGE_ALLOC, MAX_IMAGE_HEIGHT,
    MAX_IMAGE_PIXELS, MAX_IMAGE_WIDTH, MAX_INPUT_FILE_SIZE, MAX_REASONABLE_MEMORY_ESTIMATE,
};
pub use detect::detect_format;

use crate::codec::encode_image_with_options;
use crate::types::{ConversionOptions, ConversionResult, ImageFormat};
use image::{GenericImageView, ImageDecoder, ImageReader};
use jxl_oxide::integration::JxlDecoder;
use std::io::Cursor;

fn is_gif_animated(data: &[u8]) -> bool {
    if data.len() < 13 || (&data[0..6] != b"GIF87a" && &data[0..6] != b"GIF89a") {
        return false;
    }

    let screen_w = u16::from_le_bytes([data[6], data[7]]) as u32;
    let screen_h = u16::from_le_bytes([data[8], data[9]]) as u32;
    if validate_dimensions(screen_w, screen_h).is_err() {
        return false;
    }

    let packed = data[10];
    let gct_present = (packed & 0x80) != 0;
    let gct_entries = if gct_present {
        1usize << ((packed & 0x07) + 1)
    } else {
        0
    };
    let mut offset = 13 + gct_entries * 3;
    if offset > data.len() {
        return false;
    }

    let mut frame_count = 0;

    while offset < data.len() {
        let block_type = data[offset];
        offset += 1;

        match block_type {
            // Trailer ';'
            0x3B => break,
            // Null padding byte
            0x00 => continue,
            // Extension block '!'
            0x21 => {
                if offset >= data.len() {
                    break;
                }
                // Skip extension label
                offset += 1;
                // Skip sub-blocks
                while offset < data.len() {
                    let sub_block_len = data[offset] as usize;
                    offset += 1;
                    if sub_block_len == 0 {
                        break;
                    }
                    offset = offset.saturating_add(sub_block_len);
                    if offset > data.len() {
                        return false;
                    }
                }
            }
            // Image descriptor ','
            0x2C => {
                frame_count += 1;
                if frame_count > 1 {
                    return true;
                }
                // Image descriptor has 9 bytes following 0x2C:
                // Left (2), Top (2), Width (2), Height (2), Packed (1)
                if offset + 9 > data.len() {
                    break;
                }
                let frame_w = u16::from_le_bytes([data[offset + 4], data[offset + 5]]) as u32;
                let frame_h = u16::from_le_bytes([data[offset + 6], data[offset + 7]]) as u32;
                if validate_dimensions(frame_w, frame_h).is_err() {
                    return false;
                }

                let img_packed = data[offset + 8];
                offset += 9;

                let lct_present = (img_packed & 0x80) != 0;
                if lct_present {
                    let lct_entries = 1usize << ((img_packed & 0x07) + 1);
                    offset = offset.saturating_add(lct_entries * 3);
                    if offset > data.len() {
                        return false;
                    }
                }

                // Skip LZW minimum code size
                if offset >= data.len() {
                    break;
                }
                offset += 1;

                // Skip image data sub-blocks
                while offset < data.len() {
                    let sub_block_len = data[offset] as usize;
                    offset += 1;
                    if sub_block_len == 0 {
                        break;
                    }
                    offset = offset.saturating_add(sub_block_len);
                    if offset > data.len() {
                        return false;
                    }
                }
            }
            _ => {
                // Unknown block type
                break;
            }
        }
    }

    false
}

pub fn is_animated(data: &[u8], format: ImageFormat) -> bool {
    let resolved_format = if format.is_supported() {
        format
    } else {
        detect_format(data)
    };

    match resolved_format {
        ImageFormat::Gif => is_gif_animated(data),
        ImageFormat::WebP => {
            if let Ok(decoder) = image::codecs::webp::WebPDecoder::new(Cursor::new(data)) {
                decoder.has_animation()
            } else {
                false
            }
        }
        ImageFormat::Png => {
            if let Ok(decoder) = image::codecs::png::PngDecoder::new(Cursor::new(data)) {
                decoder.is_apng().unwrap_or(false)
            } else {
                false
            }
        }
        ImageFormat::Avif => {
            if let Ok(info) = oxideav_avif::info(data) {
                info.frames > 1
            } else {
                false
            }
        }
        ImageFormat::Jxl => {
            if let Ok(image) = jxl_oxide::JxlImage::builder().read(Cursor::new(data)) {
                image.image_header().metadata.animation.is_some()
            } else {
                false
            }
        }
        _ => false,
    }
}

pub fn calculate_resize_dimensions(
    orig_width: u32,
    orig_height: u32,
    target_width: Option<u32>,
    target_height: Option<u32>,
) -> Result<(u32, u32), String> {
    if orig_width == 0 || orig_height == 0 {
        return Err("Input image dimensions cannot be zero".to_string());
    }

    match (target_width, target_height) {
        (None, None) => {
            Err("Resize requires at least one dimension (--width or --height)".to_string())
        }
        (Some(0), _) | (_, Some(0)) => Err("Resize dimensions must be greater than 0".to_string()),
        (Some(w), None) => {
            if w > MAX_IMAGE_WIDTH {
                return Err(format!(
                    "Resize width ({}) exceeds maximum allowed limit ({} pixels)",
                    w, MAX_IMAGE_WIDTH
                ));
            }
            let scaled_h = ((orig_height as f64 * w as f64) / orig_width as f64)
                .round()
                .max(1.0) as u32;
            validate_dimensions(w, scaled_h)?;
            Ok((w, scaled_h))
        }
        (None, Some(h)) => {
            if h > MAX_IMAGE_HEIGHT {
                return Err(format!(
                    "Resize height ({}) exceeds maximum allowed limit ({} pixels)",
                    h, MAX_IMAGE_HEIGHT
                ));
            }
            let scaled_w = ((orig_width as f64 * h as f64) / orig_height as f64)
                .round()
                .max(1.0) as u32;
            validate_dimensions(scaled_w, h)?;
            Ok((scaled_w, h))
        }
        (Some(box_w), Some(box_h)) => {
            if box_w > MAX_IMAGE_WIDTH {
                return Err(format!(
                    "Resize width ({}) exceeds maximum allowed limit ({} pixels)",
                    box_w, MAX_IMAGE_WIDTH
                ));
            }
            if box_h > MAX_IMAGE_HEIGHT {
                return Err(format!(
                    "Resize height ({}) exceeds maximum allowed limit ({} pixels)",
                    box_h, MAX_IMAGE_HEIGHT
                ));
            }
            let pixels = box_w as u64 * box_h as u64;
            if pixels > MAX_IMAGE_PIXELS {
                return Err(format!(
                    "Resize bounding box ({}x{} = {} pixels) exceeds maximum allowed limit ({} pixels)",
                    box_w, box_h, pixels, MAX_IMAGE_PIXELS
                ));
            }

            let scale_w = box_w as f64 / orig_width as f64;
            let scale_h = box_h as f64 / orig_height as f64;
            let scale = scale_w.min(scale_h);

            let new_w = ((orig_width as f64 * scale).round() as u32).clamp(1, box_w);
            let new_h = ((orig_height as f64 * scale).round() as u32).clamp(1, box_h);

            validate_dimensions(new_w, new_h)?;
            Ok((new_w, new_h))
        }
    }
}

pub fn read_image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let format = detect_format(data);
    match format {
        ImageFormat::Jxl => JxlDecoder::new(Cursor::new(data))
            .ok()
            .map(|d| d.dimensions()),
        ImageFormat::Avif => oxideav_avif::info(data).ok().map(|i| (i.width, i.height)),
        _ => ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_dimensions().ok()),
    }
}

pub fn estimate_memory_usage(data: &[u8]) -> usize {
    if let Some((w, h)) = read_image_dimensions(data) {
        let pixels = (w as usize).saturating_mul(h as usize);
        data.len().saturating_add(pixels.saturating_mul(16))
    } else {
        data.len().saturating_mul(4)
    }
}

pub fn estimate_memory_usage_with_options(data: &[u8], options: &ConversionOptions) -> usize {
    let mut estimate = estimate_memory_usage(data);
    if let Some(ref resize_opts) = options.resize {
        if let Some((w, h)) = read_image_dimensions(data) {
            if let Ok((target_w, target_h)) =
                calculate_resize_dimensions(w, h, resize_opts.width, resize_opts.height)
            {
                let target_pixels = (target_w as usize).saturating_mul(target_h as usize);
                let target_mem = target_pixels.saturating_mul(16);
                estimate = estimate.saturating_add(target_mem);
            }
        }
    }
    estimate
}

pub fn estimate_memory_from_header(header: &[u8], file_size: usize) -> Option<usize> {
    read_image_dimensions(header).map(|(w, h)| {
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

    if options.resize.is_some() && is_animated(data, input_format) {
        return Err(
            "Resizing animated images is unsupported by the current architecture; cannot resize without dropping frames".to_string(),
        );
    }

    let mut img = decode::decode_image_with_format(data, input_format)?;

    if let Some(ref resize_opts) = options.resize {
        let (orig_w, orig_h) = img.dimensions();
        let (target_w, target_h) =
            calculate_resize_dimensions(orig_w, orig_h, resize_opts.width, resize_opts.height)?;

        if target_w != orig_w || target_h != orig_h {
            img = img.resize_exact(target_w, target_h, resize_opts.filter.to_filter_type());
        }
    }

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
