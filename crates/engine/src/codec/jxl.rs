use crate::types::JxlOptions;
use image::DynamicImage;
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
use zune_jpegxl::JxlSimpleEncoder;

fn make_encoder_options(
    width: usize,
    height: usize,
    colorspace: ColorSpace,
    depth: BitDepth,
    options: &JxlOptions,
) -> EncoderOptions {
    let effort = options.effort.clamp(1, 9);
    let mut opts = EncoderOptions::new(width, height, colorspace, depth).set_effort(effort);

    let max_threads = std::thread::available_parallelism()
        .map(|t| t.get())
        .unwrap_or(1);
    let threads = if options.threads > 0 {
        options.threads.min(max_threads)
    } else {
        max_threads.min(4)
    };
    opts = opts.set_num_threads(threads.min(255) as u8);

    opts
}

pub(super) fn encode(img: &DynamicImage, options: &JxlOptions) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    let (width, height) = (img.width() as usize, img.height() as usize);

    let result = match img {
        DynamicImage::ImageLuma8(gray) => JxlSimpleEncoder::new(
            gray.as_raw(),
            make_encoder_options(width, height, ColorSpace::Luma, BitDepth::Eight, options),
        )
        .encode(&mut buffer),
        DynamicImage::ImageLuma16(gray) => {
            let raw: &[u8] = bytemuck::cast_slice(gray.as_raw());
            JxlSimpleEncoder::new(
                raw,
                make_encoder_options(width, height, ColorSpace::Luma, BitDepth::Sixteen, options),
            )
            .encode(&mut buffer)
        }
        DynamicImage::ImageLumaA8(gray_alpha) => JxlSimpleEncoder::new(
            gray_alpha.as_raw(),
            make_encoder_options(width, height, ColorSpace::LumaA, BitDepth::Eight, options),
        )
        .encode(&mut buffer),
        DynamicImage::ImageLumaA16(gray_alpha) => {
            let raw: &[u8] = bytemuck::cast_slice(gray_alpha.as_raw());
            JxlSimpleEncoder::new(
                raw,
                make_encoder_options(width, height, ColorSpace::LumaA, BitDepth::Sixteen, options),
            )
            .encode(&mut buffer)
        }
        DynamicImage::ImageRgb8(rgb) => JxlSimpleEncoder::new(
            rgb.as_raw(),
            make_encoder_options(width, height, ColorSpace::RGB, BitDepth::Eight, options),
        )
        .encode(&mut buffer),
        DynamicImage::ImageRgb16(rgb) => {
            let raw: &[u8] = bytemuck::cast_slice(rgb.as_raw());
            JxlSimpleEncoder::new(
                raw,
                make_encoder_options(width, height, ColorSpace::RGB, BitDepth::Sixteen, options),
            )
            .encode(&mut buffer)
        }
        DynamicImage::ImageRgba8(rgba) => JxlSimpleEncoder::new(
            rgba.as_raw(),
            make_encoder_options(width, height, ColorSpace::RGBA, BitDepth::Eight, options),
        )
        .encode(&mut buffer),
        DynamicImage::ImageRgba16(rgba) => {
            let raw: &[u8] = bytemuck::cast_slice(rgba.as_raw());
            JxlSimpleEncoder::new(
                raw,
                make_encoder_options(width, height, ColorSpace::RGBA, BitDepth::Sixteen, options),
            )
            .encode(&mut buffer)
        }
        _ => {
            if img.color().has_alpha() {
                let rgba = img.to_rgba8();
                JxlSimpleEncoder::new(
                    rgba.as_raw(),
                    make_encoder_options(width, height, ColorSpace::RGBA, BitDepth::Eight, options),
                )
                .encode(&mut buffer)
            } else {
                let rgb = img.to_rgb8();
                JxlSimpleEncoder::new(
                    rgb.as_raw(),
                    make_encoder_options(width, height, ColorSpace::RGB, BitDepth::Eight, options),
                )
                .encode(&mut buffer)
            }
        }
    };

    result
        .map(|_| buffer)
        .map_err(|e| format!("Failed to encode JXL image: {}", e))
}
