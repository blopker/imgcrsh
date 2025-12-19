//! WebP encoding using libwebp
//!
//! Provides WebP compression with:
//! - Lossy mode with configurable quality
//! - Lossless mode (VP8L)
//! - Full alpha channel support
//! - ICC profile embedding via VP8X extended format

use super::Encoder;
use anyhow::{ensure, Result};

/// WebP encoding configuration
#[derive(Debug, Clone)]
pub struct WebpConfig {
    /// Enable lossless mode (VP8L)
    pub lossless: bool,
    /// Quality level 0-100 (ignored if lossless)
    /// Higher = better quality, larger file
    pub quality: u8,
}

impl Default for WebpConfig {
    fn default() -> Self {
        Self {
            lossless: false,
            quality: 80,
        }
    }
}

impl WebpConfig {
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
}

/// WebP encoder using libwebp
pub struct WebpEncoder;

impl Encoder for WebpEncoder {
    type Config = WebpConfig;

    fn encode(
        rgba: &[u8],
        width: u32,
        height: u32,
        config: &Self::Config,
        icc_profile: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        // Create encoder from RGBA data
        let encoder = webp::Encoder::from_rgba(rgba, width, height);

        // Encode based on mode
        let encoded = if config.lossless {
            encoder.encode_lossless()
        } else {
            // Use advanced encoding with method 6 for best compression
            let mut webp_config = webp::WebPConfig::new()
                .map_err(|()| anyhow::anyhow!("Failed to create WebP config"))?;
            webp_config.quality = config.quality as f32;
            webp_config.method = 6; // Slowest but best compression

            encoder
                .encode_advanced(&webp_config)
                .map_err(|e| anyhow::anyhow!("WebP encoding failed: {:?}", e))?
        };

        // Inject ICC profile if provided
        let output = if let Some(icc) = icc_profile {
            inject_icc_profile(&encoded, icc, width, height)?
        } else {
            encoded.to_vec()
        };

        Ok(output)
    }

    fn extension() -> &'static str {
        "webp"
    }

    fn mime_type() -> &'static str {
        "image/webp"
    }
}

/// Inject ICC profile into WebP by converting to VP8X extended format
///
/// WebP extended format structure:
/// - RIFF header (8 bytes): "RIFF" + file size
/// - "WEBP" (4 bytes)
/// - VP8X chunk (18 bytes): extended format header with flags
/// - ICCP chunk: "ICCP" + size + ICC data (+ padding if odd)
/// - Original image chunk (VP8/VP8L)
fn inject_icc_profile(webp: &[u8], icc_profile: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    ensure!(webp.len() >= 12, "Invalid WebP: too short");
    ensure!(&webp[0..4] == b"RIFF", "Invalid WebP: missing RIFF");
    ensure!(&webp[8..12] == b"WEBP", "Invalid WebP: missing WEBP");

    // Find the image chunk (VP8, VP8L, or VP8X)
    let chunk_start = 12;
    let chunk_id = &webp[chunk_start..chunk_start + 4];

    // If already VP8X, we'd need more complex handling - for now just handle simple cases
    ensure!(
        chunk_id == b"VP8 " || chunk_id == b"VP8L",
        "WebP already has extended format, ICC injection not supported"
    );

    // Build VP8X chunk
    // Flags: bit 5 (0x20) = ICCP present
    let flags: u32 = 0x20;
    let canvas_width = width - 1; // VP8X stores width-1
    let canvas_height = height - 1;

    let mut vp8x = Vec::with_capacity(18);
    vp8x.extend_from_slice(b"VP8X");
    vp8x.extend_from_slice(&10u32.to_le_bytes()); // Chunk size (10 bytes of data)
    vp8x.extend_from_slice(&flags.to_le_bytes());
    // Canvas width (24 bits, little-endian)
    vp8x.push((canvas_width & 0xFF) as u8);
    vp8x.push(((canvas_width >> 8) & 0xFF) as u8);
    vp8x.push(((canvas_width >> 16) & 0xFF) as u8);
    // Canvas height (24 bits, little-endian)
    vp8x.push((canvas_height & 0xFF) as u8);
    vp8x.push(((canvas_height >> 8) & 0xFF) as u8);
    vp8x.push(((canvas_height >> 16) & 0xFF) as u8);

    // Build ICCP chunk
    let icc_size = icc_profile.len() as u32;
    let icc_padded = !icc_size.is_multiple_of(2); // RIFF chunks must be even-aligned

    let mut iccp = Vec::with_capacity(8 + icc_profile.len() + if icc_padded { 1 } else { 0 });
    iccp.extend_from_slice(b"ICCP");
    iccp.extend_from_slice(&icc_size.to_le_bytes());
    iccp.extend_from_slice(icc_profile);
    if icc_padded {
        iccp.push(0); // Padding byte
    }

    // Calculate new RIFF size
    // Original: 4 (WEBP) + image_chunk
    // New: 4 (WEBP) + VP8X (18) + ICCP (8 + icc_len + padding) + image_chunk
    let image_chunk = &webp[12..];
    let new_riff_size = 4 + vp8x.len() + iccp.len() + image_chunk.len();

    // Build output
    let mut output = Vec::with_capacity(8 + new_riff_size);
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&(new_riff_size as u32).to_le_bytes());
    output.extend_from_slice(b"WEBP");
    output.extend_from_slice(&vp8x);
    output.extend_from_slice(&iccp);
    output.extend_from_slice(image_chunk);

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webp_config_defaults() {
        let config = WebpConfig::default();
        assert_eq!(config.quality, 80);
        assert!(!config.lossless);
    }

    #[test]
    fn test_webp_encode_lossy() {
        // Create a small red image (4x4)
        let rgba: Vec<u8> = (0..16)
            .flat_map(|_| [255, 0, 0, 255]) // Red pixels
            .collect();

        let config = WebpConfig::default();
        let output = WebpEncoder::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid WebP (starts with RIFF....WEBP)
        assert!(output.len() > 12);
        assert_eq!(&output[0..4], b"RIFF");
        assert_eq!(&output[8..12], b"WEBP");
    }

    #[test]
    fn test_webp_encode_lossless() {
        // Create a small gradient image (4x4)
        let rgba: Vec<u8> = (0..16)
            .flat_map(|i| [(i * 16) as u8, 0, 0, 255])
            .collect();

        let config = WebpConfig::new().with_lossless(true);
        let output = WebpEncoder::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid WebP
        assert!(output.len() > 12);
        assert_eq!(&output[0..4], b"RIFF");
        assert_eq!(&output[8..12], b"WEBP");
    }

    #[test]
    fn test_webp_with_alpha() {
        // Create image with varying alpha
        let rgba: Vec<u8> = (0..16)
            .flat_map(|i| [255, 0, 0, (i * 16) as u8])
            .collect();

        let config = WebpConfig::default();
        let output = WebpEncoder::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid WebP with alpha
        assert!(!output.is_empty());
    }
}
