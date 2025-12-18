// Compare ICC profiles byte-by-byte
use std::fs;

fn main() {
    let correct = fs::read("./outputs/wide-gamut-correct.jpeg").unwrap();
    let ours = fs::read("./outputs/wide-gamut-preserve.jpeg").unwrap();

    // Find ICC profiles
    let icc1 = extract_icc(&correct);
    let icc2 = extract_icc(&ours);

    match (icc1, icc2) {
        (Some(p1), Some(p2)) => {
            println!("ICC Profile 1 size: {} bytes", p1.len());
            println!("ICC Profile 2 size: {} bytes", p2.len());

            if p1 == p2 {
                println!("ICC profiles are IDENTICAL");
            } else {
                println!("ICC profiles are DIFFERENT!");
                let min_len = p1.len().min(p2.len());
                let mut diff_count = 0;
                for i in 0..min_len {
                    if p1[i] != p2[i] {
                        if diff_count < 10 {
                            println!("  Diff at offset {}: {:02x} vs {:02x}", i, p1[i], p2[i]);
                        }
                        diff_count += 1;
                    }
                }
                println!("  Total differences: {}", diff_count);
            }
        }
        _ => println!("Could not extract ICC profiles"),
    }
}

fn extract_icc(data: &[u8]) -> Option<Vec<u8>> {
    // Look for ICC_PROFILE marker
    let marker = b"ICC_PROFILE\x00";
    for i in 0..data.len() - marker.len() {
        if &data[i..i + marker.len()] == marker {
            // Found it - the profile starts after the chunk header
            // Format: ICC_PROFILE\0 + chunk_num(1) + total_chunks(1) + profile_data
            let profile_start = i + marker.len() + 2;
            // Profile size is in the first 4 bytes of the profile (big-endian)
            if profile_start + 4 <= data.len() {
                let size = u32::from_be_bytes([
                    data[profile_start],
                    data[profile_start + 1],
                    data[profile_start + 2],
                    data[profile_start + 3],
                ]) as usize;
                if profile_start + size <= data.len() {
                    return Some(data[profile_start..profile_start + size].to_vec());
                }
            }
        }
    }
    None
}
