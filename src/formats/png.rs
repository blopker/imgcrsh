//! PNG encoding using oxipng
//!
//! Provides optimized PNG compression with:
//! - Multi-threaded optimization
//! - Configurable compression levels
//! - ICC profile injection
//! - Alpha channel preservation
//! - Lossy mode via imagequant (pngquant) with full RGBA support

use super::Encoder;
use anyhow::{Context, Result};
use imagequant::{Attributes, RGBA};
use oxipng::{BitDepth, ColorType, Options, RGBA8, RawImage};

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
    /// Lossless mode (true) or lossy quantization (false)
    pub lossless: bool,
    /// Lossy quality (0-100, higher = better colors, larger file)
    /// Only used when lossless = false
    pub quality: u8,
}

impl Default for PngConfig {
    fn default() -> Self {
        Self {
            optimization_level: 2,
            strip_metadata: true,
            interlace: false,
            lossless: true,
            quality: 90, // High quality default for minimal color loss
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

impl PngEncoder {
    /// Quantize RGBA image to 256-color palette using imagequant
    /// Full RGBA support including semi-transparent pixels
    fn quantize_to_palette(
        rgba: &[u8],
        width: u32,
        height: u32,
        quality: u8,
    ) -> Result<(Vec<u8>, Vec<RGBA8>)> {
        // Convert raw bytes to RGBA pixels for imagequant
        let pixels: Vec<RGBA> = rgba
            .chunks_exact(4)
            .map(|c| RGBA::new(c[0], c[1], c[2], c[3]))
            .collect();

        // Create quantization attributes with quality settings
        let mut attr = Attributes::new();

        // Quality range: min quality allows more compression, max quality is target
        // Higher values = better color accuracy, larger files
        let min_quality = quality.saturating_sub(20).max(0);
        attr.set_quality(min_quality, quality)
            .map_err(|e| anyhow::anyhow!("Failed to set quality: {}", e))?;

        // Slower speed = better quality (1 = best, 10 = fastest)
        // Use speed 3 for good quality/speed balance
        attr.set_speed(3)
            .map_err(|e| anyhow::anyhow!("Failed to set speed: {}", e))?;

        // Create image from RGBA pixels
        let mut img = attr
            .new_image(&pixels[..], width as usize, height as usize, 0.0)
            .map_err(|e| anyhow::anyhow!("Failed to create image: {}", e))?;

        // Quantize to generate palette
        let mut result = attr
            .quantize(&mut img)
            .map_err(|e| anyhow::anyhow!("Quantization failed: {}", e))?;

        // Enable dithering for smooth gradients
        result
            .set_dithering_level(1.0)
            .map_err(|e| anyhow::anyhow!("Failed to set dithering: {}", e))?;

        // Remap pixels to palette indices
        let (palette, indices) = result
            .remapped(&mut img)
            .map_err(|e| anyhow::anyhow!("Remapping failed: {}", e))?;

        // Convert imagequant RGBA to oxipng RGBA8
        let rgba_palette: Vec<RGBA8> = palette
            .iter()
            .map(|c| RGBA8::new(c.r, c.g, c.b, c.a))
            .collect();

        Ok((indices, rgba_palette))
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
        // Build options
        let opts = Self::build_options(config);

        if config.lossless {
            // Lossless path: full RGBA
            let mut raw = RawImage::new(
                width,
                height,
                ColorType::RGBA,
                BitDepth::Eight,
                rgba.to_vec(),
            )
            .context("Failed to create raw PNG image")?;

            if let Some(icc) = icc_profile {
                raw.add_icc_profile(icc);
            }

            let output = raw
                .create_optimized_png(&opts)
                .context("Failed to create optimized PNG")?;

            Ok(output)
        } else {
            // Lossy path: quantize to 256-color palette
            let (indices, palette) = Self::quantize_to_palette(rgba, width, height, config.quality)?;

            let mut raw = RawImage::new(
                width,
                height,
                ColorType::Indexed { palette },
                BitDepth::Eight,
                indices,
            )
            .context("Failed to create indexed PNG image")?;

            if let Some(icc) = icc_profile {
                raw.add_icc_profile(icc);
            }

            let output = raw
                .create_optimized_png(&opts)
                .context("Failed to create optimized PNG")?;

            Ok(output)
        }
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

    #[test]
    fn test_png_lossy_quantization() {
        // Create a gradient image with many colors (16x16 = 256 pixels)
        let rgba: Vec<u8> = (0..256)
            .flat_map(|i| {
                let r = (i % 16) * 16;
                let g = (i / 16) * 16;
                let b = 128;
                [r as u8, g as u8, b, 255]
            })
            .collect();

        // Lossy mode should produce palette-based PNG
        let mut config = PngConfig::default();
        config.lossless = false;
        let lossy_output = PngEncoder::encode(&rgba, 16, 16, &config, None).unwrap();

        // Lossless mode for comparison
        config.lossless = true;
        let lossless_output = PngEncoder::encode(&rgba, 16, 16, &config, None).unwrap();

        // Both should be valid PNGs
        assert_eq!(
            &lossy_output[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
        assert_eq!(
            &lossless_output[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );

        // For this test image, lossy might be smaller or similar
        // (small images may not show much difference)
        assert!(!lossy_output.is_empty());
        assert!(!lossless_output.is_empty());
    }
}
