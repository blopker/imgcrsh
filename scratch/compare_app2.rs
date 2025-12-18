// Compare APP2 ICC segment structure between files
use std::fs;

fn main() {
    let correct = fs::read("./outputs/wide-gamut-correct.jpeg").unwrap();
    let ours = fs::read("./outputs/wide-gamut-preserve.jpeg").unwrap();
    let fixed = fs::read("./outputs/wide-gamut-fixed.jpeg").unwrap();

    println!("=== CORRECT FILE (libcaesium) ===");
    dump_app2_segments(&correct);

    println!("\n=== OLD FILE (buggy 0-indexed) ===");
    dump_app2_segments(&ours);

    println!("\n=== FIXED FILE (1-indexed) ===");
    dump_app2_segments(&fixed);
}

fn dump_app2_segments(data: &[u8]) {
    let mut pos = 0;
    while pos + 4 < data.len() {
        if data[pos] == 0xFF && data[pos + 1] == 0xE2 {
            let len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
            println!("APP2 at offset 0x{:04x}, length {}", pos, len);

            // Show first 32 bytes of segment content
            let content_start = pos + 4;
            let content_end = (content_start + 32).min(pos + 2 + len).min(data.len());
            let content = &data[content_start..content_end];

            // Check what type of APP2 this is
            if content.starts_with(b"ICC_PROFILE\x00") {
                let chunk_num = content[12];
                let total_chunks = content[13];
                println!("  Type: ICC_PROFILE, chunk {}/{}", chunk_num, total_chunks);
                println!("  Header bytes: {:02x?}", &content[..16.min(content.len())]);
            } else if content.starts_with(b"MPF\x00") {
                println!("  Type: MPF (Multi-Picture Format)");
                println!("  Header bytes: {:02x?}", &content[..16.min(content.len())]);
            } else {
                println!("  Type: Unknown");
                println!("  Header bytes: {:02x?}", &content[..16.min(content.len())]);
            }

            pos += 2 + len;
        } else if data[pos] == 0xFF && data[pos + 1] == 0xDA {
            // SOS - stop searching
            println!("Reached SOS marker, stopping search");
            break;
        } else {
            pos += 1;
        }
    }
}
