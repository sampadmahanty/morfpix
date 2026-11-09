use crate::types::ImageFormat;
use image::metadata::Orientation;
use image::{DynamicImage, ImageBuffer, ImageDecoder, ImageReader};
use jxl_oxide::integration::JxlDecoder;
use oxideav_avif::DecodeOptions;
use qcms::{DataType, Intent, Profile, Transform};
use std::io::Cursor;

pub const MAX_IMAGE_WIDTH: u32 = 16_384;
pub const MAX_IMAGE_HEIGHT: u32 = 16_384;
pub const MAX_IMAGE_PIXELS: u64 = 100_000_000;
pub const MAX_IMAGE_ALLOC: u64 = 256 * 1024 * 1024;
pub const MAX_INPUT_FILE_SIZE: usize = 256 * 1024 * 1024;
pub const MAX_REASONABLE_MEMORY_ESTIMATE: usize = 2 * 1024 * 1024 * 1024; // 2 GiB

pub fn validate_dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("Image dimensions cannot be zero".to_string());
    }
    if width > MAX_IMAGE_WIDTH || height > MAX_IMAGE_HEIGHT {
        return Err(format!(
            "Image dimensions ({}x{}) exceed maximum dimension limit ({}x{})",
            width, height, MAX_IMAGE_WIDTH, MAX_IMAGE_HEIGHT
        ));
    }
    let pixels = width as u64 * height as u64;
    if pixels > MAX_IMAGE_PIXELS {
        return Err(format!(
            "Image pixel count ({} pixels) exceeds maximum allowed limit ({} pixels)",
            pixels, MAX_IMAGE_PIXELS
        ));
    }
    Ok(())
}

pub fn decode_image(data: &[u8]) -> Result<DynamicImage, String> {
    let format = super::detect::detect_format(data);
    decode_image_with_format(data, format)
}

pub fn decode_image_with_format(data: &[u8], format: ImageFormat) -> Result<DynamicImage, String> {
    let (mut img, orientation, icc_profile) = match format {
        ImageFormat::Jxl => decode_jxl(data)?,
        ImageFormat::Avif => decode_avif(data)?,
        _ => decode_general(data, format)?,
    };

    img.apply_orientation(orientation);

    if let Some(icc_profile) = icc_profile {
        img = convert_to_srgb(img, &icc_profile);
    }

    Ok(img)
}

fn decode_general(
    data: &[u8],
    format: ImageFormat,
) -> Result<(DynamicImage, Orientation, Option<Vec<u8>>), String> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_WIDTH);
    limits.max_image_height = Some(MAX_IMAGE_HEIGHT);
    limits.max_alloc = Some(MAX_IMAGE_ALLOC);

    let mut reader = ImageReader::new(Cursor::new(data));
    if let Some(image_format) = format.to_image_format() {
        reader.set_format(image_format);
    } else {
        reader = reader
            .with_guessed_format()
            .map_err(|e| format!("Failed to read input image: {}", e))?;
    }
    reader.limits(limits);

    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("Failed to decode input image: {}", e))?;

    let (width, height) = decoder.dimensions();
    validate_dimensions(width, height)?;

    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let icc_profile = decoder.icc_profile().ok().flatten();

    let img = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("Failed to decode input image: {}", e))?;

    Ok((img, orientation, icc_profile))
}

