#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate thiserror;

use crate::{
    generator::{
        args::{Multisampling, Smoothing},
        cpu::CpuFractalGenerator,
        gpu::GpuFractalGenerator,
        row_stitcher::RowStitcher,
        view::View,
        FractalGenerator, FractalOpts,
    },
    gui::flow::{Flow, FlowModel},
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
    time::Duration,
};
use tokio::{sync::mpsc, task};
use wgpu::{
    Backends, Device, Instance, Maintain, Queue, RequestAdapterOptions, TextureFormat, TextureView,
};
use winit::{dpi::PhysicalSize, event::WindowEvent, event_loop::ControlFlow};

mod generator;
mod gui;
mod logging;

const IMAGE_WIDTH: u32 = 4096;
const IMAGE_HEIGHT: u32 = 4096;

const CHUNK_WIDTH: usize = 256;
const CHUNK_HEIGHT: usize = 256;

const CHUNK_BACKLOG: usize = 32;

fn main() {
    logging::init();
    info!("Hello from fractal-rs-2");

    let mut flow = Flow::new();
    flow.title = "Fractal-RS 2".to_string();

    flow.start::<FractalRSMain>();
}

struct FractalRSMain {}

#[async_trait]
impl FlowModel for FractalRSMain {
    async fn init(
        device: Arc<Device>,
        queue: Arc<Queue>,
        window_size: PhysicalSize<u32>,
        frame_format: TextureFormat,
    ) -> Self {
        FractalRSMain {}
    }

    async fn event(&mut self, event: WindowEvent<'async_trait>) -> ControlFlow {
        ControlFlow::Poll
    }

    async fn update(&mut self, update_delta: Duration) -> ControlFlow {
        ControlFlow::Poll
    }

    async fn render(&mut self, frame_view: &TextureView, render_delta: Duration) {}

    async fn shutdown(self) {}
}
