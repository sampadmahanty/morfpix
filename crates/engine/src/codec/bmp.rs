use crate::types::{BmpBitsPerPixel, BmpOptions};
use image::{codecs::bmp::BmpEncoder, DynamicImage, ExtendedColorType};

pub(super) fn encode(img: &DynamicImage, options: &BmpOptions) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    let mut encoder = BmpEncoder::new(&mut buffer);

    match options.bpp {
        BmpBitsPerPixel::Bpp8 => {
            let gray = img.to_luma8();
            encoder
                .encode(
                    gray.as_raw(),
                    gray.width(),
                    gray.height(),
                    ExtendedColorType::L8,
                )
                .map_err(|e| format!("Failed to encode BMP image: {e}"))?;
        }
        BmpBitsPerPixel::Bpp24 => {
            let rgb = img.to_rgb8();
            encoder
                .encode(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ExtendedColorType::Rgb8,
                )
                .map_err(|e| format!("Failed to encode BMP image: {e}"))?;
        }
        BmpBitsPerPixel::Bpp32 => {
            let rgba = img.to_rgba8();
            encoder
                .encode(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    ExtendedColorType::Rgba8,
                )
                .map_err(|e| format!("Failed to encode BMP image: {e}"))?;
        }
        BmpBitsPerPixel::Auto => {
            if img.color().has_alpha() {
                let rgba = img.to_rgba8();
                encoder
                    .encode(
                        rgba.as_raw(),
                        rgba.width(),
                        rgba.height(),
                        ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| format!("Failed to encode BMP image: {e}"))?;
            } else if img.color() == image::ColorType::L8 {
                let gray = img.to_luma8();
                encoder
                    .encode(
                        gray.as_raw(),
                        gray.width(),
                        gray.height(),
                        ExtendedColorType::L8,
                    )
                    .map_err(|e| format!("Failed to encode BMP image: {e}"))?;
            } else {
                let rgb = img.to_rgb8();
                encoder
                    .encode(
                        rgb.as_raw(),
                        rgb.width(),
                        rgb.height(),
                        ExtendedColorType::Rgb8,
                    )
                    .map_err(|e| format!("Failed to encode BMP image: {e}"))?;
            }
        }
    }

    Ok(buffer)
}
