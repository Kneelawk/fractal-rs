use crate::generator::{args::Smoothing, view::View};
use num_complex::Complex;
use std::sync::{mpsc::SyncSender, Arc, Mutex};

pub mod args;
pub mod color;
pub mod cpu;
pub mod view;

mod util;

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

/// Error returned if there is a problem starting a fractal generator.
#[derive(Debug, Copy, Clone)]
pub enum FractalGenerationStartError {
    AlreadyRunning,
}

/// Structs implementing this trait can be used to generate fractals.
pub trait FractalGenerator {
    /// Gets the recommended minimum number of views that should be submitted to
    /// this generator together as a single batch in order to operate
    /// efficiently.
    fn min_views_hint(&self) -> usize;

    /// Starts the generation of a fractal. Results are sent in the same order
    /// that views are presented in the `views` iterator.
    fn start_generation<Views>(
        self: &Arc<Self>,
        views: Arc<Mutex<Views>>,
        result: SyncSender<FractalGenerationMessage>,
    ) -> Result<(), FractalGenerationStartError>
    where
        Views: Iterator<Item = View> + Send + 'static;

    /// Gets the current progress of the fractal generator through all the views
    /// assuming each view is the same size.
    fn get_progress(&self) -> f32;

    /// Checks to see if this generator is currently running.
    fn running(&self) -> bool;
}

impl FractalOpts {
    /// Creates a new set of fractal options.
    pub fn new(
        mandelbrot: bool,
        iterations: u32,
        smoothing: Smoothing,
        c: Complex<f32>,
    ) -> FractalOpts {
        FractalOpts {
            mandelbrot,
            iterations,
            smoothing,
            c,
        }
    }
}
