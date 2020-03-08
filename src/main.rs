use crate::generator::{args::Smoothing, view::View, FractalOpts};
use num_complex::Complex32;
use std::{fs::File, io::BufWriter, path::Path};
use png::{ColorType, BitDepth};

mod generator;

const IMAGE_WIDTH: u32 = 1100;
const IMAGE_HEIGHT: u32 = 850;
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

    let mut buf = BufWriter::new(File::create(Path::new(OUT_FILE)).unwrap());
    let mut encoder = png::Encoder::new(buf, IMAGE_WIDTH, IMAGE_HEIGHT);
    encoder.set_color(ColorType::RGBA);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let mut stream = writer.stream_writer_with_size(CHUNK_SIZE);

    let chunks = view.subdivide_to_pixel_count(CHUNK_SIZE);
}
