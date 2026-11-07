use crate::types::{AvifAlphaMode, AvifBitDepth, AvifColorModel, AvifOptions};
use image::DynamicImage;
use ravif::{AlphaColorMode, BitDepth, ColorModel, Encoder, Img};
use rgb::FromSlice;

pub const DEFAULT_MAX_AVIF_THREADS: usize = 4;

pub(super) fn encode(
    img: &DynamicImage,
    quality: u8,
    options: &AvifOptions,
) -> Result<Vec<u8>, String> {
    let quality = quality.clamp(1, 100);
    let alpha_quality = options.alpha_quality.unwrap_or(quality).clamp(1, 100);
    let speed = options.speed.clamp(1, 10);
    let max_threads = std::thread::available_parallelism()
        .map(|threads| threads.get())
        .unwrap_or(1);
    let threads = if options.threads > 0 {
        options.threads.min(max_threads)
    } else {
        max_threads.min(DEFAULT_MAX_AVIF_THREADS)
    };

    let encoder = Encoder::new()
        .with_quality(quality as f32)
        .with_alpha_quality(alpha_quality as f32)
        .with_speed(speed)
        .with_bit_depth(match options.bit_depth {
            AvifBitDepth::Eight => BitDepth::Eight,
            AvifBitDepth::Ten => BitDepth::Ten,
            AvifBitDepth::Auto => BitDepth::Auto,
        })
        .with_internal_color_model(match options.color_model {
            AvifColorModel::Ycbcr => ColorModel::YCbCr,
            AvifColorModel::Rgb => ColorModel::RGB,
        })
        .with_num_threads(Some(threads))
        .with_alpha_color_mode(match options.alpha_mode {
            AvifAlphaMode::Clean => AlphaColorMode::UnassociatedClean,
            AvifAlphaMode::Dirty => AlphaColorMode::UnassociatedDirty,
            AvifAlphaMode::Premultiplied => AlphaColorMode::Premultiplied,
        });

    let encoded = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        let pixels = rgba.as_raw().as_rgba();
        encoder.encode_rgba(Img::new(
            pixels,
            rgba.width() as usize,
            rgba.height() as usize,
        ))
    } else {
        let rgb = img.to_rgb8();
        let pixels = rgb.as_raw().as_rgb();
        encoder.encode_rgb(Img::new(
            pixels,
            rgb.width() as usize,
            rgb.height() as usize,
        ))
    };

    encoded
        .map(|image| image.avif_file)
        .map_err(|e| format!("Failed to encode AVIF image: {}", e))
}
