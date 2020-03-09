use crate::generator::{
    args::Smoothing,
    cpu::CpuFractalGenerator,
    view::View,
    FractalGenerator,
    FractalOpts,
};
use mtpng::{
    encoder::{Encoder, Options},
    ColorType,
    Header,
};
use num_complex::Complex32;
use std::{
    cmp::max,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::{mpsc::sync_channel, Arc, Mutex},
};

mod generator;

const IMAGE_WIDTH: usize = 11000;
const IMAGE_HEIGHT: usize = 8500;
const PLANE_WIDTH: f32 = 3f32;
const CENTER_X: f32 = -0.75f32;
const CENTER_Y: f32 = 0f32;
const MANDELBROT: bool = true;
const ITERATIONS: u32 = 100;
const C: Complex32 = Complex32 { re: 0f32, im: 0f32 };
const THREADS: usize = 10;
const OUT_FILE: &'static str = "fractal.png";
const CHUNK_SIZE: usize = 1048576;

fn main() {
    println!("Generating fractal...");

    let view = View::new_uniform(IMAGE_WIDTH, IMAGE_HEIGHT, PLANE_WIDTH, CENTER_X, CENTER_Y);
    let smoothing = Smoothing::from_logarithmic_distance(4f32, 2f32);
    let opts = FractalOpts::new(MANDELBROT, ITERATIONS, smoothing, C);

    let generator = CpuFractalGenerator::new(opts, THREADS);

    let buf = BufWriter::new(File::create(Path::new(OUT_FILE)).unwrap());
    let mut header = Header::new();
    header
        .set_size(IMAGE_WIDTH as u32, IMAGE_HEIGHT as u32)
        .unwrap();
    header.set_color(ColorType::TruecolorAlpha, 8).unwrap();
    let options = Options::new();
    let mut encoder = Encoder::new(buf, &options);
    encoder.write_header(&header).unwrap();

    let chunks = view.subdivide_to_pixel_count(max(CHUNK_SIZE, IMAGE_WIDTH));
    let chunk_count = chunks.len();

    println!("Starting generation...");

    let (tx, rx) = sync_channel(32);

    let view_iter = Arc::new(Mutex::new(chunks.into_iter()));

    generator.start_generation(view_iter.clone(), tx).unwrap();

    let mut height = 0;
    let mut index = 0;
    for message in rx {
        index += 1;
        println!("Received chunk {}/{}. Writing...", index, chunk_count);
        println!("Generator at: {}%", generator.get_progress() * 100f32);
        println!(
            "Generator on chunk {}/{}.",
            chunk_count - view_iter.lock().unwrap().len(),
            chunk_count
        );

        height += message.image.len() / IMAGE_WIDTH / 4;
        println!("Current height: {}/{}", height, IMAGE_HEIGHT);

        encoder.write_image_rows(&message.image).unwrap();
        encoder.flush().unwrap();
        println!("Writing complete.");
    }

    println!("Flushing...");
    encoder.flush().unwrap();

    println!("Finishing...");
    encoder.finish().unwrap();

    println!("Done.");
}
