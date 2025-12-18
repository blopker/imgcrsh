use anyhow::Result;
use imgcrsh::{OutputFormat, PipelineConfig, process};
use std::env;
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <input> <output> [quality] [--preserve-icc]", args[0]);
        eprintln!();
        eprintln!("Output format is determined by file extension (.jpg, .png, .webp)");
        eprintln!("Quality: 1-100 for JPEG/WebP (default: 75/80), 0-6 for PNG optimization (default: 2)");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --preserve-icc  Keep original ICC profile (no color normalization)");
        std::process::exit(1);
    }

    // Parse arguments
    let input_path = &args[1];
    let output_path = &args[2];

    // Check for flags
    let preserve_icc = args.iter().any(|a| a == "--preserve-icc");

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
        _ => {
            eprintln!("Unknown output format. Use .jpg, .png, or .webp extension.");
            std::process::exit(1);
        }
    };

    println!("Processing: {}", input_path);
    println!("Input size: {} bytes", input_size);

    // Configure pipeline
    let config = match output_format {
        OutputFormat::Jpeg => PipelineConfig::new()
            .with_format(OutputFormat::Jpeg)
            .with_quality(quality)
            .with_lossless(false)
            .with_preserve_icc(preserve_icc),
        OutputFormat::Png => PipelineConfig::new()
            .with_format(OutputFormat::Png)
            .with_png_optimization(quality.min(6))
            .with_lossless(false)
            .with_preserve_icc(preserve_icc),
        OutputFormat::WebP => PipelineConfig::new()
            .with_format(OutputFormat::WebP)
            .with_quality(quality)
            .with_lossless(false)
            .with_preserve_icc(preserve_icc),
    };

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
