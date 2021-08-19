#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate thiserror;

use crate::generator::{
    args::{Multisampling, Smoothing},
    cpu::CpuFractalGenerator,
    gpu::GpuFractalGenerator,
    row_stitcher::RowStitcher,
    view::View,
    FractalGenerator, FractalOpts,
};
use futures::task::Poll;
use mtpng::{encoder, ColorType, Header};
use num_complex::Complex32;
use std::{
    fs::File,
    io::BufWriter,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{sync::mpsc, task};
use wgpu::{BackendBit, Instance, Maintain, RequestAdapterOptions};

mod generator;
mod logging;

const IMAGE_WIDTH: u32 = 4096;
const IMAGE_HEIGHT: u32 = 4096;

const CHUNK_WIDTH: usize = 256;
const CHUNK_HEIGHT: usize = 256;

const CHUNK_BACKLOG: usize = 32;

#[tokio::main]
async fn main() {
    logging::init();
    info!("Hello from fractal-rs-2");

    let view = View::new_centered_uniform(IMAGE_WIDTH as usize, IMAGE_HEIGHT as usize, 3.0);
    let opts = FractalOpts {
        mandelbrot: false,
        iterations: 200,
        smoothing: Smoothing::from_logarithmic_distance(4.0, 2.0),
        multisampling: Multisampling::Points { offset: 0.25 },
        c: Complex32 {
            re: 0.16611,
            im: 0.59419,
        },
    };

    let views: Vec<_> = view
        .subdivide_rectangles(CHUNK_WIDTH, CHUNK_HEIGHT)
        .collect();

    info!("Creating Instance...");
    let instance = Instance::new(BackendBit::PRIMARY);
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: Default::default(),
            compatible_surface: None,
        })
        .await
        .unwrap();

    info!("Requesting device...");
    let (device, queue) = adapter
        .request_device(&Default::default(), None)
        .await
        .unwrap();
    let queue = Arc::new(queue);

    info!("Creating device poll task...");
    let device = Arc::new(device);
    let poll_device = device.clone();
    let status = Arc::new(AtomicBool::new(true));
    let poll_status = status.clone();
    let poll_task = tokio::spawn(async move {
        while poll_status.load(Ordering::Relaxed) {
            poll_device.poll(Maintain::Poll);
            task::yield_now().await;
        }
    });

    info!("Creating generator...");
    let gen = GpuFractalGenerator::new(opts, device, queue).await.unwrap();
    // let gen = CpuFractalGenerator::new(opts, 10).unwrap();

    info!("Opening output file...");

    let mut stream_writer = Some(
        tokio::task::spawn_blocking(|| {
            let output_file = File::create("output.png").unwrap();
            let file_writer = BufWriter::new(output_file);
            let options = encoder::Options::new();
            let mut encoder = encoder::Encoder::new(file_writer, &options);
            let mut header = Header::new();
            header.set_size(IMAGE_WIDTH, IMAGE_HEIGHT).unwrap();
            header.set_color(ColorType::TruecolorAlpha, 8).unwrap();
            encoder.write_header(&header).unwrap();
            encoder
        })
        .await
        .unwrap(),
    );

    info!("Creating channel...");
    let (tx, mut rx) = mpsc::channel(CHUNK_BACKLOG);

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
                    moved_writer.write_image_rows(&row.image).unwrap();
                    moved_writer
                })
                .await
                .unwrap(),
            );
        }
    }

    info!("Finishing output file...");

    tokio::task::spawn_blocking(move || {
        stream_writer.unwrap().finish().unwrap();
    })
    .await
    .unwrap();

    info!("Shutting down...");

    status.store(false, Ordering::Relaxed);
    poll_task.await.unwrap();

    info!("Done.");
}
