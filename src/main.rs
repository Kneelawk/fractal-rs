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

mod generator;

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
    println!("Generating fractal...");

    let view = View::new_uniform(IMAGE_WIDTH, IMAGE_HEIGHT, PLANE_WIDTH, CENTER_X, CENTER_Y);
    let smoothing = Smoothing::from_logarithmic_distance(4f32, 2f32);
    let opts = FractalOpts::new(MANDELBROT, ITERATIONS, smoothing, C);

    let generator = CpuFractalGenerator::new(opts, THREADS);

    let buf = BufWriter::new(File::create(Path::new(OUT_FILE)).unwrap());
    let mut encoder = png::Encoder::new(buf, IMAGE_WIDTH as u32, IMAGE_HEIGHT as u32);
    encoder.set_color(ColorType::RGBA);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let mut stream = writer.stream_writer_with_size(CHUNK_SIZE);

    let chunks = view.subdivide_to_pixel_count(CHUNK_SIZE);
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
