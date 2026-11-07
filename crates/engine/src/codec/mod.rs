mod avif;
mod bmp;
mod gif;
mod jpeg;
mod jxl;
mod png;
mod webp;

use crate::types::{
    AvifOptions, BmpOptions, GifOptions, ImageFormat, JpegOptions, JxlOptions, PngOptions,
    WebpOptions,
};
use image::DynamicImage;

pub fn encode_image(
    img: &DynamicImage,
    format: ImageFormat,
    quality: u8,
) -> Result<Vec<u8>, String> {
    encode_image_with_options(
        img,
        format,
        quality,
        &JpegOptions::default(),
        &AvifOptions::default(),
        &PngOptions::default(),
        &WebpOptions::default(),
        &JxlOptions::default(),
        &GifOptions::default(),
        &BmpOptions::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_image_with_options(
    img: &DynamicImage,
    format: ImageFormat,
    quality: u8,
    jpeg: &JpegOptions,
    avif: &AvifOptions,
    png: &PngOptions,
    webp: &WebpOptions,
    jxl: &JxlOptions,
    gif: &GifOptions,
    bmp: &BmpOptions,
) -> Result<Vec<u8>, String> {
    let quality = quality.clamp(1, 100);

    match format {
        ImageFormat::Jpeg => jpeg::encode(img, quality, jpeg),
        ImageFormat::Png => png::encode(img, png),
        ImageFormat::WebP => webp::encode(img, quality, webp),
        ImageFormat::Bmp => bmp::encode(img, bmp),
        ImageFormat::Gif => gif::encode(img, gif),
        ImageFormat::Avif => avif::encode(img, quality, avif),
        ImageFormat::Jxl => jxl::encode(img, jxl),
        ImageFormat::Unknown => Err("unsupported output format".to_string()),
    }
}
