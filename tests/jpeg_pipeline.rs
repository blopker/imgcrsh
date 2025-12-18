//! Integration tests for the JPEG pipeline

use imgcrsh::{PipelineConfig, process};
use image::{RgbImage, ImageEncoder};
use std::io::Cursor;

/// Create a test JPEG image in memory
fn create_test_jpeg(width: u32, height: u32, quality: u8) -> Vec<u8> {
    // Create a gradient image
    let mut img = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let r = ((x as f32 / width as f32) * 255.0) as u8;
            let g = ((y as f32 / height as f32) * 255.0) as u8;
            let b = 128u8;
            img.put_pixel(x, y, image::Rgb([r, g, b]));
        }
    }

    // Encode to JPEG
    let mut buffer = Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, quality);
    encoder.write_image(
        img.as_raw(),
        width,
        height,
        image::ExtendedColorType::Rgb8,
    ).unwrap();

    buffer.into_inner()
}

#[test]
fn test_basic_jpeg_passthrough() {
    let input = create_test_jpeg(800, 600, 90);
    let input_size = input.len();

    let config = PipelineConfig::new()
        .with_quality(75);

    let output = process(&input, &config).expect("Pipeline should succeed");

    // Output should be valid JPEG
    assert!(!output.is_empty());
    // mozjpeg should compress better than the simple encoder
    println!("Input: {} bytes, Output: {} bytes", input_size, output.len());
}

#[test]
fn test_jpeg_resize() {
    let input = create_test_jpeg(1000, 800, 90);

    let config = PipelineConfig::new()
        .with_dimensions(Some(500), None) // Scale to 500px width
        .with_quality(80);

    let output = process(&input, &config).expect("Pipeline should succeed");

    // Verify output is valid by decoding it
    let decoded = image::load_from_memory(&output).expect("Output should be valid image");
    assert_eq!(decoded.width(), 500);
    assert_eq!(decoded.height(), 400); // Aspect ratio preserved
}

#[test]
fn test_jpeg_quality_levels() {
    let input = create_test_jpeg(400, 300, 95);

    // Low quality should be smaller
    let low_config = PipelineConfig::new().with_quality(30);
    let low_output = process(&input, &low_config).unwrap();

    // High quality should be larger
    let high_config = PipelineConfig::new().with_quality(95);
    let high_output = process(&input, &high_config).unwrap();

    println!("Q30: {} bytes, Q95: {} bytes", low_output.len(), high_output.len());
    assert!(low_output.len() < high_output.len());
}

#[test]
fn test_progressive_jpeg() {
    let input = create_test_jpeg(400, 300, 80);

    let config = PipelineConfig::new()
        .with_quality(75)
        .with_progressive(true);

    let output = process(&input, &config).expect("Pipeline should succeed");

    // Progressive JPEG should be valid
    let decoded = image::load_from_memory(&output).expect("Output should be valid image");
    assert_eq!(decoded.width(), 400);
    assert_eq!(decoded.height(), 300);
}

#[test]
fn test_lossless_mode() {
    let input = create_test_jpeg(200, 150, 100);

    let config = PipelineConfig::new()
        .with_lossless(true);

    let output = process(&input, &config).expect("Pipeline should succeed");

    // Lossless output should be valid
    let decoded = image::load_from_memory(&output).expect("Output should be valid image");
    assert_eq!(decoded.width(), 200);
    assert_eq!(decoded.height(), 150);
}

#[test]
fn test_normalize_to_p3() {
    let input = create_test_jpeg(300, 200, 90);

    // Normalize to Display P3 (preserve_icc: false is default)
    let config = PipelineConfig::new()
        .with_quality(80)
        .with_preserve_icc(false);

    let output = process(&input, &config).expect("Pipeline should succeed");

    // Output should be valid
    // Note: P3 profile only embedded if source has ICC profile
    assert!(!output.is_empty());
    let decoded = image::load_from_memory(&output).expect("Output should be valid image");
    assert_eq!(decoded.width(), 300);
    assert_eq!(decoded.height(), 200);
}

#[test]
fn test_preserve_icc() {
    let input = create_test_jpeg(300, 200, 90);

    // Preserve original ICC profile
    let config = PipelineConfig::new()
        .with_quality(80)
        .with_preserve_icc(true);

    let output = process(&input, &config).expect("Pipeline should succeed");

    // Output should be valid
    assert!(!output.is_empty());
    let decoded = image::load_from_memory(&output).expect("Output should be valid image");
    assert_eq!(decoded.width(), 300);
    assert_eq!(decoded.height(), 200);
}

#[test]
fn test_linear_resampling() {
    let input = create_test_jpeg(800, 600, 90);

    // Test resize with linear resampling (default enabled)
    let config = PipelineConfig::new()
        .with_dimensions(Some(400), Some(300))
        .with_quality(80);

    let output = process(&input, &config).expect("Pipeline should succeed");

    let decoded = image::load_from_memory(&output).expect("Output should be valid image");
    assert_eq!(decoded.width(), 400);
    assert_eq!(decoded.height(), 300);
}

#[test]
fn test_resize_without_linear_resampling() {
    let input = create_test_jpeg(800, 600, 90);

    // Disable linear resampling
    let mut config = PipelineConfig::new()
        .with_dimensions(Some(400), Some(300))
        .with_quality(80);
    config.linear_resampling = false;

    let output = process(&input, &config).expect("Pipeline should succeed");

    let decoded = image::load_from_memory(&output).expect("Output should be valid image");
    assert_eq!(decoded.width(), 400);
    assert_eq!(decoded.height(), 300);
}
