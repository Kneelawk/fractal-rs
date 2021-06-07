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
    cpu::CpuFractalGenerator,
    FractalGenerator,
    FractalOpts,
};
use png::{BitDepth, ColorType};
use std::{
    fs::File,
    io::{BufWriter, Write},
    sync::mpsc::sync_channel,
};

mod args;
mod config;
mod generator;
mod util;

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
