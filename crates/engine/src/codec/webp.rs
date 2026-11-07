use crate::types::WebpOptions;
use image::DynamicImage;
use webp::WebPConfig;

pub(super) fn encode(
    img: &DynamicImage,
    quality: u8,
    options: &WebpOptions,
) -> Result<Vec<u8>, String> {
    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return Err("WebP image dimensions cannot be zero".to_string());
    }
    if width > 16383 || height > 16383 {
        return Err(format!(
            "WebP maximum dimensions are 16383x16383, but image is {}x{}",
            width, height
        ));
    }

    let mut config = WebPConfig::new().map_err(|()| "Failed to create WebP config".to_string())?;
    config.lossless = if options.lossless { 1 } else { 0 };
    config.quality = quality as f32;
    config.method = options.method.clamp(0, 6) as i32;
    config.exact = if options.exact { 1 } else { 0 };
    config.thread_level = 1;

    let memory = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        webp::Encoder::from_rgba(rgba.as_raw(), width, height)
            .encode_advanced(&config)
            .map_err(|e| format!("Failed to encode WebP image: {:?}", e))?
    } else {
        let rgb = img.to_rgb8();
        webp::Encoder::from_rgb(rgb.as_raw(), width, height)
            .encode_advanced(&config)
            .map_err(|e| format!("Failed to encode WebP image: {:?}", e))?
    };

    Ok(memory.to_vec())
}
