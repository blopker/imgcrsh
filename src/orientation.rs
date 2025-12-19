//! EXIF Orientation handling
//!
//! Parses EXIF Orientation tag and applies pixel transforms to bake
//! the orientation into the image data.

use anyhow::Result;
use std::io::Cursor;

/// EXIF Orientation values (1-8)
/// See: https://exiftool.org/TagNames/EXIF.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    /// Normal (no transform needed)
    #[default]
    Normal = 1,
    /// Flip horizontal
    FlipHorizontal = 2,
    /// Rotate 180°
    Rotate180 = 3,
    /// Flip vertical
    FlipVertical = 4,
    /// Rotate 90° CW then flip horizontal
    Rotate90CwFlipH = 5,
    /// Rotate 90° CW (270° CCW)
    Rotate90Cw = 6,
    /// Rotate 90° CCW then flip horizontal
    Rotate90CcwFlipH = 7,
    /// Rotate 90° CCW (270° CW)
    Rotate90Ccw = 8,
}

impl Orientation {
    /// Parse from EXIF orientation value
    pub fn from_exif_value(value: u16) -> Self {
        match value {
            1 => Orientation::Normal,
            2 => Orientation::FlipHorizontal,
            3 => Orientation::Rotate180,
            4 => Orientation::FlipVertical,
            5 => Orientation::Rotate90CwFlipH,
            6 => Orientation::Rotate90Cw,
            7 => Orientation::Rotate90CcwFlipH,
            8 => Orientation::Rotate90Ccw,
            _ => Orientation::Normal,
        }
    }

    /// Check if this orientation requires any transform
    pub fn needs_transform(&self) -> bool {
        *self != Orientation::Normal
    }

    /// Check if this orientation swaps width and height
    pub fn swaps_dimensions(&self) -> bool {
        matches!(
            self,
            Orientation::Rotate90Cw
                | Orientation::Rotate90Ccw
                | Orientation::Rotate90CwFlipH
                | Orientation::Rotate90CcwFlipH
        )
    }
}

/// Extract EXIF Orientation tag from image data
pub fn extract_orientation(data: &[u8]) -> Orientation {
    let cursor = Cursor::new(data);
    let exif = match exif::Reader::new().read_from_container(&mut std::io::BufReader::new(cursor)) {
        Ok(exif) => exif,
        Err(_) => return Orientation::Normal,
    };

    if let Some(field) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        && let exif::Value::Short(values) = &field.value
        && let Some(&value) = values.first()
    {
        return Orientation::from_exif_value(value);
    }

    Orientation::Normal
}

/// Apply orientation transform to RGBA pixel data
///
/// Returns the transformed pixels and new dimensions (width, height)
pub fn apply_orientation(
    rgba: &[u8],
    width: u32,
    height: u32,
    orientation: Orientation,
) -> Result<(Vec<u8>, u32, u32)> {
    if !orientation.needs_transform() {
        return Ok((rgba.to_vec(), width, height));
    }

    let w = width as usize;
    let h = height as usize;

    // Determine output dimensions
    let (out_w, out_h) = if orientation.swaps_dimensions() {
        (height, width)
    } else {
        (width, height)
    };

    let out_w_usize = out_w as usize;
    let out_h_usize = out_h as usize;
    let mut output = vec![0u8; out_w_usize * out_h_usize * 4];

    // Apply transform based on orientation
    for y in 0..h {
        for x in 0..w {
            let src_idx = (y * w + x) * 4;
            let pixel = &rgba[src_idx..src_idx + 4];

            // Calculate destination position based on orientation
            let (dx, dy) = match orientation {
                Orientation::Normal => (x, y),
                Orientation::FlipHorizontal => (w - 1 - x, y),
                Orientation::Rotate180 => (w - 1 - x, h - 1 - y),
                Orientation::FlipVertical => (x, h - 1 - y),
                Orientation::Rotate90CwFlipH => (y, x),
                Orientation::Rotate90Cw => (h - 1 - y, x),
                Orientation::Rotate90CcwFlipH => (h - 1 - y, w - 1 - x),
                Orientation::Rotate90Ccw => (y, w - 1 - x),
            };

            let dst_idx = (dy * out_w_usize + dx) * 4;
            output[dst_idx..dst_idx + 4].copy_from_slice(pixel);
        }
    }

    Ok((output, out_w, out_h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orientation_from_value() {
        assert_eq!(Orientation::from_exif_value(1), Orientation::Normal);
        assert_eq!(Orientation::from_exif_value(6), Orientation::Rotate90Cw);
        assert_eq!(Orientation::from_exif_value(8), Orientation::Rotate90Ccw);
        assert_eq!(Orientation::from_exif_value(99), Orientation::Normal);
    }

    #[test]
    fn test_needs_transform() {
        assert!(!Orientation::Normal.needs_transform());
        assert!(Orientation::Rotate90Cw.needs_transform());
        assert!(Orientation::FlipHorizontal.needs_transform());
    }

    #[test]
    fn test_swaps_dimensions() {
        assert!(!Orientation::Normal.swaps_dimensions());
        assert!(!Orientation::Rotate180.swaps_dimensions());
        assert!(Orientation::Rotate90Cw.swaps_dimensions());
        assert!(Orientation::Rotate90Ccw.swaps_dimensions());
    }

    #[test]
    fn test_flip_horizontal() {
        // 2x1 image: [R, G] -> [G, R]
        let rgba = vec![
            255, 0, 0, 255, // Red
            0, 255, 0, 255, // Green
        ];
        let (result, w, h) = apply_orientation(&rgba, 2, 1, Orientation::FlipHorizontal).unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 1);
        assert_eq!(&result[0..4], &[0, 255, 0, 255]); // Green first
        assert_eq!(&result[4..8], &[255, 0, 0, 255]); // Red second
    }

    #[test]
    fn test_rotate_90_cw() {
        // 2x1 image rotated 90° CW becomes 1x2
        // [R, G] -> [R]
        //           [G]
        let rgba = vec![
            255, 0, 0, 255, // Red
            0, 255, 0, 255, // Green
        ];
        let (result, w, h) = apply_orientation(&rgba, 2, 1, Orientation::Rotate90Cw).unwrap();
        assert_eq!(w, 1);
        assert_eq!(h, 2);
        // After 90° CW rotation, Red should be at top, Green at bottom
        assert_eq!(&result[0..4], &[255, 0, 0, 255]); // Red at (0,0)
        assert_eq!(&result[4..8], &[0, 255, 0, 255]); // Green at (0,1)
    }
}
