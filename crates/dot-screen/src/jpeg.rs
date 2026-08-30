//! Turns captured RGBA pixels into the JPEG that travels with a question.
//! Must not resize; the capture already has its final size.

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder};

use crate::ScreenError;

const JPEG_QUALITY: u8 = 85;

pub(crate) fn encode_jpeg(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ScreenError> {
    // The JPEG encoder rejects RGBA outright; a screen has no transparency to lose.
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY).write_image(
        &rgb,
        width,
        height,
        ExtendedColorType::Rgb8,
    )?;
    Ok(jpeg)
}

#[cfg(test)]
mod tests {
    use super::encode_jpeg;

    #[test]
    fn rgba_pixels_become_a_jpeg_of_the_same_size() {
        let (width, height) = (40, 30);
        let rgba: Vec<u8> = (0..width * height)
            .flat_map(|pixel| [pixel as u8, 90, 200, 255])
            .collect();

        let jpeg = encode_jpeg(&rgba, width, height).unwrap();

        assert_eq!(&jpeg[..3], &[0xFF, 0xD8, 0xFF]);
        let decoded = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (width, height));
    }
}
