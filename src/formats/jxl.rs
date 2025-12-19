//! JPEG XL encoding using jpegxl-rs (libjxl)
//!
//! Provides JPEG XL compression with:
//! - Lossy mode with configurable quality (distance)
//! - Lossless mode
//! - Effort setting for encode time vs compression tradeoff

use super::Encoder;
use anyhow::Result;
use jpegxl_rs::encode::{EncoderFrame, EncoderSpeed};
use jpegxl_rs::encoder_builder;

/// JPEG XL encoding configuration
#[derive(Debug, Clone)]
pub struct JxlConfig {
    /// Enable lossless mode
    pub lossless: bool,
    /// Quality level 0-100 (converted to distance internally)
    /// Only used when lossless = false
    pub quality: u8,
    /// Encoding effort 1-10 (1 = fastest, 10 = slowest/best)
    pub effort: u8,
}

impl Default for JxlConfig {
    fn default() -> Self {
        Self {
            lossless: false,
            quality: 80,
            effort: 7, // Squirrel - good balance
        }
    }
}

impl JxlConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = quality.clamp(0, 100);
        self
    }

    pub fn with_lossless(mut self, lossless: bool) -> Self {
        self.lossless = lossless;
        self
    }

    pub fn with_effort(mut self, effort: u8) -> Self {
        self.effort = effort.clamp(1, 10);
        self
    }
}

/// Convert effort level (1-10) to EncoderSpeed
fn effort_to_speed(effort: u8) -> EncoderSpeed {
    match effort {
        1 => EncoderSpeed::Lightning,
        2 => EncoderSpeed::Thunder,
        3 => EncoderSpeed::Falcon,
        4 => EncoderSpeed::Cheetah,
        5 => EncoderSpeed::Hare,
        6 => EncoderSpeed::Wombat,
        7 => EncoderSpeed::Squirrel,
        8 => EncoderSpeed::Kitten,
        9 => EncoderSpeed::Tortoise,
        _ => EncoderSpeed::Glacier,
    }
}

/// Convert quality (0-100) to JPEG XL distance (0-15)
/// Distance 0 = lossless, 1.0 = visually lossless, higher = more lossy
fn quality_to_distance(quality: u8) -> f32 {
    // Map 100 -> 0.0 (best), 0 -> 15.0 (worst)
    // Use a curve that gives good results in the 70-90 range
    let q = quality.clamp(0, 100) as f32;
    if q >= 100.0 {
        0.0
    } else if q >= 90.0 {
        // 90-100 maps to 0.0-1.0 (visually lossless range)
        (100.0 - q) / 10.0
    } else {
        // 0-90 maps to 1.0-15.0
        1.0 + (90.0 - q) / 90.0 * 14.0
    }
}

/// JPEG XL encoder using libjxl
pub struct JxlEncoder;

impl Encoder for JxlEncoder {
    type Config = JxlConfig;

    fn encode(
        rgba: &[u8],
        width: u32,
        height: u32,
        config: &Self::Config,
        _icc_profile: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        // Build encoder with settings
        let mut encoder = encoder_builder()
            .lossless(config.lossless)
            .uses_original_profile(config.lossless) // Required for lossless mode
            .speed(effort_to_speed(config.effort))
            .has_alpha(true)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create JXL encoder: {:?}", e))?;

        // Set quality (distance) for lossy mode
        if !config.lossless {
            encoder.quality = quality_to_distance(config.quality);
        }

        // Create frame from RGBA data (4 channels for RGBA)
        let frame = EncoderFrame::new(rgba).num_channels(4);

        // Encode (specify u8 as the output pixel type)
        let result: jpegxl_rs::encode::EncoderResult<u8> = encoder
            .encode_frame(&frame, width, height)
            .map_err(|e| anyhow::anyhow!("JXL encoding failed: {:?}", e))?;

        Ok(result.data)
    }

    fn extension() -> &'static str {
        "jxl"
    }

    fn mime_type() -> &'static str {
        "image/jxl"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jxl_config_defaults() {
        let config = JxlConfig::default();
        assert_eq!(config.quality, 80);
        assert_eq!(config.effort, 7);
        assert!(!config.lossless);
    }

    #[test]
    fn test_quality_to_distance() {
        assert_eq!(quality_to_distance(100), 0.0);
        assert!(quality_to_distance(95) < 1.0);
        assert!(quality_to_distance(90) <= 1.0);
        assert!(quality_to_distance(50) > 1.0);
        assert!(quality_to_distance(0) <= 15.0);
    }

    #[test]
    fn test_jxl_encode_small_image() {
        // Create a small red image (4x4)
        let rgba: Vec<u8> = (0..16)
            .flat_map(|_| [255, 0, 0, 255]) // Red pixels
            .collect();

        let config = JxlConfig::new().with_effort(1); // Fast for tests
        let output = JxlEncoder::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid JXL
        assert!(output.len() > 2);
        // JXL files start with 0xFF 0x0A (naked codestream) or have ISOBMFF container
        assert!(
            (output[0] == 0xFF && output[1] == 0x0A)
                || (output[4..8] == *b"ftyp" || output[4..8] == *b"JXL ")
        );
    }

    #[test]
    fn test_jxl_lossless() {
        // Create a small gradient image (4x4)
        let rgba: Vec<u8> = (0..16)
            .flat_map(|i| [(i * 16) as u8, 0, 0, 255])
            .collect();

        let config = JxlConfig::new().with_lossless(true).with_effort(1);
        let output = JxlEncoder::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid JXL
        assert!(!output.is_empty());
    }
}
