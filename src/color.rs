//! Color space detection and transformation
//!
//! Handles ICC profile extraction, color space inference, and transforms
//! between color spaces using moxcms.

use anyhow::{Context, Result};
use moxcms::{ColorProfile, Layout, ProfileText, TransformOptions};

/// Detected source color space
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceColorSpace {
    /// Standard sRGB (most common)
    Srgb,
    /// Adobe RGB (1998)
    AdobeRgb,
    /// Display P3
    DisplayP3,
    /// Unknown/custom profile (will be handled via ICC)
    Custom,
}

/// Color space metadata extracted from image
#[derive(Debug, Clone)]
pub struct ColorSpaceInfo {
    /// Detected color space
    pub space: SourceColorSpace,
    /// Raw ICC profile bytes (if present)
    pub icc_profile: Option<Vec<u8>>,
}

impl Default for ColorSpaceInfo {
    fn default() -> Self {
        Self {
            space: SourceColorSpace::Srgb,
            icc_profile: None,
        }
    }
}

/// Extract text from ProfileText enum
fn profile_text_to_string(text: &ProfileText) -> Option<String> {
    match text {
        ProfileText::PlainString(s) => Some(s.clone()),
        ProfileText::Localizable(locales) => {
            locales.first().map(|l| l.value.clone())
        }
        ProfileText::Description(desc) => Some(desc.ascii_string.clone()),
    }
}

/// Extract ICC profile from JPEG data
pub fn extract_icc_from_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    // JPEG ICC profiles are stored in APP2 markers with "ICC_PROFILE\0" header
    // They can be chunked across multiple APP2 segments
    let mut chunks: Vec<(u8, u8, Vec<u8>)> = Vec::new();

    let mut pos = 0;
    while pos < data.len() - 1 {
        // Find marker
        if data[pos] != 0xFF {
            pos += 1;
            continue;
        }

        let marker = data[pos + 1];
        pos += 2;

        // Skip if not a marker with length
        if marker == 0x00 || marker == 0x01 || (0xD0..=0xD9).contains(&marker) {
            continue;
        }

        // End of image
        if marker == 0xD9 {
            break;
        }

        // Start of scan - stop parsing headers
        if marker == 0xDA {
            break;
        }

        // Read length
        if pos + 2 > data.len() {
            break;
        }
        let length = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;

        if length < 2 || pos + length - 2 > data.len() {
            break;
        }

        let segment_data = &data[pos..pos + length - 2];
        pos += length - 2;

        // APP2 marker (0xE2) with ICC_PROFILE header
        if marker == 0xE2 && segment_data.len() > 14 {
            if &segment_data[0..12] == b"ICC_PROFILE\0" {
                let chunk_num = segment_data[12];
                let total_chunks = segment_data[13];
                let profile_data = segment_data[14..].to_vec();
                chunks.push((chunk_num, total_chunks, profile_data));
            }
        }
    }

    if chunks.is_empty() {
        return None;
    }

    // Sort chunks by number and concatenate
    chunks.sort_by_key(|(num, _, _)| *num);

    let profile: Vec<u8> = chunks.into_iter().flat_map(|(_, _, data)| data).collect();

    if profile.len() >= 128 {
        Some(profile)
    } else {
        None
    }
}

/// Detect color space from ICC profile
pub fn detect_color_space(icc_data: &[u8]) -> SourceColorSpace {
    // Try to parse the ICC profile description
    if let Ok(profile) = ColorProfile::new_from_slice(icc_data) {
        // Check profile description for known color spaces
        if let Some(ref desc_text) = profile.description {
            if let Some(desc) = profile_text_to_string(desc_text) {
                let desc_lower = desc.to_lowercase();
                if desc_lower.contains("display p3") || desc_lower.contains("p3") {
                    return SourceColorSpace::DisplayP3;
                }
                if desc_lower.contains("adobe rgb") || desc_lower.contains("adobergb") {
                    return SourceColorSpace::AdobeRgb;
                }
                if desc_lower.contains("srgb") || desc_lower.contains("iec61966") {
                    return SourceColorSpace::Srgb;
                }
            }
        }
        // Has a profile but couldn't identify it
        return SourceColorSpace::Custom;
    }
    // Failed to parse, assume sRGB
    SourceColorSpace::Srgb
}

/// Check EXIF ColorSpace tag (value 2 = Adobe RGB per spec)
pub fn detect_color_space_from_exif(data: &[u8]) -> Option<SourceColorSpace> {
    use std::io::Cursor;

    let cursor = Cursor::new(data);
    let exif = exif::Reader::new()
        .read_from_container(&mut std::io::BufReader::new(cursor))
        .ok()?;

    // ColorSpace tag: 1 = sRGB, 2 = Adobe RGB, 0xFFFF = Uncalibrated
    if let Some(field) = exif.get_field(exif::Tag::ColorSpace, exif::In::PRIMARY) {
        if let exif::Value::Short(values) = &field.value {
            if let Some(&value) = values.first() {
                return match value {
                    1 => Some(SourceColorSpace::Srgb),
                    2 => Some(SourceColorSpace::AdobeRgb),
                    _ => None,
                };
            }
        }
    }
    None
}

