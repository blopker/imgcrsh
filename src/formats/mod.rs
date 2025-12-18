//! Format-specific encoding modules
//!
//! Each format implements the `Encoder` trait for consistent pipeline integration.

pub mod jpeg;
pub mod png;

pub use jpeg::{ChromaSubsampling, JpegConfig, JpegEncoder};
pub use png::{PngConfig, PngEncoder};

use anyhow::Result;

/// Common interface for all image encoders
pub trait Encoder {
    /// Associated configuration type for this encoder
    type Config;

    /// Encode RGBA pixels to the target format
    ///
    /// # Arguments
    /// * `rgba` - RGBA8 pixel data
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `config` - Format-specific configuration
    /// * `icc_profile` - Optional ICC profile to embed
    ///
    /// # Returns
    /// Encoded image bytes
    fn encode(
        rgba: &[u8],
        width: u32,
        height: u32,
        config: &Self::Config,
        icc_profile: Option<&[u8]>,
    ) -> Result<Vec<u8>>;

    /// Returns the file extension for this format
    fn extension() -> &'static str;

    /// Returns the MIME type for this format
    fn mime_type() -> &'static str;
}
