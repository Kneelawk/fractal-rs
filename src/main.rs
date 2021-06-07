//! fractal-viewer-rs
//!
//! This application generates a mandelbrot or a julia set fractal image. This
//! generator is multi-threaded. The generator can be configured through
//! command-line arguments and config files.

#[macro_use]
extern crate clap;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate serde_derive;

use crate::generator::{
    args::Smoothing,
    cpu::CpuFractalGenerator,
    view::View,
    FractalGenerator,
    FractalOpts,
};
use num_complex::Complex32;
use png::{BitDepth, ColorType};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::mpsc::sync_channel,
};

mod args;
mod config;
mod generator;
mod util;

const IMAGE_WIDTH: usize = 22000;
const IMAGE_HEIGHT: usize = 17000;
const PLANE_WIDTH: f32 = 3f32;
const CENTER_X: f32 = 0f32;
const CENTER_Y: f32 = 0f32;
const MANDELBROT: bool = false;
const ITERATIONS: u32 = 100;
const C: Complex32 = Complex32 {
    re: -0.059182f32,
    im: 0.669273f32,
};
const THREADS: usize = 10;
const OUT_FILE: &'static str = "fractal2-22000x17000.png";
const CHUNK_SIZE: usize = 1048576;

fn main() {
    let args = args::Args::parse().unwrap();
    if args.generate_config {
        println!("Generating config to {:?}", &args.config_path);
        config::Config::generate(&args.config_path).unwrap();
        println!("Config generated.");
        return;
    }

    println!("Loading config from {:?}", &args.config_path);
    let config = config::Config::load(&args.config_path).unwrap();

    println!("Generating fractal...");

    let opts = FractalOpts::new(
        config.mandelbrot,
        config.iterations,
        config.smoothing,
        config.c,
    );

    let generator = CpuFractalGenerator::new(opts, config.thread_count);

    let path = util::find_filename(&config);
    let buf = BufWriter::new(File::create(&path).unwrap());
    let mut encoder = png::Encoder::new(
        buf,
        config.view.image_width as u32,
        config.view.image_height as u32,
    );
    encoder.set_color(ColorType::RGBA);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let mut stream = writer.stream_writer_with_size(config.chunk_size);

    let chunks = config.view.subdivide_to_pixel_count(config.chunk_size);
    let chunk_count = chunks.len();

    println!("Starting generation...");

    let (tx, rx) = sync_channel(32);

    generator.start_generation(chunks.collect(), tx).unwrap();

    let mut index = 0;
    for message in rx {
        index += 1;
        println!("Received chunk {}/{}. Writing...", index, chunk_count);
        println!("Generator at: {:.1}%", generator.get_progress() * 100f32);

        let image_len = message.image.len();
        let mut offset = 0;
        while offset < image_len {
            offset += stream.write(&message.image[offset..]).unwrap();
            stream.flush().unwrap();
        }
        println!("Writing complete.");
    }

    stream.flush().unwrap();

    println!("Done.");
}
