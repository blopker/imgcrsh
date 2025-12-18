//! JPEG encoding using mozjpeg
//!
//! Provides high-quality JPEG compression with:
//! - Progressive scan encoding
//! - Configurable chroma subsampling
//! - Trellis quantization
//! - ICC profile injection

use super::Encoder;
use anyhow::Result;

/// Maximum size of ICC profile data per APP2 segment (64KB - overhead)
const ICC_CHUNK_SIZE: usize = 65519;
/// ICC profile marker identifier
const ICC_MARKER: &[u8] = b"ICC_PROFILE\x00";

/// Chroma subsampling modes for JPEG encoding
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChromaSubsampling {
    /// Full chroma resolution (no subsampling) - best quality
    Yuv444,
    /// Horizontal subsampling only
    Yuv422,
    /// Both horizontal and vertical subsampling (default) - best compression
    #[default]
    Yuv420,
}

/// JPEG encoding configuration
#[derive(Debug, Clone)]
pub struct JpegConfig {
    /// Enable lossless mode (100% quality, disables DCT quantization)
    pub lossless: bool,
    /// Quality level 1-100 (ignored if lossless)
    pub quality: u8,
    /// Enable progressive scan encoding for better compression
    pub progressive: bool,
    /// Chroma subsampling (forced to 4:4:4 if lossless)
    pub chroma_subsampling: ChromaSubsampling,
}

impl Default for JpegConfig {
    fn default() -> Self {
        Self {
            lossless: false,
            quality: 75,
            progressive: true,
            chroma_subsampling: ChromaSubsampling::Yuv420,
        }
    }
}

impl JpegConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = quality.clamp(1, 100);
        self
    }

    pub fn with_lossless(mut self, lossless: bool) -> Self {
        self.lossless = lossless;
        self
    }

    pub fn with_progressive(mut self, progressive: bool) -> Self {
        self.progressive = progressive;
        self
    }

    pub fn with_chroma_subsampling(mut self, subsampling: ChromaSubsampling) -> Self {
        self.chroma_subsampling = subsampling;
        self
    }
}

/// JPEG encoder using mozjpeg
pub struct JpegEncoder;

impl Encoder for JpegEncoder {
    type Config = JpegConfig;

    fn encode(
        rgba: &[u8],
        width: u32,
        height: u32,
        config: &Self::Config,
        icc_profile: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        // Convert RGBA to RGB for JPEG (drop alpha channel)
        let rgb: Vec<u8> = rgba
            .chunks_exact(4)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();

        // Determine effective settings
        let quality = if config.lossless { 100.0 } else { config.quality as f32 };

        // Force 4:4:4 for lossless mode per spec
        let subsampling = if config.lossless {
            ChromaSubsampling::Yuv444
        } else {
            config.chroma_subsampling
        };

        // Create mozjpeg encoder
        let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);

        comp.set_size(width as usize, height as usize);
        comp.set_quality(quality);

        // Set chroma subsampling
        match subsampling {
            ChromaSubsampling::Yuv444 => {
                comp.set_chroma_sampling_pixel_sizes((1, 1), (1, 1));
            }
            ChromaSubsampling::Yuv422 => {
                comp.set_chroma_sampling_pixel_sizes((2, 1), (2, 1));
            }
            ChromaSubsampling::Yuv420 => {
                comp.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));
            }
        };

        // Enable progressive encoding if requested
        if config.progressive {
            comp.set_progressive_mode();
        }

        // Start compression to memory
        let mut comp = comp.start_compress(Vec::new())?;

        // Don't use mozjpeg's write_icc_profile - it has a bug with 0-indexed chunks
        // We'll inject the ICC profile ourselves after encoding

        // Write scanlines
        comp.write_scanlines(&rgb)?;

        // Finish and get output
        let mut output = comp.finish()?;

        // Inject ICC profile with correct 1-indexed chunk numbering
        if let Some(icc) = icc_profile {
            output = inject_icc_profile(&output, icc)?;
        }

        Ok(output)
    }

    fn extension() -> &'static str {
        "jpg"
    }

    fn mime_type() -> &'static str {
        "image/jpeg"
    }
}

