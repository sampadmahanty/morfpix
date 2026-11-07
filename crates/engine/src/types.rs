use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Jpeg,
    Png,
    WebP,
    Bmp,
    Gif,
    Avif,
    Jxl,
    Unknown,
}

impl ImageFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Png => "png",
            ImageFormat::WebP => "webp",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Gif => "gif",
            ImageFormat::Avif => "avif",
            ImageFormat::Jxl => "jxl",
            ImageFormat::Unknown => "bin",
        }
    }

    pub fn from_extension(ext: &str) -> Self {
        let clean = ext.trim_start_matches('.').to_lowercase();
        match clean.as_str() {
            "jpg" | "jpeg" | "jfif" => ImageFormat::Jpeg,
            "png" | "apng" => ImageFormat::Png,
            "webp" => ImageFormat::WebP,
            "bmp" => ImageFormat::Bmp,
            "gif" => ImageFormat::Gif,
            "avif" => ImageFormat::Avif,
            "jxl" => ImageFormat::Jxl,
            _ => ImageFormat::Unknown,
        }
    }

    pub fn is_supported(&self) -> bool {
        !matches!(self, ImageFormat::Unknown)
    }

    pub fn to_image_format(&self) -> Option<image::ImageFormat> {
        match self {
            ImageFormat::Jpeg => Some(image::ImageFormat::Jpeg),
            ImageFormat::Png => Some(image::ImageFormat::Png),
            ImageFormat::WebP => Some(image::ImageFormat::WebP),
            ImageFormat::Bmp => Some(image::ImageFormat::Bmp),
            ImageFormat::Gif => Some(image::ImageFormat::Gif),
            ImageFormat::Avif => Some(image::ImageFormat::Avif),
            _ => None,
        }
    }
}

