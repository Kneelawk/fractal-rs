#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

use crate::generator::{
    args::Smoothing, cpu::CpuFractalGenerator, row_stitcher::RowStitcher, view::View,
    FractalGenerator, FractalOpts, BYTES_PER_PIXEL,
};
use futures::{task::Poll, StreamExt};
use num_complex::Complex32;
use png::{BitDepth, ColorType};
use std::{
    fs::File,
    io::{BufWriter, Write},
};
use tokio::sync::mpsc;

mod generator;
mod logging;

const IMAGE_WIDTH: u32 = 4096;
const IMAGE_HEIGHT: u32 = 4096;

const CHUNK_WIDTH: usize = 256;
const CHUNK_HEIGHT: usize = 256;

#[tokio::main]
async fn main() {
    logging::init();
    info!("Hello from fractal-rs-2");

    let view = View::new_centered_uniform(IMAGE_WIDTH as usize, IMAGE_HEIGHT as usize, 3.0);
    let opts = FractalOpts {
        mandelbrot: false,
        iterations: 100,
        smoothing: Smoothing::None,
        c: Complex32 {
            re: 0.16611,
            im: 0.59419,
        },
    };

    let views: Vec<_> = view
        .subdivide_rectangles(CHUNK_WIDTH, CHUNK_HEIGHT)
        .collect();

    info!("Creating generator...");

    let gen = CpuFractalGenerator::new(opts, 10).unwrap();

    info!("Opening output file...");

    let mut stream_writer = Some(
        tokio::task::spawn_blocking(|| {
            let output_file = File::create("output.png").unwrap();
            let mut file_writer = BufWriter::new(output_file);
            let mut encoder = png::Encoder::new(file_writer, IMAGE_WIDTH, IMAGE_HEIGHT);
            encoder.set_color(ColorType::RGBA);
            encoder.set_depth(BitDepth::Eight);
            let writer = encoder.write_header().unwrap();
            writer
                .into_stream_writer_with_size(IMAGE_WIDTH as usize * CHUNK_HEIGHT * BYTES_PER_PIXEL)
        })
        .await
        .unwrap(),
    );

    info!("Creating channel...");
    let (tx, mut rx) = mpsc::channel(32);

    info!("Starting generation...");
    let _instance = gen.start_generation(&views, tx).await.unwrap();

    info!("Creating row stitcher...");
    let mut stitcher = RowStitcher::new(view, &views);

    info!("Starting receiver loop...");
    while let Some(block) = rx.recv().await {
        let block = block.unwrap();
        info!(
            "Received block at ({}, {})",
            block.view.image_x, block.view.image_y
        );
        stitcher.insert(block);

        while let Poll::Ready(Some(row)) = stitcher.stitch() {
            let mut moved_writer = stream_writer.take().unwrap();
            info!("Writing row at y={}", row.view.image_y);

            stream_writer = Some(
                tokio::task::spawn_blocking(move || {
                    moved_writer.write_all(&row.image);
                    moved_writer
                })
                .await
                .unwrap(),
            );
        }
    }

    info!("Finishing output file...");

    tokio::task::spawn_blocking(move || {
        stream_writer.unwrap().finish();
    })
    .await
    .unwrap();

    info!("Done.");
}