/// Inject ICC profile into JPEG with correct 1-indexed chunk numbering
///
/// The ICC profile spec for JPEG requires chunks to be numbered 1 to N,
/// not 0 to N-1. mozjpeg-rust has a bug where it uses 0-indexed chunks.
fn inject_icc_profile(jpeg: &[u8], icc_profile: &[u8]) -> Result<Vec<u8>> {
    // Find insertion point after SOI and JFIF/APP0
    let insert_pos = find_icc_insert_position(jpeg)?;

    // Build ICC profile APP2 segments with 1-indexed chunk numbers
    let chunks: Vec<&[u8]> = icc_profile.chunks(ICC_CHUNK_SIZE).collect();
    let num_chunks = chunks.len() as u8;

    let mut icc_segments = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let chunk_num = (i + 1) as u8; // 1-indexed!

        // APP2 segment: FF E2 + length (2 bytes) + "ICC_PROFILE\0" + chunk_num + num_chunks + data
        // Length field includes itself (2 bytes) + marker + chunk info + data
        let segment_len = 2 + ICC_MARKER.len() + 2 + chunk.len();

        icc_segments.push(0xFF);
        icc_segments.push(0xE2);
        icc_segments.push(((segment_len >> 8) & 0xFF) as u8);
        icc_segments.push((segment_len & 0xFF) as u8);
        icc_segments.extend_from_slice(ICC_MARKER);
        icc_segments.push(chunk_num);
        icc_segments.push(num_chunks);
        icc_segments.extend_from_slice(chunk);
    }

    // Build output: [SOI + APP0] + [ICC segments] + [rest of JPEG]
    let mut output = Vec::with_capacity(jpeg.len() + icc_segments.len());
    output.extend_from_slice(&jpeg[..insert_pos]);
    output.extend_from_slice(&icc_segments);
    output.extend_from_slice(&jpeg[insert_pos..]);

    Ok(output)
}

/// Find the position to insert ICC profile (after SOI and APP0/JFIF)
fn find_icc_insert_position(jpeg: &[u8]) -> Result<usize> {
    anyhow::ensure!(
        jpeg.len() >= 2 && jpeg[0] == 0xFF && jpeg[1] == 0xD8,
        "Invalid JPEG: missing SOI marker"
    );

    let mut pos = 2; // After SOI

    // Skip APP0 (JFIF) if present
    while pos + 4 < jpeg.len() {
        if jpeg[pos] != 0xFF {
            break;
        }

        let marker = jpeg[pos + 1];

        // APP0 = 0xE0
        if marker == 0xE0 {
            let len = u16::from_be_bytes([jpeg[pos + 2], jpeg[pos + 3]]) as usize;
            pos += 2 + len;
        } else {
            // Stop at any other marker
            break;
        }
    }

    Ok(pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jpeg_config_defaults() {
        let config = JpegConfig::default();
        assert_eq!(config.quality, 75);
        assert!(config.progressive);
        assert!(!config.lossless);
        assert_eq!(config.chroma_subsampling, ChromaSubsampling::Yuv420);
    }

    #[test]
    fn test_jpeg_encode_small_image() {
        // Create a small red image (4x4)
        let rgba: Vec<u8> = (0..16)
            .flat_map(|_| [255, 0, 0, 255]) // Red pixels
            .collect();

        let config = JpegConfig::default();
        let output = JpegEncoder::encode(&rgba, 4, 4, &config, None).unwrap();

        // Should produce valid JPEG (starts with FFD8)
        assert!(output.len() > 2);
        assert_eq!(&output[0..2], &[0xFF, 0xD8]);
    }
}
