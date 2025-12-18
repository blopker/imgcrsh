//! Pipeline configuration types

/// Resampling filter types for spatial transformation
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FilterType {
    Nearest,
    Bilinear,
    Bicubic,
    #[default]
    Lanczos3,
}

/// Chroma subsampling modes for JPEG/AVIF encoding
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChromaSubsampling {
    /// Full chroma resolution (no subsampling)
    Yuv444,
    /// Horizontal subsampling only
    Yuv422,
    /// Both horizontal and vertical subsampling (default)
    #[default]
    Yuv420,
}

/// Output format specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Jpeg,
    // Future: Png, WebP, Avif, JpegXl, Tiff, Gif
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Jpeg
    }
}

/// JPEG-specific encoding options
#[derive(Debug, Clone)]
pub struct JpegOptions {
    /// Enable lossless mode (100% quality, disables DCT quantization)
    pub lossless: bool,
    /// Quality level 1-100 (ignored if lossless)
    pub quality: u8,
    /// Enable progressive scan encoding
    pub progressive: bool,
    /// Chroma subsampling (forced to 4:4:4 if lossless)
    pub chroma_subsampling: ChromaSubsampling,
}

impl Default for JpegOptions {
    fn default() -> Self {
        Self {
            lossless: false,
            quality: 75,
            progressive: true,
            chroma_subsampling: ChromaSubsampling::Yuv420,
        }
    }
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
    pub jpeg: JpegOptions,
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
            jpeg: JpegOptions::default(),
        }
    }
}

impl PipelineConfig {
    /// Create a new configuration with default settings
    pub fn new() -> Self {
        Self::default()
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
        self
    }

    /// Set chroma subsampling mode
    pub fn with_chroma_subsampling(mut self, subsampling: ChromaSubsampling) -> Self {
        self.jpeg.chroma_subsampling = subsampling;
        self
    }

    /// Enable/disable progressive JPEG encoding
    pub fn with_progressive(mut self, progressive: bool) -> Self {
        self.jpeg.progressive = progressive;
        self
    }
}