impl From<image::ImageFormat> for ImageFormat {
    fn from(format: image::ImageFormat) -> Self {
        match format {
            image::ImageFormat::Jpeg => ImageFormat::Jpeg,
            image::ImageFormat::Png => ImageFormat::Png,
            image::ImageFormat::WebP => ImageFormat::WebP,
            image::ImageFormat::Bmp => ImageFormat::Bmp,
            image::ImageFormat::Gif => ImageFormat::Gif,
            image::ImageFormat::Avif => ImageFormat::Avif,
            _ => ImageFormat::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AvifBitDepth {
    Eight,
    Ten,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AvifColorModel {
    Ycbcr,
    Rgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AvifAlphaMode {
    Clean,
    Dirty,
    Premultiplied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AvifOptions {
    pub speed: u8,
    pub alpha_quality: Option<u8>,
    pub bit_depth: AvifBitDepth,
    pub color_model: AvifColorModel,
    pub threads: usize,
    pub alpha_mode: AvifAlphaMode,
}

impl Default for AvifOptions {
    fn default() -> Self {
        Self {
            speed: 4,
            alpha_quality: None,
            bit_depth: AvifBitDepth::Auto,
            color_model: AvifColorModel::Ycbcr,
            threads: 0,
            alpha_mode: AvifAlphaMode::Clean,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JpegSubsampling {
    #[default]
    Auto,
    #[serde(rename = "444")]
    S444,
    #[serde(rename = "422")]
    S422,
    #[serde(rename = "420")]
    S420,
    #[serde(rename = "440")]
    S440,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct JpegOptions {
    pub progressive: bool,
    pub subsampling: JpegSubsampling,
    pub smoothing: u8,
}

impl Default for JpegOptions {
    fn default() -> Self {
        Self {
            progressive: true,
            subsampling: JpegSubsampling::Auto,
            smoothing: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PngOptions {
    pub optimize: bool,
    pub level: u8,
    pub interlace: bool,
}

impl Default for PngOptions {
    fn default() -> Self {
        Self {
            optimize: false,
            level: 2,
            interlace: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebpOptions {
    pub lossless: bool,
    pub method: u8,
    pub exact: bool,
}

impl Default for WebpOptions {
    fn default() -> Self {
        Self {
            lossless: false,
            method: 4,
            exact: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct JxlOptions {
    pub effort: u8,
    pub threads: usize,
}

impl Default for JxlOptions {
    fn default() -> Self {
        Self {
            effort: 4,
            threads: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GifRepeat {
    Infinite,
    Finite(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GifOptions {
    pub speed: u8,
    pub repeat: Option<GifRepeat>,
}

impl Default for GifOptions {
    fn default() -> Self {
        Self {
            speed: 10,
            repeat: Some(GifRepeat::Infinite),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BmpBitsPerPixel {
    #[default]
    Auto,
    #[serde(rename = "8", alias = "bpp8")]
    Bpp8,
    #[serde(rename = "24", alias = "bpp24")]
    Bpp24,
    #[serde(rename = "32", alias = "bpp32")]
    Bpp32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BmpOptions {
    pub bpp: BmpBitsPerPixel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConversionOptions {
    pub format: Option<ImageFormat>,
    pub quality: u8,
    pub jpeg: JpegOptions,
    pub avif: AvifOptions,
    pub png: PngOptions,
    pub webp: WebpOptions,
    pub jxl: JxlOptions,
    pub gif: GifOptions,
    pub bmp: BmpOptions,
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            format: None,
            quality: 80,
            jpeg: JpegOptions::default(),
            avif: AvifOptions::default(),
            png: PngOptions::default(),
            webp: WebpOptions::default(),
            jxl: JxlOptions::default(),
            gif: GifOptions::default(),
            bmp: BmpOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub input_format: ImageFormat,
    pub output_format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub input_size_bytes: usize,
    pub output_size_bytes: usize,
    pub saved_bytes: i64,
    pub saved_percent: f32,
    pub duration_ms: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_partial_conversion_options() {
        let options: ConversionOptions = serde_json::from_str(r#"{"quality": 90}"#).unwrap();

        assert_eq!(options.format, None);
        assert_eq!(options.quality, 90);
        assert_eq!(options.avif.speed, 4);
        assert_eq!(options.avif.bit_depth, AvifBitDepth::Auto);
    }

    #[test]
    fn deserializes_partial_avif_options() {
        let options: ConversionOptions =
            serde_json::from_str(r#"{"avif": {"speed": 8, "color_model": "rgb"}}"#).unwrap();

        assert_eq!(options.quality, 80);
        assert_eq!(options.avif.speed, 8);
        assert_eq!(options.avif.color_model, AvifColorModel::Rgb);
        assert_eq!(options.avif.alpha_quality, None);
        assert_eq!(options.avif.threads, 0);
    }

    #[test]
    fn deserializes_partial_png_options() {
        let options: ConversionOptions =
            serde_json::from_str(r#"{"png": {"optimize": true, "level": 4, "interlace": true}}"#)
                .unwrap();

        assert_eq!(options.quality, 80);
        assert!(options.png.optimize);
        assert_eq!(options.png.level, 4);
        assert!(options.png.interlace);
    }

    #[test]
    fn deserializes_partial_jpeg_options() {
        let options: ConversionOptions = serde_json::from_str(
            r#"{"jpeg": {"progressive": false, "subsampling": "444", "smoothing": 20}}"#,
        )
        .unwrap();

        assert_eq!(options.quality, 80);
        assert!(!options.jpeg.progressive);
        assert_eq!(options.jpeg.subsampling, JpegSubsampling::S444);
        assert_eq!(options.jpeg.smoothing, 20);
    }

    #[test]
    fn deserializes_partial_webp_options() {
        let options: ConversionOptions =
            serde_json::from_str(r#"{"webp": {"lossless": true, "method": 6, "exact": true}}"#)
                .unwrap();

        assert_eq!(options.quality, 80);
        assert!(options.webp.lossless);
        assert_eq!(options.webp.method, 6);
        assert!(options.webp.exact);
    }

    #[test]
    fn deserializes_partial_jxl_options() {
        let options: ConversionOptions =
            serde_json::from_str(r#"{"jxl": {"effort": 7, "threads": 2}}"#).unwrap();

        assert_eq!(options.quality, 80);
        assert_eq!(options.jxl.effort, 7);
        assert_eq!(options.jxl.threads, 2);
    }

    #[test]
    fn deserializes_partial_gif_options() {
        let options: ConversionOptions =
            serde_json::from_str(r#"{"gif": {"speed": 5, "repeat": "infinite"}}"#).unwrap();

        assert_eq!(options.quality, 80);
        assert_eq!(options.gif.speed, 5);
        assert_eq!(options.gif.repeat, Some(GifRepeat::Infinite));

        let finite_options: ConversionOptions =
            serde_json::from_str(r#"{"gif": {"speed": 20, "repeat": {"finite": 3}}}"#).unwrap();
        assert_eq!(finite_options.gif.speed, 20);
        assert_eq!(finite_options.gif.repeat, Some(GifRepeat::Finite(3)));

        let none_options: ConversionOptions =
            serde_json::from_str(r#"{"gif": {"repeat": null}}"#).unwrap();
        assert_eq!(none_options.gif.speed, 10);
        assert_eq!(none_options.gif.repeat, None);
    }

    #[test]
    fn deserializes_partial_bmp_options() {
        let options: ConversionOptions = serde_json::from_str(r#"{"bmp": {"bpp": "24"}}"#).unwrap();
        assert_eq!(options.bmp.bpp, BmpBitsPerPixel::Bpp24);

        let options32: ConversionOptions =
            serde_json::from_str(r#"{"bmp": {"bpp": "32"}}"#).unwrap();
        assert_eq!(options32.bmp.bpp, BmpBitsPerPixel::Bpp32);

        let options8: ConversionOptions = serde_json::from_str(r#"{"bmp": {"bpp": "8"}}"#).unwrap();
        assert_eq!(options8.bmp.bpp, BmpBitsPerPixel::Bpp8);

        let options_auto: ConversionOptions =
            serde_json::from_str(r#"{"bmp": {"bpp": "auto"}}"#).unwrap();
        assert_eq!(options_auto.bmp.bpp, BmpBitsPerPixel::Auto);

        let options_default: ConversionOptions = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(options_default.bmp.bpp, BmpBitsPerPixel::Auto);
    }

    #[test]
    fn parses_extension_with_leading_dot() {
        assert_eq!(ImageFormat::from_extension(".png"), ImageFormat::Png);
        assert_eq!(ImageFormat::from_extension(".WEBP"), ImageFormat::WebP);
        assert_eq!(ImageFormat::from_extension(".avif"), ImageFormat::Avif);
        assert_eq!(ImageFormat::from_extension(".jpg"), ImageFormat::Jpeg);
        assert_eq!(ImageFormat::from_extension(".jxl"), ImageFormat::Jxl);
        assert_eq!(ImageFormat::from_extension("png"), ImageFormat::Png);
    }
}
