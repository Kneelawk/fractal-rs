//! This module contains the traits and structures necessary for describing the fractal generation
//! process.

use futures_core::future::BoxFuture;

pub trait FractalGenerator {
    fn create_instance() -> BoxFuture<'static, anyhow::Result<Box<dyn GeneratorInstance>>>;
}

pub trait GeneratorInstance {}
