//! This crate is the primary API that various fractal generator backends will implement.

pub mod generator;

/// Rug multi-precision complex numbers are used by this API.
pub use rug;
