//! This module contains the traits and structures necessary for describing the fractal generation
//! process.

use futures_core::future::BoxFuture;
use rug::Complex;
use crate::args::{Multisampling, Smoothing};

/// Represents a set of options passed to a fractal generator at initialization.
#[derive(Debug, Clone, PartialEq)]
pub struct FractalOpts {
    pub mandelbrot: bool,
    pub iterations: u32,
    pub smoothing: Smoothing,
    pub multisampling: Multisampling,
    pub c: Complex,
    pub radius_squared: f32,
}

pub trait FractalGenerator {
    fn create_instance(opts: FractalOpts) -> BoxFuture<'static, anyhow::Result<Box<dyn GeneratorInstance>>>;
}

pub trait GeneratorInstance {}
