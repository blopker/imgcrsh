//! Core image processing pipeline
//!
//! Implements the full pipeline: decode → color transform → resize → encode

use crate::color::{extract_color_info, ColorTransformer, get_display_p3_icc, get_srgb_icc};
use crate::config::{ChromaSubsampling, FilterType, PipelineConfig};
use anyhow::{Context, Result};
use fast_image_resize::{
    images::Image, create_srgb_mapper, FilterType as FirFilterType,
    ResizeAlg, ResizeOptions, Resizer,
};
use image::{DynamicImage, GenericImageView};

/// Process an image through the pipeline
///
/// # Arguments
/// * `input` - Raw encoded image bytes (JPEG)
/// * `config` - Pipeline configuration
///
/// # Returns
/// * Encoded output bytes (JPEG)
pub fn process(input: &[u8], config: &PipelineConfig) -> Result<Vec<u8>> {
    // === Phase A: Ingestion & Color Detection ===

    // A.1: Extract color space info from source
    let color_info = extract_color_info(input);

    // Decode image pixels
    let img = decode_image(input)?;
    let (src_width, src_height) = img.dimensions();

    // Convert to RGBA8 for processing
    let mut rgba = img.to_rgba8().into_raw();

    // A.2: Color normalization (if enabled)
    let color_transformer = if config.color_normalization {
        let transformer = ColorTransformer::new(&color_info, true)?;
        if transformer.needs_transform() {
            transformer.transform_rgba8(&mut rgba, src_width as usize)?;
        }
        Some(transformer)
    } else {
        // Even without normalization, we may need to convert non-sRGB to sRGB
        let transformer = ColorTransformer::new(&color_info, false)?;
        if transformer.needs_transform() {
            transformer.transform_rgba8(&mut rgba, src_width as usize)?;
        }
        Some(transformer)
    };

    // === Phase B: Spatial Transformation ===

    // Calculate target dimensions
    let (dst_width, dst_height) = calculate_dimensions(
        src_width,
        src_height,
        config.width,
        config.height,
    );

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

    // Get ICC profile for output
    let icc_profile = if config.color_normalization {
        Some(get_display_p3_icc())
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

    // Encode to JPEG
    let output = encode_jpeg(&resized, dst_width, dst_height, config, icc_profile.as_deref())?;

    Ok(output)
}

/// Decode input bytes to a DynamicImage
fn decode_image(input: &[u8]) -> Result<DynamicImage> {
    let format = image::guess_format(input)
        .context("Failed to detect image format")?;

    let img = image::load_from_memory_with_format(input, format)
        .context("Failed to decode image")?;

    Ok(img)
}

/// Calculate output dimensions respecting aspect ratio
fn calculate_dimensions(
    src_width: u32,
    src_height: u32,
    target_width: Option<u32>,
    target_height: Option<u32>,
) -> (u32, u32) {
    match (target_width, target_height) {
        (Some(w), Some(h)) => (w, h),
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
    anyhow::ensure!(dst_width > 0 && dst_height > 0, "Invalid destination dimensions");

    // Create source image
    let mut src_image = Image::from_vec_u8(
        src_width,
        src_height,
        rgba.to_vec(),
        fast_image_resize::PixelType::U8x4,
    )?;

    // Create destination image
    let mut dst_image = Image::new(
        dst_width,
        dst_height,
        fast_image_resize::PixelType::U8x4,
    );

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

/// Encode pixels to JPEG using mozjpeg with optional ICC profile
fn encode_jpeg(
    rgba: &[u8],
    width: u32,
    height: u32,
    config: &PipelineConfig,
    icc_profile: Option<&[u8]>,
) -> Result<Vec<u8>> {
    // Convert RGBA to RGB for JPEG (drop alpha channel)
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();

    // Determine effective settings
    let quality = if config.jpeg.lossless { 100.0 } else { config.jpeg.quality as f32 };

    // Force 4:4:4 for lossless mode per spec
    let subsampling = if config.jpeg.lossless {
        ChromaSubsampling::Yuv444
    } else {
        config.jpeg.chroma_subsampling
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
    if config.jpeg.progressive {
        comp.set_progressive_mode();
    }

    // Start compression to memory
    let mut comp = comp.start_compress(Vec::new())?;

    // Inject ICC profile if provided (must be done after start_compress)
    if let Some(icc) = icc_profile {
        comp.write_icc_profile(icc);
    }

    // Write scanlines
    comp.write_scanlines(&rgb)?;

    // Finish and get output
    let output = comp.finish()?;

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_dimensions_both_specified() {
        assert_eq!(calculate_dimensions(1000, 800, Some(500), Some(400)), (500, 400));
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
