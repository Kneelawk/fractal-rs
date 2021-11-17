use crate::generator::{
    color::RGBA8Color, cpu::opts::CpuFractalOpts, view::View, FractalGenerator,
    FractalGeneratorFactory, FractalGeneratorInstance, FractalOpts, PixelBlock, BYTES_PER_PIXEL,
};
use cgmath::Vector4;
use futures::{
    future::{ready, BoxFuture, Ready},
    FutureExt,
};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    num::NonZeroU32,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc::{OwnedPermit, Sender};
use wgpu::{
    Device, Extent3d, ImageCopyTexture, ImageDataLayout, Origin3d, Queue, Texture, TextureAspect,
    TextureView,
};

pub mod opts;

pub struct CpuFractalGeneratorFactory {
    thread_count: usize,
}

impl CpuFractalGeneratorFactory {
    pub fn new(thread_count: usize) -> CpuFractalGeneratorFactory {
        CpuFractalGeneratorFactory { thread_count }
    }
}

impl FractalGeneratorFactory for CpuFractalGeneratorFactory {
    fn create_generator(
        &self,
        opts: FractalOpts,
    ) -> BoxFuture<'static, anyhow::Result<Box<dyn FractalGenerator + Send + 'static>>> {
        ready(match CpuFractalGenerator::new(opts, self.thread_count) {
            Ok(gen) => {
                let boxed: Box<dyn FractalGenerator + Send> = Box::new(gen);
                Ok(boxed)
            },
            Err(e) => Err(e.into()),
        })
        .boxed()
    }
}

pub struct CpuFractalGenerator {
    opts: FractalOpts,
    thread_pool: Arc<ThreadPool>,
    thread_count: usize,
}

impl CpuFractalGenerator {
    pub fn new(opts: FractalOpts, thread_count: usize) -> Result<CpuFractalGenerator, CpuGenError> {
        Ok(CpuFractalGenerator {
            opts,
            thread_pool: Arc::new(ThreadPoolBuilder::new().num_threads(thread_count).build()?),
            thread_count,
        })
    }
}

impl FractalGenerator for CpuFractalGenerator {
    fn min_views_hint(&self) -> BoxFuture<'static, anyhow::Result<usize>> {
        ready(Ok(self.thread_count)).boxed()
    }

    fn start_generation_to_cpu(
        &self,
        views: &[View],
        sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> BoxFuture<'static, anyhow::Result<Box<dyn FractalGeneratorInstance + Send + 'static>>>
    {
        let thread_pool = self.thread_pool.clone();
        let views = views.to_vec();
        let opts = self.opts.clone();
        async move {
            let boxed: Box<dyn FractalGeneratorInstance + Send> = Box::new(
                CpuFractalGeneratorInstance::start(thread_pool, views, sender, opts).await,
            );
            Ok(boxed)
        }
        .boxed()
    }

    fn start_generation_to_gpu(
        &self,
        views: &[View],
        _device: Arc<Device>,
        queue: Arc<Queue>,
        texture: Arc<Texture>,
        _texture_view: Arc<TextureView>,
    ) -> BoxFuture<'static, anyhow::Result<Box<dyn FractalGeneratorInstance + Send + 'static>>>
    {
        let thread_pool = self.thread_pool.clone();
        let views = views.to_vec();
        let opts = self.opts.clone();
        async move {
            let sink = GpuPixelBlockSink { queue, texture };
            let boxed: Box<dyn FractalGeneratorInstance + Send> =
                Box::new(CpuFractalGeneratorInstance::start(thread_pool, views, sink, opts).await);
            Ok(boxed)
        }
        .boxed()
    }
}

struct CpuFractalGeneratorInstance {
    view_count: usize,
    completed: Arc<AtomicUsize>,
}

