pub mod args;
pub mod color;
pub mod composite;
pub mod cpu;
pub mod error;
pub mod util;
pub mod view;

use crate::generator::{args::Smoothing, error::GenError, view::View};
use futures::{future::BoxFuture, stream::BoxStream, Stream};
use num_complex::Complex;
use tokio::sync::mpsc::Sender;

/// Represents a set of options passed to a fractal generator at initialization.
#[derive(Debug, Copy, Clone)]
pub struct FractalOpts {
    pub mandelbrot: bool,
    pub iterations: u32,
    pub smoothing: Smoothing,
    pub c: Complex<f32>,
}

/// Represents a message from a fractal generator.
pub struct FractalGenerationMessage {
    pub view: View,
    pub image: Box<[u8]>,
}

/// Structs implementing this trait can be used to generate fractals.
///
/// Note: all these methods return futures because they may require
/// communication over a network.
pub trait FractalGenerator {
    /// Gets the recommended minimum number of views that should be submitted to
    /// this generator together as a single batch in order to operate
    /// efficiently.
    fn min_views_hint(&self) -> BoxFuture<usize>;

    /// Starts the generation of a fractal. Results are sent in the same order
    /// that views are presented in the `views` iterator.
    fn start_generation(
        &self,
        views: &[View],
    ) -> BoxFuture<Result<Box<dyn FractalGeneratorInstance>, GenError>>;
}

/// Represents a running fractal generator.
pub trait FractalGeneratorInstance {
    /// Gets this generator instance's message stream. This stream should output
    /// one FractalGenerationMessage for each view passed during its creation.
    fn stream(&self) -> BoxStream<Result<FractalGenerationMessage, GenError>>;

    /// Checks whether this fractal generator instance is still running.
    fn running(&self) -> BoxFuture<bool>;
}