fn convert_to_srgb(img: DynamicImage, icc_profile: &[u8]) -> DynamicImage {
    if is_srgb_profile(icc_profile) {
        return img;
    }
    let Some(input) = Profile::new_from_slice(icc_profile, false) else {
        return img;
    };
    if input.is_sRGB() {
        return img;
    }

    let mut output = Profile::new_sRGB();
    output.precache_output_transform();

    match img {
        DynamicImage::ImageRgb8(mut image) => {
            let _ = apply_transform(&input, &output, DataType::RGB8, image.as_mut());
            DynamicImage::ImageRgb8(image)
        }
        DynamicImage::ImageRgba8(mut image) => {
            let _ = apply_transform(&input, &output, DataType::RGBA8, image.as_mut());
            DynamicImage::ImageRgba8(image)
        }
        DynamicImage::ImageLuma8(mut image) => {
            if is_gray_profile(icc_profile) {
                if let Some(transform) = Transform::new_to(
                    &input,
                    &output,
                    DataType::Gray8,
                    DataType::RGB8,
                    Intent::RelativeColorimetric,
                ) {
                    let mut dst = vec![0u8; image.len() * 3];
                    transform.convert(image.as_raw(), &mut dst);
                    for (i, p) in image.as_mut().iter_mut().enumerate() {
                        *p = dst[i * 3];
                    }
                    DynamicImage::ImageLuma8(image)
                } else {
                    let mut rgb = DynamicImage::ImageLuma8(image).into_rgb8();
                    let _ = apply_transform(&input, &output, DataType::RGB8, rgb.as_mut());
                    DynamicImage::ImageRgb8(rgb)
                }
            } else {
                let mut rgb = DynamicImage::ImageLuma8(image).into_rgb8();
                let _ = apply_transform(&input, &output, DataType::RGB8, rgb.as_mut());
                DynamicImage::ImageRgb8(rgb)
            }
        }
        DynamicImage::ImageLumaA8(mut image) => {
            if is_gray_profile(icc_profile) {
                if let Some(transform) = Transform::new_to(
                    &input,
                    &output,
                    DataType::GrayA8,
                    DataType::RGBA8,
                    Intent::RelativeColorimetric,
                ) {
                    let mut dst = vec![0u8; (image.len() / 2) * 4];
                    transform.convert(image.as_raw(), &mut dst);
                    for (i, chunk) in image.as_mut().chunks_exact_mut(2).enumerate() {
                        chunk[0] = dst[i * 4];
                        chunk[1] = dst[i * 4 + 3];
                    }
                    DynamicImage::ImageLumaA8(image)
                } else {
                    let mut rgba = DynamicImage::ImageLumaA8(image).into_rgba8();
                    let _ = apply_transform(&input, &output, DataType::RGBA8, rgba.as_mut());
                    DynamicImage::ImageRgba8(rgba)
                }
            } else {
                let mut rgba = DynamicImage::ImageLumaA8(image).into_rgba8();
                let _ = apply_transform(&input, &output, DataType::RGBA8, rgba.as_mut());
                DynamicImage::ImageRgba8(rgba)
            }
        }
        DynamicImage::ImageRgb16(image) => {
            let (w, h) = (image.width(), image.height());
            let mut rgb8 = DynamicImage::ImageRgb16(image).into_rgb8();
            let _ = apply_transform(&input, &output, DataType::RGB8, rgb8.as_mut());
            let rgb16 = ImageBuffer::from_fn(w, h, |x, y| {
                let p = rgb8.get_pixel(x, y);
                image::Rgb([
                    (p[0] as u16) * 257,
                    (p[1] as u16) * 257,
                    (p[2] as u16) * 257,
                ])
            });
            DynamicImage::ImageRgb16(rgb16)
        }
        DynamicImage::ImageRgba16(image) => {
            let (w, h) = (image.width(), image.height());
            let mut rgba8 = DynamicImage::ImageRgba16(image).into_rgba8();
            let _ = apply_transform(&input, &output, DataType::RGBA8, rgba8.as_mut());
            let rgba16 = ImageBuffer::from_fn(w, h, |x, y| {
                let p = rgba8.get_pixel(x, y);
                image::Rgba([
                    (p[0] as u16) * 257,
                    (p[1] as u16) * 257,
                    (p[2] as u16) * 257,
                    (p[3] as u16) * 257,
                ])
            });
            DynamicImage::ImageRgba16(rgba16)
        }
        DynamicImage::ImageLuma16(image) => {
            let (w, h) = (image.width(), image.height());
            let mut luma8 = DynamicImage::ImageLuma16(image).into_luma8();
            if is_gray_profile(icc_profile) {
                if let Some(transform) = Transform::new_to(
                    &input,
                    &output,
                    DataType::Gray8,
                    DataType::RGB8,
                    Intent::RelativeColorimetric,
                ) {
                    let mut dst = vec![0u8; luma8.len() * 3];
                    transform.convert(luma8.as_raw(), &mut dst);
                    for (i, p) in luma8.as_mut().iter_mut().enumerate() {
                        *p = dst[i * 3];
                    }
                    let luma16 = ImageBuffer::from_fn(w, h, |x, y| {
                        let p = luma8.get_pixel(x, y);
                        image::Luma([(p[0] as u16) * 257])
                    });
                    DynamicImage::ImageLuma16(luma16)
                } else {
                    let mut rgb = DynamicImage::ImageLuma8(luma8).into_rgb8();
                    let _ = apply_transform(&input, &output, DataType::RGB8, rgb.as_mut());
                    DynamicImage::ImageRgb8(rgb)
                }
            } else {
                let mut rgb = DynamicImage::ImageLuma8(luma8).into_rgb8();
                let _ = apply_transform(&input, &output, DataType::RGB8, rgb.as_mut());
                DynamicImage::ImageRgb8(rgb)
            }
        }
        DynamicImage::ImageLumaA16(image) => {
            let (w, h) = (image.width(), image.height());
            let mut la8 = DynamicImage::ImageLumaA16(image).into_luma_alpha8();
            if is_gray_profile(icc_profile) {
                if let Some(transform) = Transform::new_to(
                    &input,
                    &output,
                    DataType::GrayA8,
                    DataType::RGBA8,
                    Intent::RelativeColorimetric,
                ) {
                    let mut dst = vec![0u8; (la8.len() / 2) * 4];
                    transform.convert(la8.as_raw(), &mut dst);
                    for (i, chunk) in la8.as_mut().chunks_exact_mut(2).enumerate() {
                        chunk[0] = dst[i * 4];
                        chunk[1] = dst[i * 4 + 3];
                    }
                    let la16 = ImageBuffer::from_fn(w, h, |x, y| {
                        let p = la8.get_pixel(x, y);
                        image::LumaA([(p[0] as u16) * 257, (p[1] as u16) * 257])
                    });
                    DynamicImage::ImageLumaA16(la16)
                } else {
                    let mut rgba = DynamicImage::ImageLumaA8(la8).into_rgba8();
                    let _ = apply_transform(&input, &output, DataType::RGBA8, rgba.as_mut());
                    DynamicImage::ImageRgba8(rgba)
                }
            } else {
                let mut rgba = DynamicImage::ImageLumaA8(la8).into_rgba8();
                let _ = apply_transform(&input, &output, DataType::RGBA8, rgba.as_mut());
                DynamicImage::ImageRgba8(rgba)
            }
        }
        other => other,
    }
}

