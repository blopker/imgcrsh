// Quick test to compare decoded pixel values between two JPEGs
use std::fs;

fn main() {
    let correct = fs::read("./outputs/wide-gamut-correct.jpeg").unwrap();
    let ours = fs::read("./outputs/wide-gamut-preserve.jpeg").unwrap();

    let img1 = image::load_from_memory(&correct).unwrap();
    let img2 = image::load_from_memory(&ours).unwrap();

    let rgb1 = img1.to_rgb8();
    let rgb2 = img2.to_rgb8();

    println!("Correct: {}x{}", img1.width(), img1.height());
    println!("Ours: {}x{}", img2.width(), img2.height());

    let pixels1 = rgb1.as_raw();
    let pixels2 = rgb2.as_raw();

    // Sample positions (pixel index * 3 for RGB)
    for &idx in &[0, 1000, 10000, 100000, 500000, 1000000] {
        let pos = idx * 3;
        if pos + 2 < pixels1.len() && pos + 2 < pixels2.len() {
            let p1 = (pixels1[pos], pixels1[pos+1], pixels1[pos+2]);
            let p2 = (pixels2[pos], pixels2[pos+1], pixels2[pos+2]);
            let diff = (
                (p1.0 as i32 - p2.0 as i32).abs(),
                (p1.1 as i32 - p2.1 as i32).abs(),
                (p1.2 as i32 - p2.2 as i32).abs()
            );
            println!("Pixel {}: correct={:?}, ours={:?}, diff={:?}", idx, p1, p2, diff);
        }
    }

    // Calculate average and max difference
    let mut total_diff: u64 = 0;
    let mut max_diff: i32 = 0;
    let mut count = 0;
    for i in 0..pixels1.len().min(pixels2.len()) {
        let d = (pixels1[i] as i32 - pixels2[i] as i32).abs();
        total_diff += d as u64;
        max_diff = max_diff.max(d);
        count += 1;
    }
    let avg = total_diff as f64 / count as f64;
    println!("\nAverage per-channel difference: {:.2}", avg);
    println!("Max per-channel difference: {}", max_diff);
}
