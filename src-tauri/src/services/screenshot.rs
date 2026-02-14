use anyhow::Result;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::DynamicImage;
use std::io::Cursor;
use xcap::Monitor;

const TARGET_WIDTH: u32 = 1280;
const TARGET_HEIGHT: u32 = 720;
const HASH_SIZE: u32 = 8;

/// A simple perceptual hash (dHash) computed manually to avoid image crate version conflicts.
#[derive(Debug, Clone)]
pub struct DHash {
    pub bits: u64,
}

impl DHash {
    /// Compute a difference hash (dHash) from a DynamicImage.
    /// Resizes to 9x8 grayscale, then compares adjacent pixels.
    pub fn compute(img: &DynamicImage) -> Self {
        let small = img.resize_exact(HASH_SIZE + 1, HASH_SIZE, FilterType::Lanczos3);
        let gray = small.to_luma8();

        let mut bits: u64 = 0;
        for y in 0..HASH_SIZE {
            for x in 0..HASH_SIZE {
                let left = gray.get_pixel(x, y)[0];
                let right = gray.get_pixel(x + 1, y)[0];
                if left > right {
                    bits |= 1 << (y * HASH_SIZE + x);
                }
            }
        }

        DHash { bits }
    }

    /// Hamming distance between two hashes.
    pub fn distance(&self, other: &DHash) -> u32 {
        (self.bits ^ other.bits).count_ones()
    }

    /// Convert to a hex string for storage.
    pub fn to_hex(&self) -> String {
        format!("{:016x}", self.bits)
    }
}

pub struct ScreenshotCapture;

impl ScreenshotCapture {
    /// Capture the primary monitor and return resized JPEG bytes + perceptual hash.
    pub fn capture() -> Result<(Vec<u8>, DHash)> {
        let monitors = Monitor::all()?;
        let monitor = monitors
            .into_iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .or_else(|| Monitor::all().ok()?.into_iter().next())
            .ok_or_else(|| anyhow::anyhow!("No monitor found"))?;

        let raw_image = monitor.capture_image()?;

        let dynamic = DynamicImage::ImageRgba8(raw_image);
        let resized = dynamic.resize(TARGET_WIDTH, TARGET_HEIGHT, FilterType::Lanczos3);

        // Compute perceptual hash for dedup
        let hash = DHash::compute(&resized);

        // Encode as JPEG
        let mut jpeg_buf = Cursor::new(Vec::new());
        let encoder = JpegEncoder::new_with_quality(&mut jpeg_buf, 85);
        resized.to_rgb8().write_with_encoder(encoder)?;

        Ok((jpeg_buf.into_inner(), hash))
    }
}
