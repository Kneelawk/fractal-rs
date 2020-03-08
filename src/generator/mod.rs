use crate::generator::{args::Smoothing, view::View};
use num_complex::Complex;
use std::{
    mem::transmute,
    sync::{mpsc::Sender, Arc, Mutex},
};

pub mod args;
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
    view: View,
    image: Box<[u8]>,
}

/// Error returned if there is a problem starting a fractal generator.
pub enum FractalGenerationStartError {
    AlreadyRunning,
}

/// Represents an RGBA color, with 8 bits per channel.
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct RGBAColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Structs implementing this trait can be used to generate fractals.
pub trait FractalGenerator {
    /// Starts the generation of a fractal. Results are sent in the same order
    /// that views are presented in the `views` iterator.
    fn start_generation<Views>(
        self: &Arc<Self>,
        views: Arc<Mutex<Views>>,
        result: Sender<FractalGenerationMessage>,
    ) -> Result<(), FractalGenerationStartError>
    where
        Views: ExactSizeIterator<Item = View> + Send + 'static;

    /// Gets the current progress of the fractal generator through all the views
    /// assuming each view is the same size.
    fn get_progress(&self) -> f32;
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

impl RGBAColor {
    /// Creates a new RGBAColor from the given color byte values.
    pub fn new(red: u8, green: u8, blue: u8, alpha: u8) -> RGBAColor {
        RGBAColor {
            r: red,
            g: green,
            b: blue,
            a: alpha,
        }
    }

    /// Creates a new RGBAColor from these HSBA values. All HSBA values must be
    /// in the range 0..1.
    pub fn from_hsb(hue: f32, saturation: f32, brightness: f32, alpha: f32) -> RGBAColor {
        let alpha = (alpha * 255f32 + 0.5f32) as u8;
        if saturation == 0f32 {
            let brightness = (brightness * 255f32 + 0.5f32) as u8;
            RGBAColor {
                r: brightness,
                g: brightness,
                b: brightness,
                a: alpha,
            }
        } else {
            let sector = (hue - hue.floor()) * 6f32;
            let offset_in_sector = sector - sector.floor();
            let off = brightness * (1f32 - saturation);
            let fade_out = brightness * (1f32 - saturation * offset_in_sector);
            let fade_in = brightness * (1f32 - saturation * (1f32 - offset_in_sector));
            match sector as u32 {
                0 => RGBAColor {
                    r: (brightness * 255f32 + 0.5f32) as u8,
                    g: (fade_in * 255f32 + 0.5f32) as u8,
                    b: (off * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                1 => RGBAColor {
                    r: (fade_out * 255f32 + 0.5f32) as u8,
                    g: (brightness * 255f32 + 0.5f32) as u8,
                    b: (off * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                2 => RGBAColor {
                    r: (off * 255f32 + 0.5f32) as u8,
                    g: (brightness * 255f32 + 0.5f32) as u8,
                    b: (fade_in * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                3 => RGBAColor {
                    r: (off * 255f32 + 0.5f32) as u8,
                    g: (fade_out * 255f32 + 0.5f32) as u8,
                    b: (brightness * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                4 => RGBAColor {
                    r: (fade_in * 255f32 + 0.5f32) as u8,
                    g: (off * 255f32 + 0.5f32) as u8,
                    b: (brightness * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                5 => RGBAColor {
                    r: (brightness * 255f32 + 0.5f32) as u8,
                    g: (off * 255f32 + 0.5f32) as u8,
                    b: (fade_out * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                _ => unreachable!("Invalid color wheel sector"),
            }
        }
    }
}

impl Into<[u8; 4]> for RGBAColor {
    fn into(self) -> [u8; 4] {
        unsafe { transmute(self) }
    }
}