fn read_icc_description(icc: &[u8]) -> Option<String> {
    if icc.len() < 132 {
        return None;
    }
    let tag_count = u32::from_be_bytes(icc[128..132].try_into().ok()?) as usize;
    let mut offset = 132;
    for _ in 0..tag_count {
        if offset + 12 > icc.len() {
            return None;
        }
        let tag_sig = &icc[offset..offset + 4];
        let tag_offset = u32::from_be_bytes(icc[offset + 4..offset + 8].try_into().ok()?) as usize;
        let tag_len = u32::from_be_bytes(icc[offset + 8..offset + 12].try_into().ok()?) as usize;
        offset += 12;

        if tag_sig == b"desc" {
            let data = icc.get(tag_offset..tag_offset + tag_len)?;
            if data.len() < 8 {
                return None;
            }
            let type_sig = &data[0..4];
            if type_sig == b"desc" {
                if data.len() < 12 {
                    return None;
                }
                let str_len = u32::from_be_bytes(data[8..12].try_into().ok()?) as usize;
                let str_bytes = data.get(12..12 + str_len)?;
                return Some(
                    String::from_utf8_lossy(str_bytes)
                        .trim_matches('\0')
                        .to_string(),
                );
            } else if type_sig == b"mluc" {
                if data.len() < 16 {
                    return None;
                }
                let record_count = u32::from_be_bytes(data[8..12].try_into().ok()?) as usize;
                if record_count == 0 || data.len() < 16 + 12 {
                    return None;
                }
                let str_len = u32::from_be_bytes(data[20..24].try_into().ok()?) as usize;
                let str_offset = u32::from_be_bytes(data[24..28].try_into().ok()?) as usize;
                let str_bytes = data.get(str_offset..str_offset + str_len)?;
                let u16_chars: Vec<u16> = str_bytes
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                return Some(
                    String::from_utf16_lossy(&u16_chars)
                        .trim_matches('\0')
                        .to_string(),
                );
            }
        }
    }
    None
}

