use crate::types::{JpegOptions, JpegSubsampling};
use image::DynamicImage;
use mozjpeg_rs::{Encoder, Preset, Subsampling};

pub(super) fn encode(
    img: &DynamicImage,
    quality: u8,
    options: &JpegOptions,
) -> Result<Vec<u8>, String> {
    let mut encoder = Encoder::new(Preset::default())
        .quality(quality)
        .progressive(options.progressive)
        .smoothing(options.smoothing);

    match options.subsampling {
        JpegSubsampling::Auto => {}
        JpegSubsampling::S444 => encoder = encoder.subsampling(Subsampling::S444),
        JpegSubsampling::S422 => encoder = encoder.subsampling(Subsampling::S422),
        JpegSubsampling::S420 => encoder = encoder.subsampling(Subsampling::S420),
        JpegSubsampling::S440 => encoder = encoder.subsampling(Subsampling::S440),
    }

    let result = match img {
        DynamicImage::ImageLuma8(gray) => {
            encoder.encode_gray(gray.as_raw(), gray.width(), gray.height())
        }
        _ => {
            let rgb = img.to_rgb8();
            encoder.encode_rgb(rgb.as_raw(), rgb.width(), rgb.height())
        }
    };

    result.map_err(|e| format!("Failed to encode JPEG image: {}", e))
}
