//! GIF encoding using imagequant + gif crate
//!
//! Provides GIF compression with:
//! - High-quality color quantization via imagequant (pngquant)
//! - Binary transparency support
//! - Dithering for smooth gradients

use super::Encoder;
use anyhow::{Context, Result};
use gif::{Encoder as GifEncoder, Frame, Repeat};
use imagequant::{Attributes, RGBA};
use std::io::Cursor;

/// GIF encoding configuration
#[derive(Debug, Clone)]
pub struct GifConfig {
    /// Quality level 0-100 (controls color quantization)
    /// Higher = better colors, but GIF is always limited to 256
    pub quality: u8,
    /// Alpha threshold for transparency (0-255)
    /// Pixels with alpha below this become fully transparent
    pub alpha_threshold: u8,
}

impl Default for GifConfig {
    fn default() -> Self {
        Self {
            quality: 80,
            alpha_threshold: 128, // 50% alpha threshold
        }
    }
}

impl GifConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = quality.clamp(0, 100);
        self
    }

    pub fn with_alpha_threshold(mut self, threshold: u8) -> Self {
        self.alpha_threshold = threshold;
        self
    }
}

/// GIF encoder using imagequant for quantization
pub struct GifEncoderImpl;

impl GifEncoderImpl {
    /// Quantize RGBA image to 256-color palette with transparency support
    fn quantize_with_transparency(
        rgba: &[u8],
        width: u32,
        height: u32,
        quality: u8,
        alpha_threshold: u8,
    ) -> Result<(Vec<u8>, Vec<u8>, Option<u8>)> {
        // Convert raw bytes to RGBA pixels for imagequant
        // Apply alpha threshold: below threshold becomes fully transparent
        let pixels: Vec<RGBA> = rgba
            .chunks_exact(4)
            .map(|c| {
                if c[3] < alpha_threshold {
                    RGBA::new(0, 0, 0, 0) // Fully transparent
                } else {
                    RGBA::new(c[0], c[1], c[2], 255) // Fully opaque
                }
            })
            .collect();

        // Create quantization attributes
        let mut attr = Attributes::new();

        // Quality range
        let min_quality = quality.saturating_sub(20);
        attr.set_quality(min_quality, quality)
            .map_err(|e| anyhow::anyhow!("Failed to set quality: {}", e))?;

        // Speed 3 for good quality/speed balance
        attr.set_speed(3)
            .map_err(|e| anyhow::anyhow!("Failed to set speed: {}", e))?;

        // Create image
        let mut img = attr
            .new_image(&pixels[..], width as usize, height as usize, 0.0)
            .map_err(|e| anyhow::anyhow!("Failed to create image: {}", e))?;

        // Quantize
        let mut result = attr
            .quantize(&mut img)
            .map_err(|e| anyhow::anyhow!("Quantization failed: {}", e))?;

        // Enable dithering
        result
            .set_dithering_level(1.0)
            .map_err(|e| anyhow::anyhow!("Failed to set dithering: {}", e))?;

        // Remap to indices
        let (palette, indices) = result
            .remapped(&mut img)
            .map_err(|e| anyhow::anyhow!("Remapping failed: {}", e))?;

        // Convert palette to RGB (GIF format: [r,g,b,r,g,b,...])
        // Also find transparent color index (if any palette entry has alpha = 0)
        let mut rgb_palette = Vec::with_capacity(palette.len() * 3);
        let mut transparent_index: Option<u8> = None;

        for (i, color) in palette.iter().enumerate() {
            rgb_palette.push(color.r);
            rgb_palette.push(color.g);
            rgb_palette.push(color.b);

            // First fully transparent color becomes the transparent index
            if color.a == 0 && transparent_index.is_none() {
                transparent_index = Some(i as u8);
            }
        }

        Ok((indices, rgb_palette, transparent_index))
    }
}

impl Encoder for GifEncoderImpl {
    type Config = GifConfig;

    fn encode(
        rgba: &[u8],
        width: u32,
        height: u32,
        config: &Self::Config,
        _icc_profile: Option<&[u8]>, // GIF doesn't support ICC profiles
    ) -> Result<Vec<u8>> {
        // GIF dimensions are u16
        let w = width.min(65535) as u16;
        let h = height.min(65535) as u16;

        // Quantize with transparency
        let (indices, palette, transparent) =
            Self::quantize_with_transparency(rgba, width, height, config.quality, config.alpha_threshold)?;

        // Create output buffer
        let mut output = Cursor::new(Vec::new());

        // Create GIF encoder with global palette
        let mut encoder = GifEncoder::new(&mut output, w, h, &palette)
            .context("Failed to create GIF encoder")?;

        // Set to not repeat (single image, not animation)
        encoder
            .set_repeat(Repeat::Finite(0))
            .context("Failed to set repeat")?;

        // Create frame from indexed pixels
        let frame = Frame::from_palette_pixels(w, h, indices, palette, transparent);

        // Write frame
        encoder
            .write_frame(&frame)
            .context("Failed to write GIF frame")?;

        // Get output bytes
        drop(encoder);
        Ok(output.into_inner())
    }

    fn extension() -> &'static str {
        "gif"
    }

    fn mime_type() -> &'static str {
        "image/gif"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gif_config_defaults() {
        let config = GifConfig::default();
        assert_eq!(config.quality, 80);
        assert_eq!(config.alpha_threshold, 128);
    }

    #[test]
    fn test_gif_encode_small_image() {
        // Create a small red image (4x4)
        let rgba: Vec<u8> = (0..16)
            .flat_map(|_| [255, 0, 0, 255]) // Red pixels
            .collect();

        let config = GifConfig::default();
        let output = GifEncoderImpl::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid GIF (starts with GIF89a or GIF87a)
        assert!(output.len() > 6);
        assert_eq!(&output[0..3], b"GIF");
    }

    #[test]
    fn test_gif_with_transparency() {
        // Create image with some transparent pixels
        let mut rgba: Vec<u8> = Vec::new();
        for i in 0..16 {
            if i < 8 {
                rgba.extend_from_slice(&[255, 0, 0, 255]); // Opaque red
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]); // Transparent
            }
        }

        let config = GifConfig::default();
        let output = GifEncoderImpl::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid GIF
        assert!(!output.is_empty());
        assert_eq!(&output[0..3], b"GIF");
    }

    #[test]
    fn test_gif_gradient() {
        // Create a gradient (tests dithering)
        let rgba: Vec<u8> = (0..256)
            .flat_map(|i| {
                let v = i as u8;
                [v, v, v, 255]
            })
            .collect();

        let config = GifConfig::default();
        let output = GifEncoderImpl::encode(&rgba, 16, 16, &config, None).unwrap();

        assert!(!output.is_empty());
        assert_eq!(&output[0..3], b"GIF");
    }
}
