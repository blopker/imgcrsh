use anyhow::Result;
use imgcrsh::{OutputFormat, PipelineConfig, process};
use std::env;
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <input> <output> [quality]", args[0]);
        eprintln!();
        eprintln!("Output format is determined by file extension (.jpg, .png)");
        eprintln!("Quality: 1-100 for JPEG (default: 75), 0-6 for PNG optimization (default: 2)");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];
    let quality: u8 = args.get(3).and_then(|q| q.parse().ok()).unwrap_or(75);

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
        _ => {
            eprintln!("Unknown output format. Use .jpg or .png extension.");
            std::process::exit(1);
        }
    };

    println!("Processing: {}", input_path);
    println!("Input size: {} bytes", input_size);

    // Configure pipeline
    let config = match output_format {
        OutputFormat::Jpeg => PipelineConfig::new()
            .with_format(OutputFormat::Jpeg)
            .with_quality(quality),
        OutputFormat::Png => PipelineConfig::new()
            .with_format(OutputFormat::Png)
            .with_png_optimization(quality.min(6)),
    };

    // Process image
    let output = process(&input, &config)?;
    let output_size = output.len();

    // Write output
    fs::write(output_path, &output)?;

    let ratio = (output_size as f64 / input_size as f64) * 100.0;
    println!("Output size: {} bytes ({:.1}% of original)", output_size, ratio);
    println!("Saved to: {}", output_path);

    Ok(())
}
