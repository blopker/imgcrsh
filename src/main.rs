use anyhow::Result;
use imgcrsh::{process, OutputFormat, PipelineConfig};
use std::env;
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <input> <output> [quality] [options]", args[0]);
        eprintln!();
        eprintln!("Output format is determined by file extension (.jpg, .png, .webp, .avif, .jxl, .gif)");
        eprintln!("Quality: 1-100 for JPEG/WebP/AVIF/JXL/GIF (default: 75-80), 0-6 for PNG optimization (default: 2)");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --width=N       Resize to width N (preserves aspect ratio)");
        eprintln!("  --height=N      Resize to height N (preserves aspect ratio)");
        eprintln!("                  (if both specified, fits within bounding box)");
        eprintln!("  --lossless      Lossless encoding (PNG, WebP, JXL)");
        eprintln!("  --preserve-icc  Keep original ICC profile (no color normalization)");
        std::process::exit(1);
    }

    // Parse arguments
    let input_path = &args[1];
    let output_path = &args[2];

    // Check for flags
    let preserve_icc = args.iter().any(|a| a == "--preserve-icc");
    let lossless = args.iter().any(|a| a == "--lossless");

    // Parse --width=N and --height=N
    let width: Option<u32> = args
        .iter()
        .find(|a| a.starts_with("--width="))
        .and_then(|a| a.strip_prefix("--width="))
        .and_then(|v| v.parse().ok());

    let height: Option<u32> = args
        .iter()
        .find(|a| a.starts_with("--height="))
        .and_then(|a| a.strip_prefix("--height="))
        .and_then(|v| v.parse().ok());

    // Parse quality (skip flags)
    let quality: u8 = args
        .iter()
        .skip(3)
        .find(|a| !a.starts_with("--"))
        .and_then(|q| q.parse().ok())
        .unwrap_or(75);

    // Read input file
    let input = fs::read(input_path)?;
    let input_size = input.len();

    // Detect output format from extension
    let output_format = match Path::new(output_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("png") => OutputFormat::Png,
        Some("jpg") | Some("jpeg") => OutputFormat::Jpeg,
        Some("webp") => OutputFormat::WebP,
        Some("avif") => OutputFormat::Avif,
        Some("jxl") => OutputFormat::Jxl,
        Some("gif") => OutputFormat::Gif,
        _ => {
            eprintln!("Unknown output format. Use .jpg, .png, .webp, .avif, .jxl, or .gif extension.");
            std::process::exit(1);
        }
    };

    println!("Processing: {}", input_path);
    println!("Input size: {} bytes", input_size);

    // Configure pipeline
    let mut config = PipelineConfig::new()
        .with_format(output_format)
        .with_quality(quality)
        .with_lossless(lossless)
        .with_preserve_icc(preserve_icc)
        .with_dimensions(width, height);

    // PNG uses optimization level instead of quality
    if matches!(output_format, OutputFormat::Png) {
        config = config.with_png_optimization(quality.min(6));
    }

    if width.is_some() || height.is_some() {
        match (width, height) {
            (Some(w), Some(h)) => println!("Fitting within {}x{}", w, h),
            (Some(w), None) => println!("Resizing to width {}", w),
            (None, Some(h)) => println!("Resizing to height {}", h),
            _ => {}
        }
    }
    if lossless {
        println!("Using lossless encoding");
    }
    if preserve_icc {
        println!("Preserving original ICC profile");
    }

    // Process image
    let output = process(&input, &config)?;
    let output_size = output.len();

    // Write output
    fs::write(output_path, &output)?;

    let ratio = (output_size as f64 / input_size as f64) * 100.0;
    println!(
        "Output size: {} bytes ({:.1}% of original)",
        output_size, ratio
    );
    println!("Saved to: {}", output_path);

    Ok(())
}
