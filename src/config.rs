//! Pipeline configuration types

use crate::formats::{avif::AvifConfig, gif::GifConfig, jpeg::JpegConfig, jxl::JxlConfig, png::PngConfig, webp::WebpConfig};

/// Resampling filter types for spatial transformation
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FilterType {
    Nearest,
    Bilinear,
    Bicubic,
    #[default]
    Lanczos3,
}

/// Output format specification
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Jpeg,
    Png,
    WebP,
    Avif,
    Jxl,
    Gif,
}

/// Main pipeline configuration
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    // === Normalization ===
    /// When true, strips EXIF/XMP metadata (orientation is always baked)
    pub strip_metadata: bool,
    /// When true, preserves original ICC profile without color conversion
    /// When false, normalizes to Display P3 (if source has profile) or keeps sRGB
    pub preserve_icc: bool,

    // === Resampling ===
    /// Target width (None preserves original or aspect ratio)
    pub width: Option<u32>,
    /// Target height (None preserves original or aspect ratio)
    pub height: Option<u32>,
    /// Resampling filter algorithm
    pub filter_type: FilterType,
    /// Perform resampling in linear light (f32) to prevent energy loss
    pub linear_resampling: bool,

    // === Output ===
    /// Output format
    pub output_format: OutputFormat,
    /// JPEG-specific options
    pub jpeg: JpegConfig,
    /// PNG-specific options
    pub png: PngConfig,
    /// WebP-specific options
    pub webp: WebpConfig,
    /// AVIF-specific options
    pub avif: AvifConfig,
    /// JPEG XL-specific options
    pub jxl: JxlConfig,
    /// GIF-specific options
    pub gif: GifConfig,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            strip_metadata: true,
            preserve_icc: false, // Normalize to P3 by default
            width: None,
            height: None,
            filter_type: FilterType::Lanczos3,
            linear_resampling: true,
            output_format: OutputFormat::Jpeg,
            jpeg: JpegConfig::default(),
            png: PngConfig::default(),
            webp: WebpConfig::default(),
            avif: AvifConfig::default(),
            jxl: JxlConfig::default(),
            gif: GifConfig::default(),
        }
    }
}

impl PipelineConfig {
    /// Create a new configuration with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set output format
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
        self
    }

    /// Set target dimensions (None for either preserves aspect ratio)
    pub fn with_dimensions(mut self, width: Option<u32>, height: Option<u32>) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set quality for all lossy formats (1-100)
    pub fn with_quality(mut self, quality: u8) -> Self {
        let q = quality.clamp(1, 100);
        self.jpeg.quality = q;
        self.png.quality = q;
        self.webp.quality = q;
        self.avif.quality = q;
        self.jxl.quality = q;
        self.gif.quality = q;
        self
    }

    /// Enable/disable metadata stripping
    pub fn with_strip_metadata(mut self, strip: bool) -> Self {
        self.strip_metadata = strip;
        self
    }

    /// Preserve original ICC profile (true) or normalize to P3 (false)
    pub fn with_preserve_icc(mut self, preserve: bool) -> Self {
        self.preserve_icc = preserve;
        self
    }

    /// Enable lossless mode for all formats
    pub fn with_lossless(mut self, lossless: bool) -> Self {
        self.jpeg.lossless = lossless;
        self.png.lossless = lossless;
        self.webp.lossless = lossless;
        self.jxl.lossless = lossless;
        self
    }

    /// Set chroma subsampling mode (JPEG)
    pub fn with_chroma_subsampling(
        mut self,
        subsampling: crate::formats::jpeg::ChromaSubsampling,
    ) -> Self {
        self.jpeg.chroma_subsampling = subsampling;
        self
    }

    /// Enable/disable progressive JPEG encoding
    pub fn with_progressive(mut self, progressive: bool) -> Self {
        self.jpeg.progressive = progressive;
        self
    }

    /// Set PNG optimization level (0-6)
    pub fn with_png_optimization(mut self, level: u8) -> Self {
        self.png.optimization_level = level.min(6);
        self
    }
}
