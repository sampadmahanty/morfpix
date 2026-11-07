use crate::types::ImageFormat;

pub fn detect_format(data: &[u8]) -> ImageFormat {
    if is_jxl(data) {
        return ImageFormat::Jxl;
    }
    if oxideav_avif::probe(data) {
        return ImageFormat::Avif;
    }
    image::guess_format(data)
        .map(ImageFormat::from)
        .unwrap_or(ImageFormat::Unknown)
}

pub(crate) fn is_jxl(data: &[u8]) -> bool {
    const CONTAINER_SIGNATURE: [u8; 12] = [
        0x00, 0x00, 0x00, 0x0C, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A,
    ];
    data.starts_with(&[0xFF, 0x0A]) || data.starts_with(&CONTAINER_SIGNATURE)
}
