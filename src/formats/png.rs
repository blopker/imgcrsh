//! PNG encoding using oxipng
//!
//! Provides optimized PNG compression with:
//! - Multi-threaded optimization
//! - Configurable compression levels
//! - ICC profile injection
//! - Alpha channel preservation

use super::Encoder;
use anyhow::{Context, Result};
use oxipng::{BitDepth, ColorType, Options, RawImage};

/// PNG encoding configuration
#[derive(Debug, Clone)]
pub struct PngConfig {
    /// Optimization level (0-6, higher = slower but better compression)
    /// 0 = fast, minimal optimization
    /// 2 = default, good balance
    /// 6 = maximum, exhaustive filtering
    pub optimization_level: u8,
    /// Strip metadata chunks (keeps only essential data)
    pub strip_metadata: bool,
    /// Enable interlacing (Adam7)
    pub interlace: bool,
    // Set lossless optimizations
    pub lossless: bool,
}

impl Default for PngConfig {
    fn default() -> Self {
        Self {
            optimization_level: 2,
            strip_metadata: true,
            interlace: false,
            lossless: true,
        }
    }
}

impl PngConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_optimization_level(mut self, level: u8) -> Self {
        self.optimization_level = level.min(6);
        self
    }

    pub fn with_strip_metadata(mut self, strip: bool) -> Self {
        self.strip_metadata = strip;
        self
    }

    pub fn with_interlace(mut self, interlace: bool) -> Self {
        self.interlace = interlace;
        self
    }
}

/// PNG encoder using oxipng
pub struct PngEncoder;

impl PngEncoder {
    /// Build oxipng Options from our config
    fn build_options(config: &PngConfig) -> Options {
        // Configure based on optimization level
        // Higher levels use more filter strategies and compression effort
        let mut opts = Options::from_preset(config.optimization_level.clamp(0, 6));

        // Strip metadata if requested
        if config.strip_metadata {
            opts.strip = oxipng::StripChunks::Safe;
        }

        opts.interlace = Some(config.interlace);
        opts.force = true;

        if !config.lossless {
            opts.optimize_alpha = true;
            opts.scale_16 = true;
        }

        opts
    }
}

impl Encoder for PngEncoder {
    type Config = PngConfig;

    fn encode(
        rgba: &[u8],
        width: u32,
        height: u32,
        config: &Self::Config,
        icc_profile: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        // Create RawImage from RGBA data
        let mut raw = RawImage::new(
            width,
            height,
            ColorType::RGBA,
            BitDepth::Eight,
            rgba.to_vec(),
        )
        .context("Failed to create raw PNG image")?;

        // Add ICC profile if provided
        if let Some(icc) = icc_profile {
            raw.add_icc_profile(icc);
        }

        // Build options
        let opts = Self::build_options(config);

        // Create optimized PNG
        let output = raw
            .create_optimized_png(&opts)
            .context("Failed to create optimized PNG")?;

        Ok(output)
    }

    fn extension() -> &'static str {
        "png"
    }

    fn mime_type() -> &'static str {
        "image/png"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_png_config_defaults() {
        let config = PngConfig::default();
        assert_eq!(config.optimization_level, 2);
        assert!(config.strip_metadata);
        assert!(!config.interlace);
    }

    #[test]
    fn test_png_encode_small_image() {
        // Create a small red image (4x4)
        let rgba: Vec<u8> = (0..16)
            .flat_map(|_| [255, 0, 0, 255]) // Red pixels
            .collect();

        let config = PngConfig::default();
        let output = PngEncoder::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid PNG (starts with PNG signature)
        assert!(output.len() > 8);
        assert_eq!(
            &output[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }

    #[test]
    fn test_png_with_transparency() {
        // Create image with semi-transparent pixels
        let rgba: Vec<u8> = (0..16)
            .flat_map(|i| [255, 0, 0, (i * 16) as u8]) // Varying alpha
            .collect();

        let config = PngConfig::default();
        let output = PngEncoder::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid PNG
        assert!(!output.is_empty());
    }
}
