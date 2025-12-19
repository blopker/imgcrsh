//! Core image processing pipeline
//!
//! Implements the full pipeline: decode → orientation → color transform → resize → encode

use crate::color::{extract_color_info, get_display_p3_icc, get_srgb_icc, ColorTransformer};
use crate::config::{FilterType, OutputFormat, PipelineConfig};
use crate::formats::{AvifEncoder, Encoder, GifEncoderImpl, JpegEncoder, JxlEncoder, PngEncoder, WebpEncoder};
use crate::orientation::{apply_orientation, extract_orientation};
use anyhow::{Context, Result};
use fast_image_resize::{
    create_srgb_mapper, images::Image, FilterType as FirFilterType, ResizeAlg, ResizeOptions,
    Resizer,
};
use image::{DynamicImage, GenericImageView};

/// Process an image through the pipeline
///
/// # Arguments
/// * `input` - Raw encoded image bytes
/// * `config` - Pipeline configuration
///
/// # Returns
/// * Encoded output bytes in the configured format
pub fn process(input: &[u8], config: &PipelineConfig) -> Result<Vec<u8>> {
    // Detect input format
    let input_format = image::guess_format(input).context("Failed to detect image format")?;

    // === Phase A: Ingestion & Normalization ===

    // A.0: Extract EXIF orientation (before decoding changes anything)
    let orientation = extract_orientation(input);

    // A.1: Extract color space info from source
    let color_info = extract_color_info(input);

    // Decode image pixels
    let img = decode_image_with_format(input, input_format)?;
    let (src_width, src_height) = img.dimensions();

    // Convert to RGBA8 for processing
    let rgba = img.to_rgba8().into_raw();

    // A.0 (cont): Apply EXIF orientation transform
    // Always bake orientation since we're re-encoding (EXIF won't be preserved)
    let (mut rgba, src_width, src_height) = if orientation.needs_transform() {
        apply_orientation(&rgba, src_width, src_height, orientation)?
    } else {
        (rgba, src_width, src_height)
    };

    // A.2: Color normalization
    // - preserve_icc: true → no transform, keep original ICC
    // - preserve_icc: false → normalize to P3 if source has profile, else sRGB
    // - Quantized formats (lossy PNG) always stay in sRGB
    // - AVIF must stay in sRGB (ravif assumes sRGB, no CICP control)
    let uses_quantization =
        matches!(config.output_format, OutputFormat::Png) && !config.png.lossless;
    let requires_srgb = uses_quantization
        || matches!(config.output_format, OutputFormat::Avif | OutputFormat::Jxl | OutputFormat::Gif);
    let has_source_profile = color_info.icc_profile.is_some();

    // Determine color handling strategy
    let (apply_p3, preserve_original) = if config.preserve_icc {
        // Preserve original - no transform needed if already sRGB
        (false, true)
    } else if requires_srgb {
        // Format requires sRGB (quantization or AVIF)
        (false, false)
    } else if has_source_profile {
        // Has profile and not preserving - normalize to P3
        (true, false)
    } else {
        // No profile - keep as sRGB
        (false, false)
    };

    let color_transformer = if preserve_original {
        // No color transform - preserve original
        None
    } else if apply_p3 {
        let transformer = ColorTransformer::new(&color_info, true)?;
        if transformer.needs_transform() {
            transformer.transform_rgba8(&mut rgba, src_width as usize)?;
        }
        Some(transformer)
    } else {
        // Convert non-sRGB to sRGB
        let transformer = ColorTransformer::new(&color_info, false)?;
        if transformer.needs_transform() {
            transformer.transform_rgba8(&mut rgba, src_width as usize)?;
        }
        Some(transformer)
    };

    // === Phase B: Spatial Transformation ===

    // Calculate target dimensions
    let (dst_width, dst_height) =
        calculate_dimensions(src_width, src_height, config.width, config.height);

    // Resize if needed
    let resized = if dst_width != src_width || dst_height != src_height {
        resize_image(
            &rgba,
            src_width,
            src_height,
            dst_width,
            dst_height,
            config.filter_type,
            config.linear_resampling,
        )?
    } else {
        rgba
    };

    // === Phase D: Format-Specific Encoding ===

    // Get ICC profile for output (matches color space used above)
    let icc_profile = if preserve_original {
        // Pass through original ICC profile
        color_info.icc_profile.clone()
    } else if apply_p3 {
        // If source was already P3 and no transform happened, keep original profile
        // (different P3 profiles can have different tone curves)
        let did_transform = color_transformer
            .as_ref()
            .map(|t| t.needs_transform())
            .unwrap_or(false);
        if !did_transform && color_info.icc_profile.is_some() {
            color_info.icc_profile.clone()
        } else {
            Some(get_display_p3_icc())
        }
    } else {
        // Only embed sRGB if we did a color transform
        color_transformer.as_ref().and_then(|t| {
            if t.needs_transform() {
                Some(get_srgb_icc())
            } else {
                None
            }
        })
    };

    // Encode to target format
    let output = match config.output_format {
        OutputFormat::Jpeg => JpegEncoder::encode(
            &resized,
            dst_width,
            dst_height,
            &config.jpeg,
            icc_profile.as_deref(),
        )?,
        OutputFormat::Png => PngEncoder::encode(
            &resized,
            dst_width,
            dst_height,
            &config.png,
            icc_profile.as_deref(),
        )?,
        OutputFormat::WebP => WebpEncoder::encode(
            &resized,
            dst_width,
            dst_height,
            &config.webp,
            icc_profile.as_deref(),
        )?,
        OutputFormat::Avif => AvifEncoder::encode(
            &resized,
            dst_width,
            dst_height,
            &config.avif,
            icc_profile.as_deref(),
        )?,
        OutputFormat::Jxl => JxlEncoder::encode(
            &resized,
            dst_width,
            dst_height,
            &config.jxl,
            icc_profile.as_deref(),
        )?,
        OutputFormat::Gif => GifEncoderImpl::encode(
            &resized,
            dst_width,
            dst_height,
            &config.gif,
            None, // GIF doesn't support ICC profiles
        )?,
    };

    Ok(output)
}

