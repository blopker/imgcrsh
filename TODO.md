# imgcrsh Implementation Progress

Tracking implementation of the [Technical Specification](SPEC.md) for the High-Fidelity Rust Image Pipeline.

## Core Infrastructure

### Dependencies
- [x] `image` - Multi-format decoding
- [x] `kamadak-exif` - EXIF parsing
- [x] `moxcms` - Color management (SIMD-accelerated)
- [x] `fast_image_resize` - Lanczos3 resampling
- [ ] `quantette` - Oklab quantization for lossy PNG/WebP
- [x] `mozjpeg` - JPEG encoding
- [x] `oxipng` - PNG optimization (dependency added, not integrated)
- [x] `webp` - WebP encoding (dependency added, not integrated)
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
- [ ] Parse EXIF Orientation tag
- [ ] Apply rotation/flip transforms when `strip_metadata: true`
- [ ] Pass through orientation when `strip_metadata: false`
- [ ] Calculate aspect ratios based on intended orientation

### A.1: Color Management
- [x] ICC profile extraction from JPEG APP2 markers
- [x] Color space detection from ICC profile description
- [x] EXIF ColorSpace tag fallback (value 2 = Adobe RGB)
- [x] Transform to Display P3 when `color_normalization: true`
- [x] Transform to sRGB when `color_normalization: false`
- [x] `ColorTransformer` struct with moxcms integration
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
- [ ] Perceptual intent rendering
- [ ] Floyd-Steinberg dithering
- [ ] Oklab quantization (via `quantette`)

---

## Phase D: Format-Specific Encoding

### JPEG
- [x] mozjpeg encoder integration
- [x] Quality setting (1-100)
- [x] Progressive scan encoding
- [x] Chroma subsampling (4:4:4, 4:2:2, 4:2:0)
- [x] Lossless mode (100% quality, forced 4:4:4)
- [x] ICC profile injection (`write_icc_profile`)
- [ ] Trellis quantization optimization

### PNG
- [ ] oxipng integration
- [ ] Lossless optimization levels (0-6)
- [ ] Lossy mode via Oklab quantization
- [ ] 256-color palette generation
- [ ] Dithered remap

### WebP
- [ ] libwebp integration
- [ ] Lossless mode (VP8L)
- [ ] Lossy mode (Method 6, SNS strength)
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
- [x] `strip_metadata: bool` (default: true) - *partial, no orientation yet*
- [x] `color_normalization: bool` (default: true)
- [x] `width: Option<u32>` / `height: Option<u32>`
- [x] `filter_type: enum` (Nearest, Bilinear, Bicubic, Lanczos3)
- [x] `linear_resampling: bool` (default: true)
- [x] `jpeg.lossless: bool` (default: false)
- [x] `jpeg.quality: u8` (default: 75)
- [x] `jpeg.progressive: bool` (default: true)
- [x] `jpeg.chroma_subsampling: enum` (default: 4:2:0)
- [ ] ~~`dssim_threshold`~~ - Removed (DSSIM is dev-only for regression tests)

### Not Yet Implemented
- [ ] `png.lossless_level: u8` (0-6)
- [ ] `png.lossy_quantize: bool`
- [ ] `webp.lossless: bool`
- [ ] `webp.quantized_lossy: bool`
- [ ] `webp.sns_strength: u8`
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
├── lib.rs          # Public API exports
├── config.rs       # Pipeline configuration structs
├── color.rs        # Color space detection & transforms
├── pipeline.rs     # Core processing pipeline
└── main.rs         # CLI interface

tests/
└── jpeg_pipeline.rs  # Integration tests
```

---

## Next Steps (Suggested Priority)

1. **EXIF Orientation** - Complete Phase A.0 normalization
2. **PNG Encoding** - Integrate oxipng for lossless output
3. **WebP Encoding** - Integrate libwebp for web-optimized output
4. **Chroma Alignment** - Ensure even dimensions for subsampled formats
5. **AVIF Encoding** - Modern format with CICP color management
6. **DSSIM Tests** - Add perceptual regression tests (dev-only)
