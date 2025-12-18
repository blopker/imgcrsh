//! imgcrsh - High-Fidelity Rust Image Pipeline
//!
//! A color-accurate, SIMD-accelerated image transformation pipeline
//! targeting Display P3 as the primary wide-gamut output.

mod color;
mod config;
mod pipeline;

pub use color::{ColorSpaceInfo, ColorTransformer, SourceColorSpace};
pub use config::*;
pub use pipeline::process;
