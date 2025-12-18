# imgcrsh Implementation Progress

Tracking implementation of the [Technical Specification](SPEC.md) for the High-Fidelity Rust Image Pipeline.

## Core Infrastructure

### Dependencies
- [x] `image` - Multi-format decoding
- [x] `kamadak-exif` - EXIF parsing
- [x] `moxcms` - Color management (SIMD-accelerated)
- [x] `fast_image_resize` - Lanczos3 resampling
- [x] `imagequant` - RGBA quantization for lossy PNG (pngquant library)
- [x] `mozjpeg` - JPEG encoding
- [x] `oxipng` - PNG optimization
- [x] `webp` - WebP encoding
- [ ] `ravif` - AVIF encoding
- [ ] `zune-jpegxl` / `libjxl` - JPEG XL encoding
- [ ] `gifski` - GIF encoding

### Dev Dependencies
- [x] `dssim` - Perceptual quality regression tests

### Architecture
- [x] Synchronous, blocking pipeline
- [x] Memory-to-memory processing (`&[u8]` → `Vec<u8>`)
- [x] Single-context execution
- [ ] Buffer alignment (16/32-byte) for SIMD
- [ ] `bytemuck` zero-copy casting

---

## Phase A: Ingestion & Normalization

### A.0: Physical Normalization (EXIF Orientation)
- [x] Parse EXIF Orientation tag (values 1-8)
- [x] Apply rotation/flip transforms (always baked since EXIF won't survive re-encoding)
- [x] Pixel-level transforms for all 8 orientation values
- [x] Dimension swapping for 90°/270° rotations

### A.1: Color Management
- [x] ICC profile extraction from JPEG APP2 markers
- [x] Color space detection from ICC profile description
- [x] EXIF ColorSpace tag fallback (value 2 = Adobe RGB)
- [x] `preserve_icc` option to keep original ICC profile
- [x] Normalize to Display P3 when source has profile and `preserve_icc: false`
- [x] Keep sRGB for untagged images
- [x] `ColorTransformer` struct with moxcms integration
- [x] Preserve original P3 profile when no transform needed (avoids tone curve changes)
- [ ] Perceptual intent for gamut clipping
- [ ] Floyd-Steinberg dithering for f32→u8 conversion

---

## Phase B: Spatial Transformation

### Resampling
- [x] `fast_image_resize` integration
- [x] Filter types: Nearest, Bilinear, Bicubic, Lanczos3
- [x] Linear light resampling (`create_srgb_mapper`)
- [x] Aspect ratio preservation
- [ ] Chroma alignment (even dimensions for 4:2:0)
- [ ] Edge-aware tiling for alpha halos

---

## Phase C: Color Mapping & Quantization

- [x] Gamut mapping via moxcms
- [x] RGBA quantization via imagequant (for lossy PNG)
- [x] Dithering for smooth gradients (imagequant built-in)
- [ ] Perceptual intent rendering
- [ ] Floyd-Steinberg dithering for f32→u8 conversion

---

## Phase D: Format-Specific Encoding

### JPEG
- [x] mozjpeg encoder integration
- [x] Quality setting (1-100)
- [x] Progressive scan encoding
- [x] Chroma subsampling (4:4:4, 4:2:2, 4:2:0)
- [x] Lossless mode (100% quality, forced 4:4:4)
- [x] ICC profile injection (custom implementation with correct 1-indexed chunks)
  - Note: mozjpeg-rust has a bug with 0-indexed chunk numbers
- [ ] Trellis quantization optimization

### PNG
- [x] oxipng integration
- [x] Lossless optimization levels (0-6)
- [x] ICC profile injection
- [x] Lossy mode via imagequant (pngquant library)
- [x] 256-color palette generation with full RGBA support
- [x] Dithered remap for smooth gradients
- [x] Quality setting (0-100) for lossy mode
- Note: Quantized formats stay in sRGB (imagequant is sRGB-optimized)

### WebP
- [x] libwebp integration
- [x] Lossless mode (VP8L)
- [x] Lossy mode with quality setting (method 6 for best compression)
- [x] ICC profile embedding via VP8X extended format
- [ ] Advanced options (SNS strength, near-lossless)
- [ ] Quantized lossy mode (Oklab + lossless)

### AVIF
- [ ] ravif/libavif integration
- [ ] Speed/effort setting (0-10)
- [ ] Quantizer setting (0-63)
- [ ] CICP flags for Display P3 (12/13/0)

### JPEG XL
- [ ] zune-jpegxl/libjxl integration
- [ ] Effort setting (1-9)
- [ ] Distance setting (Butteraugli, 0-15)
- [ ] VarDCT for lossy
- [ ] Modular for lossless
- [ ] Lossless JPEG transcoding

### TIFF
- [ ] image-tiff integration
- [ ] 16-bit depth preservation
- [ ] Full ICC profile injection

### GIF
- [ ] gifski integration
- [ ] Quality setting
- [ ] sRGB enforcement
- [ ] Temporal/spatial dithering

---

## Configuration Options

### Implemented
- [x] `strip_metadata: bool` (default: true)
- [x] `preserve_icc: bool` (default: false) - keep original ICC profile
- [x] `width: Option<u32>` / `height: Option<u32>`
- [x] `filter_type: enum` (Nearest, Bilinear, Bicubic, Lanczos3)
- [x] `linear_resampling: bool` (default: true)
- [x] `jpeg.lossless: bool` (default: false)
- [x] `jpeg.quality: u8` (default: 75)
- [x] `jpeg.progressive: bool` (default: true)
- [x] `jpeg.chroma_subsampling: enum` (default: 4:2:0)
- [x] `png.optimization_level: u8` (0-6)
- [x] `png.lossless: bool` (default: true)
- [x] `png.quality: u8` (default: 90) - for lossy mode
- [x] `webp.lossless: bool` (default: false)
- [x] `webp.quality: u8` (default: 80)

### Not Yet Implemented
- [ ] `webp.quantized_lossy: bool`
- [ ] `webp.sns_strength: u8`
- [ ] `webp.method: u8` (0-6)
- [ ] `avif.speed: u8` (0-10)
- [ ] `avif.quantizer: u8` (0-63)
- [ ] `jxl.effort: u8` (1-9)
- [ ] `jxl.distance: f32` (0-15)
- [ ] `gif.gifski_quality: u8`

---

## Error Handling & Safety

- [ ] `catch_unwind` for FFI decoder panics
- [ ] OOM guard for high-resolution images (100MP+)
- [ ] Header-only dimension check

---

## Testing & Verification

### Unit Tests
- [x] Dimension calculation tests
- [x] Color space info defaults
- [x] ICC profile generation (sRGB, Display P3)

### Integration Tests
- [x] Basic JPEG passthrough
- [x] JPEG resize with aspect ratio
- [x] JPEG quality levels
- [x] Progressive JPEG encoding
- [x] Lossless JPEG mode
- [x] Color normalization to P3
- [x] Color normalization disabled
- [x] Linear resampling
- [x] Non-linear resampling

### Verification (Dev/Test Only)
- [ ] DSSIM regression tests (dev dependency)
- [ ] Gamut validation tests via moxcms
- [ ] Performance benchmarks (target: >1.5 GB/s)

---

## Current File Structure

```
src/
├── lib.rs              # Public API exports
├── config.rs           # Pipeline configuration structs
├── color.rs            # Color space detection & transforms
├── orientation.rs      # EXIF orientation parsing & transforms
├── pipeline.rs         # Core processing pipeline
├── main.rs             # CLI interface
└── formats/
    ├── mod.rs          # Encoder trait & re-exports
    ├── jpeg.rs         # JPEG encoding (mozjpeg + ICC injection fix)
    ├── png.rs          # PNG encoding (oxipng + imagequant)
    └── webp.rs         # WebP encoding (libwebp)

tests/
└── jpeg_pipeline.rs    # Integration tests

scratch/                # Development utilities (not part of library)
```

---

## Next Steps (Suggested Priority)

1. **AVIF Encoding** - Modern format with CICP color management
2. **Chroma Alignment** - Ensure even dimensions for subsampled formats
3. **DSSIM Tests** - Add perceptual regression tests (dev-only)
4. **CLI Enhancements** - Add more options to main.rs (lossless flag, resize)
5. **JPEG XL Encoding** - Future-proof format support