/// Decode input bytes to a DynamicImage with known format
fn decode_image_with_format(input: &[u8], format: image::ImageFormat) -> Result<DynamicImage> {
    let img =
        image::load_from_memory_with_format(input, format).context("Failed to decode image")?;

    Ok(img)
}

/// Calculate output dimensions respecting aspect ratio
/// When both width and height are specified, fits within the bounding box
fn calculate_dimensions(
    src_width: u32,
    src_height: u32,
    target_width: Option<u32>,
    target_height: Option<u32>,
) -> (u32, u32) {
    match (target_width, target_height) {
        (Some(max_w), Some(max_h)) => {
            // Fit within bounding box, preserving aspect ratio
            let width_ratio = max_w as f64 / src_width as f64;
            let height_ratio = max_h as f64 / src_height as f64;
            let ratio = width_ratio.min(height_ratio);
            let w = (src_width as f64 * ratio).round() as u32;
            let h = (src_height as f64 * ratio).round() as u32;
            (w.max(1), h.max(1))
        }
        (Some(w), None) => {
            let ratio = w as f64 / src_width as f64;
            let h = (src_height as f64 * ratio).round() as u32;
            (w, h.max(1))
        }
        (None, Some(h)) => {
            let ratio = h as f64 / src_height as f64;
            let w = (src_width as f64 * ratio).round() as u32;
            (w.max(1), h)
        }
        (None, None) => (src_width, src_height),
    }
}

/// Resize image using fast_image_resize with optional linear light processing
fn resize_image(
    rgba: &[u8],
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    filter: FilterType,
    linear_resampling: bool,
) -> Result<Vec<u8>> {
    anyhow::ensure!(src_width > 0 && src_height > 0, "Invalid source dimensions");
    anyhow::ensure!(
        dst_width > 0 && dst_height > 0,
        "Invalid destination dimensions"
    );

    // Create source image
    let mut src_image = Image::from_vec_u8(
        src_width,
        src_height,
        rgba.to_vec(),
        fast_image_resize::PixelType::U8x4,
    )?;

    // Create destination image
    let mut dst_image = Image::new(dst_width, dst_height, fast_image_resize::PixelType::U8x4);

    // Select filter type
    let fir_filter = match filter {
        FilterType::Nearest => ResizeAlg::Nearest,
        FilterType::Bilinear => ResizeAlg::Convolution(FirFilterType::Bilinear),
        FilterType::Bicubic => ResizeAlg::Convolution(FirFilterType::CatmullRom),
        FilterType::Lanczos3 => ResizeAlg::Convolution(FirFilterType::Lanczos3),
    };

    let mut resizer = Resizer::new();
    let options = ResizeOptions::new().resize_alg(fir_filter);

    if linear_resampling {
        // Convert to linear color space before resizing
        let mapper = create_srgb_mapper();
        mapper.forward_map_inplace(&mut src_image)?;

        // Resize in linear space
        resizer.resize(&src_image, &mut dst_image, Some(&options))?;

        // Convert back to sRGB
        mapper.backward_map_inplace(&mut dst_image)?;
    } else {
        // Direct resize in gamma-encoded space
        resizer.resize(&src_image, &mut dst_image, Some(&options))?;
    }

    Ok(dst_image.into_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_dimensions_both_specified_fit_width() {
        // 1000x800 into 500x500 box -> limited by width -> 500x400
        assert_eq!(
            calculate_dimensions(1000, 800, Some(500), Some(500)),
            (500, 400)
        );
    }

    #[test]
    fn test_calculate_dimensions_both_specified_fit_height() {
        // 800x1000 into 500x500 box -> limited by height -> 400x500
        assert_eq!(
            calculate_dimensions(800, 1000, Some(500), Some(500)),
            (400, 500)
        );
    }

    #[test]
    fn test_calculate_dimensions_width_only() {
        assert_eq!(calculate_dimensions(1000, 800, Some(500), None), (500, 400));
    }

    #[test]
    fn test_calculate_dimensions_height_only() {
        assert_eq!(calculate_dimensions(1000, 800, None, Some(400)), (500, 400));
    }

    #[test]
    fn test_calculate_dimensions_none() {
        assert_eq!(calculate_dimensions(1000, 800, None, None), (1000, 800));
    }
}
