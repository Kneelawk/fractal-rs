//! This crate is the primary API that various fractal generator backends will implement.

pub mod generator;
pub mod args;
mod util;

/// Rug multi-precision complex numbers are used by this API.
pub use rug;
