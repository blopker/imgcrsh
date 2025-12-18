use anyhow::Result;
use imgcrsh::{PipelineConfig, process};
use std::env;
use std::fs;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <input.jpg> <output.jpg> [quality]", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];
    let quality: u8 = args.get(3).and_then(|q| q.parse().ok()).unwrap_or(75);

    // Read input file
    let input = fs::read(input_path)?;
    let input_size = input.len();

    println!("Processing: {}", input_path);
    println!("Input size: {} bytes", input_size);

    // Configure pipeline
    let config = PipelineConfig::new()
        .with_quality(quality);

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
