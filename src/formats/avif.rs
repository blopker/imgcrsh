//! AVIF encoding using ravif (rav1e)
//!
//! Provides AVIF compression with:
//! - Lossy mode with configurable quality
//! - Speed/effort setting for encode time vs compression tradeoff
//! - Full alpha channel support

use super::Encoder;
use anyhow::Result;
use ravif::{Img, RGBA8};

/// AVIF encoding configuration
#[derive(Debug, Clone)]
pub struct AvifConfig {
    /// Quality level 0-100 (higher = better quality, larger file)
    pub quality: u8,
    /// Encoding speed 1-10 (1 = slowest/best, 10 = fastest/worst)
    pub speed: u8,
}

impl Default for AvifConfig {
    fn default() -> Self {
        Self {
            quality: 80,
            speed: 4, // Balanced default
        }
    }
}

impl AvifConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = quality.clamp(0, 100);
        self
    }

    pub fn with_speed(mut self, speed: u8) -> Self {
        self.speed = speed.clamp(1, 10);
        self
    }
}

/// AVIF encoder using ravif
pub struct AvifEncoder;

impl Encoder for AvifEncoder {
    type Config = AvifConfig;

    fn encode(
        rgba: &[u8],
        width: u32,
        height: u32,
        config: &Self::Config,
        _icc_profile: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        // Convert &[u8] to &[RGBA8] for ravif
        let pixels: &[RGBA8] = bytemuck_cast_slice(rgba);

        // Create image reference
        let img = Img::new(pixels, width as usize, height as usize);

        // Create encoder with settings
        let encoder = ravif::Encoder::new()
            .with_quality(config.quality as f32)
            .with_speed(config.speed);

        // Encode
        let result = encoder
            .encode_rgba(img)
            .map_err(|e| anyhow::anyhow!("AVIF encoding failed: {}", e))?;

        Ok(result.avif_file)
    }

    fn extension() -> &'static str {
        "avif"
    }

    fn mime_type() -> &'static str {
        "image/avif"
    }
}

/// Cast &[u8] to &[RGBA8] without copying
/// Safety: RGBA8 is repr(C) with 4 u8 fields, so this is safe
fn bytemuck_cast_slice(bytes: &[u8]) -> &[RGBA8] {
    assert!(
        bytes.len().is_multiple_of(4),
        "Input must be RGBA (4 bytes per pixel)"
    );
    // SAFETY: RGBA8 is #[repr(C)] and contains exactly 4 u8 values
    // The alignment of RGBA8 is 1 (same as u8), and we've verified length is divisible by 4
    unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const RGBA8, bytes.len() / 4) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avif_config_defaults() {
        let config = AvifConfig::default();
        assert_eq!(config.quality, 80);
        assert_eq!(config.speed, 4);
    }

    #[test]
    fn test_avif_encode_small_image() {
        // Create a small red image (4x4)
        let rgba: Vec<u8> = (0..16)
            .flat_map(|_| [255, 0, 0, 255]) // Red pixels
            .collect();

        let config = AvifConfig::new().with_speed(10); // Fast for tests
        let output = AvifEncoder::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid AVIF (starts with ftyp box)
        assert!(output.len() > 12);
        // AVIF files start with a size field then "ftyp"
        assert_eq!(&output[4..8], b"ftyp");
    }

    #[test]
    fn test_avif_with_alpha() {
        // Create image with varying alpha
        let rgba: Vec<u8> = (0..16).flat_map(|i| [255, 0, 0, (i * 16) as u8]).collect();

        let config = AvifConfig::new().with_speed(10);
        let output = AvifEncoder::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid AVIF
        assert!(!output.is_empty());
    }
}
