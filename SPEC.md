Technical Specification: High-Fidelity Rust Image Pipeline (v2025.12)
This document outlines the architectural requirements and implementation strategy for a color-accurate, SIMD-accelerated image transformation pipeline in Rust. It prioritizes mathematical truth over legacy compatibility, targeting Display P3 as the primary wide-gamut output.

1. Core Dependency Stack


Component
	Crate
	Rationale
	Decoding
	image (Minimal)
	Decoupled entry point for multi-format ingestion.
	Metadata Parsing
	kamadak-exif
	Lightweight, read-only EXIF parser for pre-processing logic.
	Color Management
	moxcms
	Pure Rust, SIMD-accelerated (AVX2/NEON). Faster than lcms2.
	Resampling
	fast_image_resize
	Industry-leading performance; supports Lanczos3 filtering.
	Quantization
	quantette
	Utilizes Oklab color space for perceptually accurate palette generation.
	JPEG Encoding
	mozjpeg
	Advanced trellis quantization and progressive scan optimization.
	PNG Encoding
	oxipng
	Multi-threaded lossless optimization and Zopfli integration.
	WebP Encoding
	webp / libwebp
	Support for lossy and lossless static formats.
	AVIF Encoding
	ravif / libavif
	State-of-the-art compression using CICP flags for zero-metadata color mapping.
	JPEG XL Encoding
	zune-jpegxl / libjxl
	Modular format supporting lossless JPEG transcoding and native HDR/WCG.
	TIFF Encoding
	image-tiff
	Archival-grade lossless support for high bit-depth and native ICC tags.
	GIF Encoding
	gifski
	High-quality GIF encoder utilizing temporal and spatial dithering.
	2. Pipeline Configuration Options
Category
	Option
	Type
	Default
	Technical Description
	Normalization
	strip_metadata
	bool
	true
	When true, bakes Orientation/Flip into the bitstream and strips EXIF.
	

	color_normalization
	bool
	true
	Enforces target gamut transformation (e.g., to Display P3).
	Resampling
	width / height
	Option<u32>
	None
	Target dimensions. Resizer respects aspect ratio if one is None.
	

	filter_type
	enum
	Lanczos3
	Options: Nearest, Bilinear, Bicubic, Lanczos3.
	

	linear_resampling
	bool
	true
	Performs math in linear light (f32) to prevent energy loss.
	JPEG
	lossless
	bool
	false
	Enables 100% quality lossless mode (strips DCT quantization).
	

	quality
	u8
	75
	JPEG quality (1-100). Triggers Trellis quantization if lossless is false.
	

	progressive
	bool
	true
	Enables progressive scan encoding for 2-10% better compression.
	

	chroma_subsampling
	enum
	4:2:0
	Options: 4:4:4, 4:2:2, 4:2:0. Forced to 4:4:4 if lossless.
	PNG
	lossless_level
	u8 (0-6)
	2
	oxipng optimization level. 6 enables exhaustive filtering.
	

	lossy_quantize
	bool
	false
	Pre-quantizes to 256 colors using Oklab before encoding.
	WebP
	lossless
	bool
	false
	Switches to VP8L lossless encoding.
	

	quantized_lossy
	bool
	false
	Uses Oklab pre-quantization + lossless for "clean" illustratives.
	

	sns_strength
	u8
	80
	Spatial Noise Shaping strength for native lossy encoding.
	AVIF
	speed
	u8 (0-10)
	4
	Encoding effort vs speed. 0 is slowest/highest quality.
	

	quantizer
	u8 (0-63)
	25
	Lower values are higher quality.
	JPEG XL
	effort
	u8 (1-9)
	7
	Encoding effort level. Higher is slower but better compression.
	

	distance
	f32 (0-15)
	1.0
	Butteraugli perceptual distance. 0.0 is mathematically lossless.
	GIF
	gifski_quality
	u8
	90
	Perceptual quality slider for gifski palette/dither engine.
	Global
	dssim_threshold
	f64
	0.01
	Perceptual budget; rejects output if structural drift is too high.
	3. Architectural Constraints: Pure CPU Execution
