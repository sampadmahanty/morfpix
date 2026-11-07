pub mod codec;
pub mod pipeline;
pub mod types;

pub use codec::{encode_image, encode_image_with_options};
pub use pipeline::{
    convert, decode_image, decode_image_with_format, detect_format, estimate_memory_from_header,
    estimate_memory_usage, MAX_IMAGE_ALLOC, MAX_IMAGE_HEIGHT, MAX_IMAGE_PIXELS, MAX_IMAGE_WIDTH,
    MAX_INPUT_FILE_SIZE, MAX_REASONABLE_MEMORY_ESTIMATE,
};
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageEncoder, Rgba, RgbaImage};

    const DISPLAY_P3_ICC: &[u8] = include_bytes!("../../../tests/fixtures/display-p3.icc");

    fn create_icc_jpeg(width: u32, height: u32, color: [u8; 3], icc: &[u8]) -> Vec<u8> {
        let mut img = image::RgbImage::new(width, height);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgb(color);
        }

        let mut jpeg = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 100);
        encoder.set_icc_profile(icc.to_vec()).unwrap();
        encoder
            .encode(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
            .unwrap();
        jpeg
    }

    fn create_icc_png(
        width: u32,
        height: u32,
        color_type: image::ExtendedColorType,
        data: &[u8],
        icc: &[u8],
    ) -> Vec<u8> {
        use image::ImageEncoder;
        let mut png = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut png);
        encoder.set_icc_profile(icc.to_vec()).unwrap();
        encoder
            .write_image(data, width, height, color_type)
            .unwrap();
        png
    }

    fn find_colr_nclx(data: &[u8]) -> Option<(u16, u16, u16, u8)> {
        let pos = data.windows(8).position(|window| window == b"colrnclx")?;
        Some((
            u16::from_be_bytes([data[pos + 8], data[pos + 9]]),
            u16::from_be_bytes([data[pos + 10], data[pos + 11]]),
            u16::from_be_bytes([data[pos + 12], data[pos + 13]]),
            data[pos + 14] >> 7,
        ))
    }

    fn create_test_image(width: u32, height: u32) -> Vec<u8> {
        let mut img = RgbaImage::new(width, height);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            let r = ((x * 255) / width) as u8;
            let g = ((y * 255) / height) as u8;
            *pixel = Rgba([r, g, 128, 255]);
        }

        let mut buffer = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buffer);
        image::ImageEncoder::write_image(
            encoder,
            img.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
        buffer
    }

    #[test]
    fn detects_png_format() {
        let data = create_test_image(32, 32);
        assert_eq!(detect_format(&data), ImageFormat::Png);
    }

    #[test]
    fn converts_png_to_jpeg() {
        let data = create_test_image(120, 80);
        let options = ConversionOptions {
            format: Some(ImageFormat::Jpeg),
            quality: 80,
            ..Default::default()
        };

        let (output, result) = convert(&data, &options).unwrap();

        assert!(!output.is_empty());
        assert_eq!(result.input_format, ImageFormat::Png);
        assert_eq!(result.output_format, ImageFormat::Jpeg);
        assert_eq!(result.width, 120);
        assert_eq!(result.height, 80);
        assert_eq!(result.output_size_bytes, output.len());
    }

    #[test]
    fn applies_jpeg_encoding_options() {
        let data = create_test_image(64, 64);

        // Test baseline vs progressive
        let baseline_opts = ConversionOptions {
            format: Some(ImageFormat::Jpeg),
            quality: 80,
            jpeg: JpegOptions {
                progressive: false,
                subsampling: JpegSubsampling::Auto,
                smoothing: 0,
            },
            ..Default::default()
        };
        let progressive_opts = ConversionOptions {
            format: Some(ImageFormat::Jpeg),
            quality: 80,
            jpeg: JpegOptions {
                progressive: true,
                subsampling: JpegSubsampling::Auto,
                smoothing: 0,
            },
            ..Default::default()
        };

        let (baseline_out, baseline_res) = convert(&data, &baseline_opts).unwrap();
        let (prog_out, prog_res) = convert(&data, &progressive_opts).unwrap();

        assert_eq!(baseline_res.output_format, ImageFormat::Jpeg);
        assert_eq!(prog_res.output_format, ImageFormat::Jpeg);

        // Baseline JPEG uses SOF0 (0xFF, 0xC0) marker
        assert!(baseline_out.windows(2).any(|w| w == [0xFF, 0xC0]));
        // Progressive JPEG uses SOF2 (0xFF, 0xC2) marker
        assert!(prog_out.windows(2).any(|w| w == [0xFF, 0xC2]));

        // Test chroma subsampling 444 vs 420
        let s444_opts = ConversionOptions {
            format: Some(ImageFormat::Jpeg),
            quality: 85,
            jpeg: JpegOptions {
                progressive: false,
                subsampling: JpegSubsampling::S444,
                smoothing: 0,
            },
            ..Default::default()
        };
        let s420_opts = ConversionOptions {
            format: Some(ImageFormat::Jpeg),
            quality: 85,
            jpeg: JpegOptions {
                progressive: false,
                subsampling: JpegSubsampling::S420,
                smoothing: 0,
            },
            ..Default::default()
        };

        let (s444_out, _) = convert(&data, &s444_opts).unwrap();
        let (s420_out, _) = convert(&data, &s420_opts).unwrap();

        assert!(!s444_out.is_empty());
        assert!(!s420_out.is_empty());
        // 444 has higher color fidelity, so file size is generally larger than 420
        assert!(s444_out.len() > s420_out.len());

        // Test grayscale image encoding
        let mut gray_img = image::GrayImage::new(32, 32);
        for (x, y, p) in gray_img.enumerate_pixels_mut() {
            *p = image::Luma([(x * 8 + y * 8) as u8]);
        }
        let mut gray_png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut gray_png)
            .write_image(gray_img.as_raw(), 32, 32, image::ExtendedColorType::L8)
            .unwrap();

        let (gray_jpeg, gray_res) = convert(&gray_png, &baseline_opts).unwrap();
        assert_eq!(gray_res.output_format, ImageFormat::Jpeg);
        let decoded = image::load_from_memory(&gray_jpeg).unwrap();
        assert_eq!(decoded.width(), 32);
        assert_eq!(decoded.height(), 32);
    }

    #[test]
    fn converts_png_to_webp() {
        let data = create_test_image(64, 64);
        let options = ConversionOptions {
            format: Some(ImageFormat::WebP),
            quality: 75,
            ..Default::default()
        };

        let (output, result) = convert(&data, &options).unwrap();

        assert!(!output.is_empty());
        assert_eq!(result.output_format, ImageFormat::WebP);
        assert_eq!(&output[0..4], b"RIFF");
        assert_eq!(&output[8..12], b"WEBP");
    }

    #[test]
    fn converts_png_to_webp_lossless() {
        let mut img = RgbaImage::new(16, 16);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = Rgba([(x * 15) as u8, (y * 15) as u8, 128, 255]);
        }

        let mut input = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut input);
        image::ImageEncoder::write_image(
            encoder,
            img.as_raw(),
            16,
            16,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();

        let options = ConversionOptions {
            format: Some(ImageFormat::WebP),
            webp: WebpOptions {
                lossless: true,
                method: 6,
                exact: true,
            },
            ..Default::default()
        };

        let (output, result) = convert(&input, &options).unwrap();
        assert_eq!(result.output_format, ImageFormat::WebP);

        let decoded =
            image::load_from_memory_with_format(&output, image::ImageFormat::WebP).unwrap();
        assert_eq!(decoded.to_rgba8().into_raw(), img.into_raw());
    }

    #[test]
    fn applies_webp_encoding_options() {
        let data = create_test_image(32, 32);

        let fast_options = ConversionOptions {
            format: Some(ImageFormat::WebP),
            quality: 75,
            webp: WebpOptions {
                lossless: false,
                method: 0,
                exact: false,
            },
            ..Default::default()
        };

        let high_options = ConversionOptions {
            format: Some(ImageFormat::WebP),
            quality: 75,
            webp: WebpOptions {
                lossless: false,
                method: 6,
                exact: false,
            },
            ..Default::default()
        };

        let (fast_output, _) = convert(&data, &fast_options).unwrap();
        let (high_output, _) = convert(&data, &high_options).unwrap();

        assert!(!fast_output.is_empty());
        assert!(!high_output.is_empty());
        assert_ne!(fast_output, high_output);
    }

    #[test]
    fn converts_png_to_avif() {
        let data = create_test_image(32, 32);
        let options = ConversionOptions {
            format: Some(ImageFormat::Avif),
            quality: 80,
            ..Default::default()
        };

        let (output, result) = convert(&data, &options).unwrap();

        assert!(!output.is_empty());
        assert_eq!(result.output_format, ImageFormat::Avif);
        assert_eq!(&output[4..8], b"ftyp");
        assert_eq!(detect_format(&output), ImageFormat::Avif);

        let decoded = oxideav_avif::decode(&output).unwrap();
        assert_eq!(decoded.width(), 32);
        assert_eq!(decoded.height(), 32);
    }

    #[test]
    fn avif_output_preserves_alpha() {
        let mut img = RgbaImage::new(16, 16);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = Rgba([(x * 16) as u8, (y * 16) as u8, 64, 128]);
        }

        let mut input = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut input);
        image::ImageEncoder::write_image(
            encoder,
            img.as_raw(),
            16,
            16,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();

        let options = ConversionOptions {
            format: Some(ImageFormat::Avif),
            quality: 80,
            ..Default::default()
        };
        let (output, _) = convert(&input, &options).unwrap();

        assert!(oxideav_avif::decode(&output).unwrap().has_alpha());
    }

    #[test]
    fn bmp_output_preserves_alpha() {
        let mut img = RgbaImage::new(8, 8);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([50, 100, 150, 128]);
        }

        let mut input = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut input);
        image::ImageEncoder::write_image(
            encoder,
            img.as_raw(),
            8,
            8,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();

        let options = ConversionOptions {
            format: Some(ImageFormat::Bmp),
            ..Default::default()
        };
        let (output, result) = convert(&input, &options).unwrap();
        assert_eq!(result.output_format, ImageFormat::Bmp);

        let decoded = image::load_from_memory(&output).unwrap();
        assert_eq!(decoded.color(), image::ColorType::Rgba8);
        let pixel = decoded.to_rgba8().get_pixel(0, 0).0;
        assert_eq!(pixel[3], 128);
    }

    #[test]
    fn applies_avif_encoding_options() {
        let mut img = RgbaImage::new(16, 16);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = Rgba([(x * 16) as u8, (y * 16) as u8, 64, 128]);
        }

        let mut input = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut input);
        image::ImageEncoder::write_image(
            encoder,
            img.as_raw(),
            16,
            16,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();

        let options = ConversionOptions {
            format: Some(ImageFormat::Avif),
            quality: 60,
            avif: AvifOptions {
                speed: 10,
                alpha_quality: Some(40),
                bit_depth: AvifBitDepth::Eight,
                color_model: AvifColorModel::Rgb,
                threads: 1,
                alpha_mode: AvifAlphaMode::Dirty,
            },
            ..Default::default()
        };

        let (output, result) = convert(&input, &options).unwrap();

        assert_eq!(result.output_format, ImageFormat::Avif);
        assert!(oxideav_avif::decode(&output).unwrap().has_alpha());
    }

    #[test]
    fn converts_display_p3_jpeg_to_srgb() {
        let jpeg = create_icc_jpeg(8, 8, [94, 148, 211], DISPLAY_P3_ICC);
        let options = ConversionOptions {
            format: Some(ImageFormat::Png),
            ..Default::default()
        };
        let (output, _) = convert(&jpeg, &options).unwrap();
        let decoded = image::load_from_memory(&output).unwrap().to_rgb8();
        let pixel = decoded.get_pixel(4, 4).0;

        assert!(pixel[0].abs_diff(75) <= 3, "red was {}", pixel[0]);
        assert!(pixel[1].abs_diff(150) <= 3, "green was {}", pixel[1]);
        assert!(pixel[2].abs_diff(216) <= 3, "blue was {}", pixel[2]);
    }

    #[test]
    fn avif_output_keeps_srgb_metadata_after_p3_conversion() {
        let jpeg = create_icc_jpeg(8, 8, [94, 148, 211], DISPLAY_P3_ICC);
        let options = ConversionOptions {
            format: Some(ImageFormat::Avif),
            quality: 100,
            avif: AvifOptions {
                speed: 10,
                bit_depth: AvifBitDepth::Eight,
                color_model: AvifColorModel::Rgb,
                threads: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let (output, _) = convert(&jpeg, &options).unwrap();

        assert_eq!(find_colr_nclx(&output), Some((1, 13, 0, 1)));

        let decoded = oxideav_avif::decode(&output).unwrap();
        let rgb = decoded.to_rgb8();
        let offset = (4 * decoded.width() as usize + 4) * 3;
        assert!(rgb[offset].abs_diff(75) <= 6, "red was {}", rgb[offset]);
        assert!(
            rgb[offset + 1].abs_diff(150) <= 6,
            "green was {}",
            rgb[offset + 1]
        );
        assert!(
            rgb[offset + 2].abs_diff(216) <= 6,
            "blue was {}",
            rgb[offset + 2]
        );
    }

    #[test]
    fn p3_png_preserves_alpha_after_srgb_conversion() {
        let mut img = RgbaImage::new(4, 4);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([94, 148, 211, 123]);
        }

        let mut png = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut png);
        encoder.set_icc_profile(DISPLAY_P3_ICC.to_vec()).unwrap();
        encoder
            .write_image(img.as_raw(), 4, 4, image::ExtendedColorType::Rgba8)
            .unwrap();

        let options = ConversionOptions {
            format: Some(ImageFormat::Png),
            ..Default::default()
        };
        let (output, _) = convert(&png, &options).unwrap();
        let decoded = image::load_from_memory(&output).unwrap().to_rgba8();
        let pixel = decoded.get_pixel(0, 0).0;

        assert_eq!(pixel[3], 123);
        assert!(pixel[0].abs_diff(75) <= 3, "red was {}", pixel[0]);
        assert!(pixel[1].abs_diff(150) <= 3, "green was {}", pixel[1]);
        assert!(pixel[2].abs_diff(216) <= 3, "blue was {}", pixel[2]);
    }

    #[test]
    fn srgb_icc_profile_is_left_unchanged() {
        let jpeg = include_bytes!("../../../tests/fixtures/phone-photo.jpeg");
        let raw = image::load_from_memory(jpeg).unwrap().to_rgb8();
        let options = ConversionOptions {
            format: Some(ImageFormat::Png),
            ..Default::default()
        };
        let (output, _) = convert(jpeg, &options).unwrap();
        let decoded = image::load_from_memory(&output).unwrap().to_rgb8();

        assert_eq!(decoded, raw);
    }

    #[test]
    fn corrupt_icc_profile_falls_back_to_raw_image() {
        let corrupt_icc = b"INVALID_CORRUPTED_ICC_PROFILE_PAYLOAD";
        let jpeg = create_icc_jpeg(8, 8, [100, 150, 200], corrupt_icc);
        let options = ConversionOptions {
            format: Some(ImageFormat::Png),
            ..Default::default()
        };
        let (output, _) = convert(&jpeg, &options)
            .expect("conversion should succeed by falling back to raw image");
        let decoded = image::load_from_memory(&output).unwrap().to_rgb8();
        let pixel = decoded.get_pixel(4, 4).0;
        assert!(pixel[0].abs_diff(100) <= 2, "red was {}", pixel[0]);
        assert!(pixel[1].abs_diff(150) <= 2, "green was {}", pixel[1]);
        assert!(pixel[2].abs_diff(200) <= 2, "blue was {}", pixel[2]);
    }

    #[test]
    fn converts_png_to_jxl_losslessly() {
        let data = create_test_image(32, 32);
        let options = ConversionOptions {
            format: Some(ImageFormat::Jxl),
            quality: 80,
            ..Default::default()
        };

        let (output, result) = convert(&data, &options).unwrap();

        assert!(!output.is_empty());
        assert_eq!(result.output_format, ImageFormat::Jxl);
        assert_eq!(&output[0..2], &[0xFF, 0x0A]);
        assert_eq!(detect_format(&output), ImageFormat::Jxl);

        let png_options = ConversionOptions {
            format: Some(ImageFormat::Png),
            quality: 80,
            ..Default::default()
        };
        let (decoded_png, decoded_result) = convert(&output, &png_options).unwrap();
        assert_eq!(decoded_result.input_format, ImageFormat::Jxl);
        assert_eq!(decoded_result.width, 32);
        assert_eq!(decoded_result.height, 32);

        let original = image::load_from_memory(&data).unwrap().to_rgba8();
        let decoded = image::load_from_memory(&decoded_png).unwrap().to_rgba8();
        assert_eq!(decoded, original);
    }

    #[test]
    fn applies_jxl_encoding_options() {
        let data = create_test_image(128, 128);

        let fast_options = ConversionOptions {
            format: Some(ImageFormat::Jxl),
            jxl: JxlOptions {
                effort: 1,
                threads: 1,
            },
            ..Default::default()
        };

        let slow_options = ConversionOptions {
            format: Some(ImageFormat::Jxl),
            jxl: JxlOptions {
                effort: 9,
                threads: 2,
            },
            ..Default::default()
        };

        let (fast_output, fast_result) = convert(&data, &fast_options).unwrap();
        let (slow_output, slow_result) = convert(&data, &slow_options).unwrap();

        assert_eq!(fast_result.output_format, ImageFormat::Jxl);
        assert_eq!(slow_result.output_format, ImageFormat::Jxl);
        assert_eq!(&fast_output[0..2], &[0xFF, 0x0A]);
        assert_eq!(&slow_output[0..2], &[0xFF, 0x0A]);

        // Verify both decode losslessly to original image
        let original = image::load_from_memory(&data).unwrap().to_rgba8();

        let png_opts = ConversionOptions {
            format: Some(ImageFormat::Png),
            ..Default::default()
        };
        let (fast_png, _) = convert(&fast_output, &png_opts).unwrap();
        let (slow_png, _) = convert(&slow_output, &png_opts).unwrap();

        assert_eq!(
            image::load_from_memory(&fast_png).unwrap().to_rgba8(),
            original
        );
        assert_eq!(
            image::load_from_memory(&slow_png).unwrap().to_rgba8(),
            original
        );
    }

    #[test]
    fn converts_grayscale_png_to_jxl_losslessly() {
        let mut img = image::GrayImage::new(16, 16);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Luma([(x + y) as u8]);
        }

        let mut input = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut input);
        image::ImageEncoder::write_image(
            encoder,
            img.as_raw(),
            16,
            16,
            image::ExtendedColorType::L8,
        )
        .unwrap();

        let options = ConversionOptions {
            format: Some(ImageFormat::Jxl),
            quality: 80,
            ..Default::default()
        };
        let (output, _) = convert(&input, &options).unwrap();

        let png_options = ConversionOptions {
            format: Some(ImageFormat::Png),
            quality: 80,
            ..Default::default()
        };
        let (decoded_png, _) = convert(&output, &png_options).unwrap();

        let original = image::load_from_memory(&input).unwrap().to_luma8();
        let decoded = image::load_from_memory(&decoded_png).unwrap().to_luma8();
        assert_eq!(decoded, original);
    }

    #[test]
    fn converts_16bit_grayscale_png_to_jxl_losslessly() {
        let mut img = image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::new(16, 16);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Luma([(x + y) as u16 * 1000]);
        }

        let mut input = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut input);
        let raw_bytes: Vec<u8> = img.as_raw().iter().flat_map(|x| x.to_be_bytes()).collect();
        image::ImageEncoder::write_image(
            encoder,
            &raw_bytes,
            16,
            16,
            image::ExtendedColorType::L16,
        )
        .unwrap();

        let options = ConversionOptions {
            format: Some(ImageFormat::Jxl),
            ..Default::default()
        };
        let (output, _) = convert(&input, &options).unwrap();

        let png_options = ConversionOptions {
            format: Some(ImageFormat::Png),
            ..Default::default()
        };
        let (decoded_png, _) = convert(&output, &png_options).unwrap();

        let original = image::load_from_memory(&input).unwrap().to_luma16();
        let decoded = image::load_from_memory(&decoded_png).unwrap().to_luma16();
        assert_eq!(decoded, original);
    }

    #[test]
    fn png_output_preserves_color_type() {
        let mut img = image::RgbImage::new(32, 32);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgb([x as u8, y as u8, 128]);
        }

        let mut input = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut input);
        image::ImageEncoder::write_image(
            encoder,
            img.as_raw(),
            32,
            32,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();

        let options = ConversionOptions {
            format: Some(ImageFormat::Png),
            quality: 80,
            ..Default::default()
        };
        let (output, _) = convert(&input, &options).unwrap();

        let decoded = image::load_from_memory(&output).unwrap();
        assert_eq!(decoded.color(), image::ColorType::Rgb8);
    }

    #[test]
    fn applies_exif_orientation() {
        let mut img = image::RgbImage::new(2, 1);
        img.put_pixel(0, 0, image::Rgb([255, 0, 0]));
        img.put_pixel(1, 0, image::Rgb([0, 0, 255]));

        let mut jpeg = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 100);
        encoder
            .encode(img.as_raw(), 2, 1, image::ExtendedColorType::Rgb8)
            .unwrap();

        let mut with_exif = Vec::new();
        with_exif.extend_from_slice(&jpeg[0..2]);
        with_exif.extend_from_slice(&[
            0xFF, 0xE1, 0x00, 0x22, b'E', b'x', b'i', b'f', 0x00, 0x00, 0x49, 0x49, 0x2A, 0x00,
            0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]);
        with_exif.extend_from_slice(&jpeg[2..]);

        let options = ConversionOptions {
            format: Some(ImageFormat::Png),
            quality: 80,
            ..Default::default()
        };
        let (output, result) = convert(&with_exif, &options).unwrap();

        assert_eq!(result.width, 1);
        assert_eq!(result.height, 2);

        let decoded = image::load_from_memory(&output).unwrap();
        assert_eq!(decoded.width(), 1);
        assert_eq!(decoded.height(), 2);
    }

    #[test]
    fn keeps_input_format_by_default() {
        let data = create_test_image(40, 40);
        let (output, result) = convert(&data, &ConversionOptions::default()).unwrap();

        assert!(!output.is_empty());
        assert_eq!(result.input_format, ImageFormat::Png);
        assert_eq!(result.output_format, ImageFormat::Png);
    }

    #[test]
    fn rejects_unknown_output_format() {
        let data = create_test_image(16, 16);
        let options = ConversionOptions {
            format: Some(ImageFormat::Unknown),
            quality: 80,
            ..Default::default()
        };

        assert!(convert(&data, &options).is_err());
    }

    #[test]
    fn rejects_webp_exceeding_max_dimensions() {
        let large_img = image::DynamicImage::ImageRgb8(image::RgbImage::new(16384, 1));
        let result = encode_image(&large_img, ImageFormat::WebP, 80);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("16383"));
    }

    #[test]
    fn rejects_invalid_input() {
        assert!(convert(b"not an image", &ConversionOptions::default()).is_err());
    }

    #[test]
    fn avif_applies_exif_orientation() {
        let rgb = vec![255, 0, 0, 0, 0, 255];
        let exif_payload = vec![
            0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let avif_img = oxideav_avif::AvifImage::from_rgb8(2, 1, rgb)
            .unwrap()
            .with_metadata(oxideav_avif::Metadata::default().with_exif(Some(exif_payload)));
        let avif_bytes =
            oxideav_avif::encode(&avif_img, &oxideav_avif::EncodeOptions::default()).unwrap();

        let options = ConversionOptions {
            format: Some(ImageFormat::Png),
            ..Default::default()
        };
        let (output, result) = convert(&avif_bytes, &options).unwrap();
        assert_eq!(result.input_format, ImageFormat::Avif);
        assert_eq!(result.width, 1);
        assert_eq!(result.height, 2);

        let decoded = image::load_from_memory(&output).unwrap();
        assert_eq!(decoded.width(), 1);
        assert_eq!(decoded.height(), 2);
    }

    #[test]
    fn avif_converts_display_p3_icc_to_srgb() {
        let p3_icc = include_bytes!("../../../tests/fixtures/display-p3.icc");
        let rgb = vec![100, 150, 200];
        let avif_img = oxideav_avif::AvifImage::from_rgb8(1, 1, rgb)
            .unwrap()
            .with_metadata(oxideav_avif::Metadata::default().with_icc(Some(p3_icc.to_vec())));
        let avif_bytes =
            oxideav_avif::encode(&avif_img, &oxideav_avif::EncodeOptions::default()).unwrap();

        let options = ConversionOptions {
            format: Some(ImageFormat::Png),
            ..Default::default()
        };
        let (output, _) = convert(&avif_bytes, &options).unwrap();
        let decoded = image::load_from_memory(&output).unwrap().to_rgb8();
        let pixel = decoded.get_pixel(0, 0).0;
        assert!(pixel[0].abs_diff(83) <= 4, "red was {}", pixel[0]);
        assert!(pixel[1].abs_diff(150) <= 6, "green was {}", pixel[1]);
        assert!(pixel[2].abs_diff(205) <= 4, "blue was {}", pixel[2]);
    }

    #[test]
    fn converts_16bit_rgb_with_icc_to_srgb() {
        let p3_icc = include_bytes!("../../../tests/fixtures/display-p3.icc");
        // 16-bit RGB sample: [100, 150, 200] scaled to 16-bit
        let raw: Vec<u8> = [100u16 * 257, 150u16 * 257, 200u16 * 257]
            .iter()
            .flat_map(|x| x.to_be_bytes())
            .collect();
        let png = create_icc_png(1, 1, image::ExtendedColorType::Rgb16, &raw, p3_icc);

        let options = ConversionOptions {
            format: Some(ImageFormat::Png),
            ..Default::default()
        };
        let (output, _) = convert(&png, &options).unwrap();
        let decoded = image::load_from_memory(&output).unwrap().to_rgb8();
        let pixel = decoded.get_pixel(0, 0).0;
        assert!(pixel[0].abs_diff(83) <= 4, "red was {}", pixel[0]);
        assert!(pixel[1].abs_diff(150) <= 6, "green was {}", pixel[1]);
        assert!(pixel[2].abs_diff(205) <= 4, "blue was {}", pixel[2]);
    }

    #[test]
    fn converts_grayscale_with_icc_to_srgb() {
        let p3_icc = include_bytes!("../../../tests/fixtures/display-p3.icc");
        let raw = [150u8];
        let png = create_icc_png(1, 1, image::ExtendedColorType::L8, &raw, p3_icc);

        let options = ConversionOptions {
            format: Some(ImageFormat::Png),
            ..Default::default()
        };
        let (output, result) = convert(&png, &options).unwrap();
        assert_eq!(result.input_format, ImageFormat::Png);
        let decoded = image::load_from_memory(&output).unwrap();
        assert_eq!(decoded.width(), 1);
        assert_eq!(decoded.height(), 1);
    }

    #[test]
    fn rejects_image_exceeding_max_dimensions() {
        // Valid PNG header with 20000x100 px dimensions
        let png = [
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 78, 32, 0, 0, 0,
            100, 8, 2, 0, 0, 0, 236, 239, 151, 48,
        ];
        let options = ConversionOptions::default();
        let err = convert(&png, &options).unwrap_err();
        assert!(
            err.contains("exceed maximum dimension limit") || err.contains("Failed to decode"),
            "unexpected error message: {}",
            err
        );
    }

    #[test]
    fn avif_rejects_excessive_dimensions() {
        let avif_img = oxideav_avif::AvifImage::from_rgb8(1, 1, vec![255, 0, 0]).unwrap();
        let avif_bytes =
            oxideav_avif::encode(&avif_img, &oxideav_avif::EncodeOptions::default()).unwrap();
        let opts = oxideav_avif::DecodeOptions::default().with_max_width(Some(0));
        assert!(oxideav_avif::decode_with(&avif_bytes, &opts).is_err());
    }

    #[test]
    fn applies_png_optimization_options() {
        let sample = include_bytes!("../../../tests/fixtures/sample-photo.png");

        let fast_options = ConversionOptions {
            format: Some(ImageFormat::Png),
            png: PngOptions {
                optimize: false,
                level: 0,
                interlace: false,
            },
            ..Default::default()
        };
        let (fast_out, _) = convert(sample, &fast_options).unwrap();
        assert!(image::load_from_memory(&fast_out).is_ok());
        // Byte 28 in standard PNG IHDR is the interlace method (0 = none)
        assert_eq!(fast_out[28], 0);

        let interlace_options = ConversionOptions {
            format: Some(ImageFormat::Png),
            png: PngOptions {
                optimize: false,
                level: 0,
                interlace: true,
            },
            ..Default::default()
        };
        let (interlace_out, _) = convert(sample, &interlace_options).unwrap();
        assert!(image::load_from_memory(&interlace_out).is_ok());
        // Byte 28 in PNG IHDR is 1 for Adam7 interlaced
        assert_eq!(interlace_out[28], 1);

        let opt_options = ConversionOptions {
            format: Some(ImageFormat::Png),
            png: PngOptions {
                optimize: true,
                level: 3,
                interlace: false,
            },
            ..Default::default()
        };
        let (opt_out, _) = convert(sample, &opt_options).unwrap();
        assert!(image::load_from_memory(&opt_out).is_ok());
    }

    #[test]
    fn applies_gif_encoding_options() {
        let sample = include_bytes!("../../../tests/fixtures/sample-photo.png");

        let infinite_options = ConversionOptions {
            format: Some(ImageFormat::Gif),
            gif: GifOptions {
                speed: 10,
                repeat: Some(GifRepeat::Infinite),
            },
            ..Default::default()
        };
        let (infinite_out, _) = convert(sample, &infinite_options).unwrap();
        assert_eq!(&infinite_out[0..6], b"GIF89a");
        assert!(infinite_out.windows(11).any(|w| w == b"NETSCAPE2.0"));
        let decoded = image::load_from_memory(&infinite_out).unwrap();
        assert_eq!(decoded.width(), 64);
        assert_eq!(decoded.height(), 64);

        let none_options = ConversionOptions {
            format: Some(ImageFormat::Gif),
            gif: GifOptions {
                speed: 10,
                repeat: None,
            },
            ..Default::default()
        };
        let (none_out, _) = convert(sample, &none_options).unwrap();
        assert_eq!(&none_out[0..6], b"GIF89a");
        assert!(!none_out.windows(11).any(|w| w == b"NETSCAPE2.0"));
        let decoded = image::load_from_memory(&none_out).unwrap();
        assert_eq!(decoded.width(), 64);
        assert_eq!(decoded.height(), 64);

        let finite_options = ConversionOptions {
            format: Some(ImageFormat::Gif),
            gif: GifOptions {
                speed: 10,
                repeat: Some(GifRepeat::Finite(3)),
            },
            ..Default::default()
        };
        let (finite_out, _) = convert(sample, &finite_options).unwrap();
        assert_eq!(&finite_out[0..6], b"GIF89a");
        assert!(finite_out.windows(11).any(|w| w == b"NETSCAPE2.0"));

        let speed_options = ConversionOptions {
            format: Some(ImageFormat::Gif),
            gif: GifOptions {
                speed: 1,
                repeat: Some(GifRepeat::Infinite),
            },
            ..Default::default()
        };
        let (speed_out, _) = convert(sample, &speed_options).unwrap();
        assert_eq!(&speed_out[0..6], b"GIF89a");
        let decoded = image::load_from_memory(&speed_out).unwrap();
        assert_eq!(decoded.width(), 64);
        assert_eq!(decoded.height(), 64);
    }

    #[test]
    fn applies_bmp_encoding_options() {
        let sample = include_bytes!("../../../tests/fixtures/sample-photo.png");

        // Test Bpp8: forces 8-bit grayscale BMP
        let bpp8_options = ConversionOptions {
            format: Some(ImageFormat::Bmp),
            bmp: BmpOptions {
                bpp: BmpBitsPerPixel::Bpp8,
            },
            ..Default::default()
        };
        let (bpp8_out, _) = convert(sample, &bpp8_options).unwrap();
        assert_eq!(&bpp8_out[0..2], b"BM");
        // DIB header size is 40
        assert_eq!(&bpp8_out[14..18], &[40, 0, 0, 0]);
        // Bits per pixel is 8
        assert_eq!(&bpp8_out[28..30], &[8, 0]);
        let decoded = image::load_from_memory(&bpp8_out).unwrap();
        assert_eq!(decoded.width(), 64);
        assert_eq!(decoded.height(), 64);

        // Test Bpp24: forces 24-bit RGB BMP (discards alpha if any)
        let bpp24_options = ConversionOptions {
            format: Some(ImageFormat::Bmp),
            bmp: BmpOptions {
                bpp: BmpBitsPerPixel::Bpp24,
            },
            ..Default::default()
        };
        let (bpp24_out, _) = convert(sample, &bpp24_options).unwrap();
        assert_eq!(&bpp24_out[0..2], b"BM");
        // DIB header size is 40
        assert_eq!(&bpp24_out[14..18], &[40, 0, 0, 0]);
        // Bits per pixel is 24
        assert_eq!(&bpp24_out[28..30], &[24, 0]);
        let decoded = image::load_from_memory(&bpp24_out).unwrap();
        assert_eq!(decoded.color(), image::ColorType::Rgb8);
        assert_eq!(decoded.width(), 64);
        assert_eq!(decoded.height(), 64);

        // Test Bpp32: forces 32-bit RGBA BMP with BITMAPV4HEADER
        let bpp32_options = ConversionOptions {
            format: Some(ImageFormat::Bmp),
            bmp: BmpOptions {
                bpp: BmpBitsPerPixel::Bpp32,
            },
            ..Default::default()
        };
        let (bpp32_out, _) = convert(sample, &bpp32_options).unwrap();
        assert_eq!(&bpp32_out[0..2], b"BM");
        // DIB header size is 108 (BITMAPV4HEADER)
        assert_eq!(&bpp32_out[14..18], &[108, 0, 0, 0]);
        // Bits per pixel is 32
        assert_eq!(&bpp32_out[28..30], &[32, 0]);
        let decoded = image::load_from_memory(&bpp32_out).unwrap();
        assert_eq!(decoded.color(), image::ColorType::Rgba8);
        assert_eq!(decoded.width(), 64);
        assert_eq!(decoded.height(), 64);

        // Size check: 8-bit is smaller than 24-bit, 24-bit is smaller than 32-bit
        assert!(bpp8_out.len() < bpp24_out.len());
        assert!(bpp24_out.len() < bpp32_out.len());
    }
}
