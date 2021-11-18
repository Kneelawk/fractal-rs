use crate::{
    generator::{
        color::RGBA8Color, cpu::opts::CpuFractalOpts, view::View, FractalGenerator,
        FractalGeneratorFactory, FractalGeneratorInstance, FractalOpts, PixelBlock,
        BYTES_PER_PIXEL,
    },
    util::result::ResultExt,
};
use cgmath::Vector4;
use chrono::{DateTime, Utc};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use futures::{
    future::{ready, BoxFuture},
    FutureExt,
};
use rayon::{
    iter::{IndexedParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
    ThreadPool, ThreadPoolBuilder,
};
use std::{
    num::NonZeroU32,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::{sync::mpsc::Sender, task};
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
    canceled: Arc<AtomicBool>,
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
        let canceled = Arc::new(AtomicBool::new(false));
        let async_completed = completed.clone();
        let async_canceled = canceled.clone();

        let sample_count = opts.multisampling.sample_count();
        let sample_count_f32 = sample_count as f32;
        let sample_count = sample_count as usize;
        let offsets = Arc::new(opts.multisampling.offsets());

        tokio::spawn(async move {
            let start_time = Utc::now();

            for view in views {
                if async_canceled.load(Ordering::Acquire) {
                    info!("Received cancel signal.");
                    return;
                }

                let spawn_thread_pool = thread_pool.clone();
                let spawn_offsets = offsets.clone();
                let spawn_completed = async_completed.clone();
                let spawn_canceled = async_canceled.clone();

                let res = task::spawn_blocking(move || {
                    let mut image =
                        vec![0u8; view.image_width * view.image_height * BYTES_PER_PIXEL];

                    let res = spawn_thread_pool.install(|| {
                        image
                            .par_chunks_exact_mut(BYTES_PER_PIXEL)
                            .enumerate()
                            .try_for_each(|(index, pixel)| {
                                let x = index % view.image_width;
                                let y = index / view.image_width;

                                if spawn_canceled.load(Ordering::Acquire) {
                                    info!("Received cancel signal.");
                                    return Err(());
                                }

                                let mut color = Vector4 {
                                    x: 0.0,
                                    y: 0.0,
                                    z: 0.0,
                                    w: 0.0,
                                };

                                for i in 0..sample_count {
                                    let offset = spawn_offsets[i];
                                    color += opts.gen_pixel(
                                        view,
                                        x as f32 + offset.x,
                                        y as f32 + offset.y,
                                    ) / sample_count_f32;
                                }

                                let color: RGBA8Color = color.into();
                                let color: [u8; 4] = color.into();

                                pixel.copy_from_slice(&color);

                                Ok(())
                            })
                    });

                    if res.is_err() {
                        return None;
                    }

                    info!("Generated chunk at ({}, {})", view.image_x, view.image_y);
                    let completed = spawn_completed.fetch_add(1, Ordering::AcqRel) + 1;

                    if completed == view_count {
                        display_duration(start_time);
                    }

                    Some(image.into_boxed_slice())
                })
                .await
                .on_err(|e| warn!("JoinError in CPU generator: {:?}", e))
                .flatten();

                if let Some(image) = res {
                    sink.accept(PixelBlock { view, image }).await.on_err(|e| {
                        warn!(
                            "Error while submitting pixel block in CPU generator: {:?}",
                            e
                        )
                    });
                }
            }
        });

        info!("Threads started.");

        CpuFractalGeneratorInstance {
            view_count,
            completed,
            canceled,
        }
    }
}

fn display_duration(start_time: DateTime<Utc>) {
    let end_time = Utc::now();
    let duration = end_time - start_time;

    info!(
        "Completed in: {}",
        HumanTime::from(duration).to_text_en(Accuracy::Precise, Tense::Present)
    );
}

impl FractalGeneratorInstance for CpuFractalGeneratorInstance {
    fn cancel(&self) {
        self.canceled.store(true, Ordering::Release);
    }

    fn progress(&self) -> BoxFuture<'static, anyhow::Result<f32>> {
        ready(Ok(
            self.completed.load(Ordering::Acquire) as f32 / self.view_count as f32
        ))
        .boxed()
    }

    fn running(&self) -> BoxFuture<'static, anyhow::Result<bool>> {
        ready(Ok(self.completed.load(Ordering::Acquire) < self.view_count
            && !self.canceled.load(Ordering::Acquire)))
        .boxed()
    }
}

#[derive(Error, Debug)]
pub enum CpuGenError {
    #[error("Error building thread pool")]
    ThreadPoolBuildError(#[from] rayon::ThreadPoolBuildError),
}

trait PixelBlockSink {
    type Error: std::fmt::Debug;

    fn accept(&self, pixel_block: PixelBlock) -> BoxFuture<'_, Result<(), Self::Error>>;
}

impl PixelBlockSink for Sender<anyhow::Result<PixelBlock>> {
    type Error = tokio::sync::mpsc::error::SendError<anyhow::Result<PixelBlock>>;

    fn accept(&self, pixel_block: PixelBlock) -> BoxFuture<'_, Result<(), Self::Error>> {
        self.send(Ok(pixel_block)).boxed()
    }
}

#[derive(Clone)]
struct GpuPixelBlockSink {
    queue: Arc<Queue>,
    texture: Arc<Texture>,
}

impl PixelBlockSink for GpuPixelBlockSink {
    type Error = ();

    fn accept(&self, pixel_block: PixelBlock) -> BoxFuture<'_, Result<(), Self::Error>> {
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

        ready(Ok(())).boxed()
    }
}
