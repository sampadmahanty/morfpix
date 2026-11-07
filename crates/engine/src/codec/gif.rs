use crate::types::GifOptions;
use image::codecs::gif::{GifEncoder, Repeat};
use image::{DynamicImage, ExtendedColorType};

pub(super) fn encode(img: &DynamicImage, options: &GifOptions) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    let speed = (options.speed as i32).clamp(1, 30);
    let mut encoder = GifEncoder::new_with_speed(&mut buffer, speed);

    if let Some(repeat) = options.repeat {
        let r = match repeat {
            crate::types::GifRepeat::Infinite => Repeat::Infinite,
            crate::types::GifRepeat::Finite(n) => Repeat::Finite(n),
        };
        encoder.set_repeat(r).map_err(|e| e.to_string())?;
    }

    let (bytes, color) = if img.color().has_alpha() {
        (img.to_rgba8().into_raw(), ExtendedColorType::Rgba8)
    } else {
        (img.to_rgb8().into_raw(), ExtendedColorType::Rgb8)
    };

    encoder
        .encode(&bytes, img.width(), img.height(), color)
        .map_err(|e| format!("Failed to encode GIF image: {e}"))?;

    drop(encoder);
    Ok(buffer)
}