To maintain maximum performance and predictable latency, the pipeline adheres to the following constraints:
1. Strictly Synchronous: No async/await syntax. The pipeline is a blocking, CPU-bound operation designed to saturate local compute resources.
2. Single-Context Execution: The pipeline handles a single unit of work (static image) from memory-to-memory. Parallelization (e.g., rayon iteration over multiple images) must be implemented by the orchestrator.
3. I/O Isolation: No filesystem or network access. The input is a &[u8] (encoded bytes) and the output is a Vec<u8> (encoded bytes).
4. The Architectural Pipeline
Phase A: Ingestion & Conditional Normalization
A.0: The Normalization Trigger (Bake-in)
Physical attribute normalization (Rotation/Flip) is conditional.
* IF strip_metadata == true: The pipeline must resolve "metadata-dependent" states into "pixel-defined" states. Parse EXIF Orientation and apply the corresponding affine transformation to the raw buffer. Reset output Orientation tag to 1.
* ELSE: Pass metadata markers through to the encoder. Note: Aspect ratios for the resizing pass in Phase B must be calculated based on the intended orientation, not the raw pixel dimensions.
A.1: Independent Color Management
Color space handling is a sovereign architectural primitive.
1. Inference: Identify source gamut via ICC profile or EXIF ColorSpace (Value 2 = Adobe RGB).
2. Standardization: Transform from inferred space to Linear f32 Space.
   * Note: If color_normalization == true, the target is Linear Display P3. Otherwise, use Linear sRGB.
Phase B: Spatial Transformation (Resizing)
1. Precision Resampling: The f32 buffer is resampled using fast_image_resize with Lanczos3.
2. Chroma Alignment: When targeting formats with chroma subsampling (JPEG/AVIF/JXL/Lossy WebP 4:2:0), ensure output dimensions are even integers to prevent "edge bleed" or macroblock misalignment.
3. Boundary Handling: Use fast_image_resize's edge-aware tiling to prevent "alpha halos" in transparent regions.
Phase C: Color Mapping & Quantization
1. Gamut Mapping: Use moxcms to map from the linear workspace to the target Non-linear Space.
2. Intent: Use Perceptual Intent for graceful gamut clipping.
3. Dithering: Apply Floyd-Steinberg dithering during f32 $\rightarrow$ u8 conversion to mitigate banding in 8-bit wide-gamut containers.
Phase D: Format-Specific Optimization
1. JPEG (Lossy & Lossless)
* Engine: mozjpeg.
* Lossless Mode: Utilizes the arithmetic coding or 100% quality path in MozJPEG. This disables DCT quantization entirely.
* Constraint: Lossless JPEG mandates 4:4:4 chroma subsampling.
* Metadata: IF color_normalization == true, inject the Nano Display P3 ICC (~450 bytes).
2. PNG (Lossless vs. Lossy)
* Lossless: oxipng preset 6.
* Lossy: Apply quantette (Oklab space) $\rightarrow$ 256-color palette $\rightarrow$ Dithered remap $\rightarrow$ oxipng.
3. WebP (Lossless vs. Lossy)
* Lossless: lossless: true, exact: true.
* Standard Lossy: Method 6, sns_strength at 80.
* High-Compression Lossy (Quantized): Apply quantette (Oklab space) before lossless encoding.
4. AVIF (Next-Gen Lossy)
* Engine: ravif / libavif.
* Color Management: Use CICP Flags (12/13/0 for Display P3) in the bitstream.
5. JPEG XL (The Modern Standard)
* Engine: zune-jpegxl / libjxl.
* Strategy: Use VarDCT for lossy (perceptual) and Modular for lossless.
* JPEG Transcoding: If the input is a JPEG and the output is JXL, the pipeline should perform a Lossless Transcode.
6. TIFF (Archival Lossless)
* Engine: image-tiff.
* Strategy: Preserve 16-bit depth if the source is 16-bit. Inject full-resolution ICC profile.
7. GIF (High-Fidelity)
* Engine: gifski.
* Strategy: Enforce sRGB transformation in Phase C.
5. Buffer & Memory Strategy
1. Zero-Copy Intent: Use bytemuck for safe casting between raw u8 buffers and f32 pixel arrays where alignment permits.
2. Alignment: Ensure buffers are 16-byte or 32-byte aligned to maximize the efficiency of SIMD intrinsics.
6. Error Handling & Recovery
1. Corrupt Bitstreams: Gracefully catch panics from format decoders using catch_unwind where FFI is involved.
2. OOM Guard: For high-resolution ingestion (e.g., 100MP+), implement a "header-only" check to reject files that exceed system memory limits.
7. Metadata Policy: "Truth over Noise"
1. Strip (Optional): Removes EXIF, GPS, and IPTC. Physical attributes are moved to the bitstream (Bake-in).
2. Color Normalization (Independent): Enforces target gamut (Display P3) regardless of stripping state.
3. Preserve: Only essential dimensions and, if normalized, the minified ICC profile (or CICP flags for AVIF/JXL).
8. Performance Expectations
Throughput on modern hardware should target >1.5 GB/s.
9. Verification & Perceptual Integrity (Testing)
1. Ground Truth: Use dssim for regression testing.
2. The "Perceptual Budget": Rejects output if DSSIM value exceeds 0.01.
3. Gamut Validation: Automated verification of ICC profile and pixel alignment via moxcms.
