//! Pipeline configuration types

use crate::formats::{jpeg::JpegConfig, png::PngConfig};

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
    // Future: WebP, Avif, JpegXl, Tiff, Gif
}

/// Main pipeline configuration
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    // === Normalization ===
    /// When true, bakes Orientation/Flip into pixels and strips EXIF
    pub strip_metadata: bool,
    /// Enforces target gamut transformation (to Display P3)
    pub color_normalization: bool,

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
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            strip_metadata: true,
            color_normalization: true,
            width: None,
            height: None,
            filter_type: FilterType::Lanczos3,
            linear_resampling: true,
            output_format: OutputFormat::Jpeg,
            jpeg: JpegConfig::default(),
            png: PngConfig::default(),
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

    /// Set JPEG quality (1-100)
    pub fn with_quality(mut self, quality: u8) -> Self {
        self.jpeg.quality = quality.clamp(1, 100);
        self
    }

    /// Enable/disable metadata stripping
    pub fn with_strip_metadata(mut self, strip: bool) -> Self {
        self.strip_metadata = strip;
        self
    }

    /// Enable/disable color normalization to Display P3
    pub fn with_color_normalization(mut self, normalize: bool) -> Self {
        self.color_normalization = normalize;
        self
    }

    /// Enable lossless JPEG mode
    pub fn with_lossless(mut self, lossless: bool) -> Self {
        self.jpeg.lossless = lossless;
        self.png.lossless = lossless;
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
