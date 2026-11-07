use crate::types::PngOptions;
use image::DynamicImage;
use std::io::Cursor;

pub(super) fn encode(img: &DynamicImage, options: &PngOptions) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    img.write_to(&mut Cursor::new(&mut buffer), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG image: {}", e))?;

    if options.optimize || options.interlace {
        let level = if options.optimize {
            options.level.clamp(0, 6)
        } else {
            0
        };
        let mut oxipng_options = oxipng::Options::from_preset(level);
        if options.interlace {
            oxipng_options.interlace = Some(true);
        }
        oxipng::optimize_from_memory(&buffer, &oxipng_options)
            .map_err(|e| format!("Failed to optimize PNG image: {}", e))
    } else {
        Ok(buffer)
    }
}