/// Extract complete color space info from JPEG
pub fn extract_color_info(data: &[u8]) -> ColorSpaceInfo {
    // First, try to get ICC profile
    if let Some(icc) = extract_icc_from_jpeg(data) {
        let space = detect_color_space(&icc);
        return ColorSpaceInfo {
            space,
            icc_profile: Some(icc),
        };
    }

    // Fall back to EXIF ColorSpace tag
    if let Some(space) = detect_color_space_from_exif(data) {
        return ColorSpaceInfo {
            space,
            icc_profile: None,
        };
    }

    // Default to sRGB
    ColorSpaceInfo::default()
}

/// Color transformer for converting between color spaces
pub struct ColorTransformer {
    /// Source profile
    source: ColorProfile,
    /// Destination profile
    dest: ColorProfile,
    /// Whether color normalization is enabled
    normalize: bool,
    /// Source color space (cached)
    source_space: SourceColorSpace,
}

impl ColorTransformer {
    /// Create a new transformer from source color info to target space
    pub fn new(color_info: &ColorSpaceInfo, normalize_to_p3: bool) -> Result<Self> {
        // Build source profile
        let source = if let Some(icc) = &color_info.icc_profile {
            ColorProfile::new_from_slice(icc)
                .context("Failed to parse source ICC profile")?
        } else {
            match color_info.space {
                SourceColorSpace::Srgb => ColorProfile::new_srgb(),
                SourceColorSpace::AdobeRgb => ColorProfile::new_adobe_rgb(),
                SourceColorSpace::DisplayP3 => ColorProfile::new_display_p3(),
                SourceColorSpace::Custom => ColorProfile::new_srgb(), // Fallback
            }
        };

        // Build destination profile
        let dest = if normalize_to_p3 {
            ColorProfile::new_display_p3()
        } else {
            ColorProfile::new_srgb()
        };

        Ok(Self {
            source,
            dest,
            normalize: normalize_to_p3,
            source_space: color_info.space,
        })
    }

    /// Check if transform is needed (source != dest)
    pub fn needs_transform(&self) -> bool {
        // If normalizing to P3, always transform unless source is already P3
        if self.normalize {
            return self.source_space != SourceColorSpace::DisplayP3;
        }
        // Otherwise, only transform if source is not sRGB
        self.source_space != SourceColorSpace::Srgb
    }

    /// Transform RGB8 pixels in place
    pub fn transform_rgb8(&self, pixels: &mut [u8], width: usize) -> Result<()> {
        let transform = self.source
            .create_transform_8bit(Layout::Rgb, &self.dest, Layout::Rgb, TransformOptions::default())
            .context("Failed to create color transform")?;

        // Process scanlines
        let row_bytes = width * 3;
        for row in pixels.chunks_exact_mut(row_bytes) {
            let mut dst = vec![0u8; row_bytes];
            transform.transform(row, &mut dst)
                .context("Color transform failed")?;
            row.copy_from_slice(&dst);
        }

        Ok(())
    }

    /// Transform RGBA8 pixels (ignores alpha)
    pub fn transform_rgba8(&self, pixels: &mut [u8], width: usize) -> Result<()> {
        let transform = self.source
            .create_transform_8bit(Layout::Rgba, &self.dest, Layout::Rgba, TransformOptions::default())
            .context("Failed to create color transform")?;

        // Process scanlines
        let row_bytes = width * 4;
        for row in pixels.chunks_exact_mut(row_bytes) {
            let mut dst = vec![0u8; row_bytes];
            transform.transform(row, &mut dst)
                .context("Color transform failed")?;
            row.copy_from_slice(&dst);
        }

        Ok(())
    }

    /// Get the ICC profile bytes for the destination color space
    pub fn dest_icc_profile(&self) -> Result<Vec<u8>> {
        self.dest.encode()
            .context("Failed to encode destination ICC profile")
    }

    /// Returns true if output is Display P3
    pub fn is_p3_output(&self) -> bool {
        self.normalize
    }
}

/// Display P3 ICC profile bytes
pub fn get_display_p3_icc() -> Vec<u8> {
    ColorProfile::new_display_p3()
        .encode()
        .unwrap_or_default()
}

/// Standard sRGB ICC profile
pub fn get_srgb_icc() -> Vec<u8> {
    ColorProfile::new_srgb()
        .encode()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_color_info() {
        let info = ColorSpaceInfo::default();
        assert_eq!(info.space, SourceColorSpace::Srgb);
        assert!(info.icc_profile.is_none());
    }

    #[test]
    fn test_p3_icc_generation() {
        let icc = get_display_p3_icc();
        assert!(!icc.is_empty());
        // Verify it's a valid ICC profile (starts with profile size)
        assert!(icc.len() >= 128);
    }

    #[test]
    fn test_srgb_icc_generation() {
        let icc = get_srgb_icc();
        assert!(!icc.is_empty());
        assert!(icc.len() >= 128);
    }
}