fn is_srgb_profile(icc_profile: &[u8]) -> bool {
    if let Some(desc) = read_icc_description(icc_profile) {
        let desc_lower = desc.to_lowercase();
        if desc_lower.contains("p3")
            || desc_lower.contains("smpte432")
            || desc_lower.contains("adobe")
            || desc_lower.contains("prophoto")
            || desc_lower.contains("rec2020")
            || desc_lower.contains("2020")
        {
            return false;
        }
        if desc_lower.contains("srgb") || desc_lower.contains("bt709") {
            return true;
        }
    }
    false
}

fn is_gray_profile(icc: &[u8]) -> bool {
    icc.len() >= 20 && &icc[16..20] == b"GRAY"
}

fn apply_transform(
    input: &Profile,
    output: &Profile,
    data_type: DataType,
    pixels: &mut [u8],
) -> Result<(), String> {
    let transform = Transform::new(input, output, data_type, Intent::RelativeColorimetric)
        .ok_or_else(|| "Failed to create ICC color transform".to_string())?;
    transform.apply(pixels);
    Ok(())
}

fn decode_jxl(data: &[u8]) -> Result<(DynamicImage, Orientation, Option<Vec<u8>>), String> {
    let mut decoder = JxlDecoder::new(Cursor::new(data))
        .map_err(|e| format!("Failed to decode JXL image: {}", e))?;

    let (width, height) = decoder.dimensions();
    validate_dimensions(width, height)?;

    // JxlDecoder automatically applies orientation during decoding
    let orientation = Orientation::NoTransforms;
    let icc_profile = decoder.icc_profile().ok().flatten();

    let img = DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("Failed to decode JXL image: {}", e))?;

    Ok((img, orientation, icc_profile))
}

fn decode_avif(data: &[u8]) -> Result<(DynamicImage, Orientation, Option<Vec<u8>>), String> {
    let opts = DecodeOptions::default()
        .with_max_width(Some(MAX_IMAGE_WIDTH))
        .with_max_height(Some(MAX_IMAGE_HEIGHT))
        .with_max_pixels(Some(MAX_IMAGE_PIXELS))
        .with_max_bytes(Some(MAX_INPUT_FILE_SIZE as u64));

    let avif = oxideav_avif::decode_with(data, &opts)
        .map_err(|e| format!("Failed to decode AVIF image: {}", e))?;
    let (width, height) = (avif.width(), avif.height());
    validate_dimensions(width, height)?;

    let img = if avif.has_alpha() {
        let rgba = avif
            .try_to_rgba8()
            .map_err(|e| format!("Failed to decode AVIF image: {}", e))?;
        let buffer = image::RgbaImage::from_raw(width, height, rgba)
            .ok_or_else(|| "Failed to decode AVIF image: invalid RGBA buffer".to_string())?;
        DynamicImage::ImageRgba8(buffer)
    } else {
        let rgb = avif
            .try_to_rgb8()
            .map_err(|e| format!("Failed to decode AVIF image: {}", e))?;
        let buffer = image::RgbImage::from_raw(width, height, rgb)
            .ok_or_else(|| "Failed to decode AVIF image: invalid RGB buffer".to_string())?;
        DynamicImage::ImageRgb8(buffer)
    };

    let orientation = avif
        .metadata
        .exif
        .as_deref()
        .and_then(|exif| {
            let chunk = if exif.starts_with(b"Exif\0\0") {
                &exif[6..]
            } else {
                exif
            };
            Orientation::from_exif_chunk(chunk)
        })
        .unwrap_or(Orientation::NoTransforms);

    let icc_profile = avif.metadata.icc;

    Ok((img, orientation, icc_profile))
}