impl CpuFractalGeneratorInstance {
    async fn start<S: PixelBlockSink + Send + Sync + 'static>(
        thread_pool: Arc<ThreadPool>,
        views: Vec<View>,
        sink: S,
        opts: FractalOpts,
    ) -> CpuFractalGeneratorInstance {
        info!("Starting new CPU fractal generator...");
        let view_count = views.len();
        let completed = Arc::new(AtomicUsize::new(0));
        let async_completed = completed.clone();

        let sample_count = opts.multisampling.sample_count();
        let sample_count_f32 = sample_count as f32;
        let sample_count = sample_count as usize;
        let offsets = Arc::new(opts.multisampling.offsets());

        tokio::spawn(async move {
            for view in views {
                let spawn_offsets = offsets.clone();
                let spawn_completed = async_completed.clone();
                let spawn_tx: S::Reserved = sink.reserve().await.unwrap();
                thread_pool.spawn(move || {
                    let mut image =
                        vec![0u8; view.image_width * view.image_height * BYTES_PER_PIXEL];

                    for y in 0..view.image_height {
                        for x in 0..view.image_width {
                            let index = (x + y * view.image_width) * BYTES_PER_PIXEL;

                            let mut color = Vector4 {
                                x: 0.0,
                                y: 0.0,
                                z: 0.0,
                                w: 0.0,
                            };

                            for i in 0..sample_count {
                                let offset = spawn_offsets[i];
                                color +=
                                    opts.gen_pixel(view, x as f32 + offset.x, y as f32 + offset.y)
                                        / sample_count_f32;
                            }

                            let color: RGBA8Color = color.into();
                            let color: [u8; 4] = color.into();

                            image[index..index + BYTES_PER_PIXEL].copy_from_slice(&color);
                        }
                    }

                    info!("Generated chunk at ({}, {})", view.image_x, view.image_y);
                    spawn_completed.fetch_add(1, Ordering::AcqRel);

                    spawn_tx.accept(PixelBlock {
                        view,
                        image: image.into_boxed_slice(),
                    });
                });
            }
        });

        info!("Threads started.");

        CpuFractalGeneratorInstance {
            view_count,
            completed,
        }
    }
}

impl FractalGeneratorInstance for CpuFractalGeneratorInstance {
    fn progress(&self) -> BoxFuture<'static, anyhow::Result<f32>> {
        ready(Ok(
            self.completed.load(Ordering::Acquire) as f32 / self.view_count as f32
        ))
        .boxed()
    }

    fn running(&self) -> BoxFuture<'static, anyhow::Result<bool>> {
        ready(Ok(self.completed.load(Ordering::Acquire) < self.view_count)).boxed()
    }
}

#[derive(Error, Debug)]
pub enum CpuGenError {
    #[error("Error building thread pool")]
    ThreadPoolBuildError(#[from] rayon::ThreadPoolBuildError),
}

trait PixelBlockSink {
    type Reserved: ReservedPixelBlockSink + Send;
    type Error: std::fmt::Debug;
    type Future: std::future::Future<Output = Result<Self::Reserved, Self::Error>> + Send;

    fn reserve(&self) -> Self::Future;
}

trait ReservedPixelBlockSink {
    fn accept(self, pixel_block: PixelBlock);
}

impl PixelBlockSink for Sender<anyhow::Result<PixelBlock>> {
    type Reserved = OwnedPermit<anyhow::Result<PixelBlock>>;
    type Error = tokio::sync::mpsc::error::SendError<()>;
    type Future = BoxFuture<'static, Result<Self::Reserved, Self::Error>>;

    fn reserve(&self) -> Self::Future {
        self.clone().reserve_owned().boxed()
    }
}

impl ReservedPixelBlockSink for OwnedPermit<anyhow::Result<PixelBlock>> {
    fn accept(self, pixel_block: PixelBlock) {
        self.send(Ok(pixel_block));
    }
}

#[derive(Clone)]
struct GpuPixelBlockSink {
    queue: Arc<Queue>,
    texture: Arc<Texture>,
}

impl PixelBlockSink for GpuPixelBlockSink {
    type Reserved = GpuPixelBlockSink;
    type Error = ();
    type Future = Ready<Result<GpuPixelBlockSink, ()>>;

    fn reserve(&self) -> Self::Future {
        ready(Ok(self.clone()))
    }
}

impl ReservedPixelBlockSink for GpuPixelBlockSink {
    fn accept(self, pixel_block: PixelBlock) {
        self.queue.write_texture(
            ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: pixel_block.view.image_x as u32,
                    y: pixel_block.view.image_y as u32,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            &pixel_block.image,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(
                    NonZeroU32::new((pixel_block.view.image_width * BYTES_PER_PIXEL) as u32)
                        .unwrap(),
                ),
                rows_per_image: None,
            },
            Extent3d {
                width: pixel_block.view.image_width as u32,
                height: pixel_block.view.image_height as u32,
                depth_or_array_layers: 1,
            },
        );
    }
}
