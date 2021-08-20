use crate::generator::{
    color::RGBA8Color, cpu::opts::CpuFractalOpts, view::View, FractalGenerator,
    FractalGeneratorInstance, FractalOpts, PixelBlock, BYTES_PER_PIXEL,
};
use cgmath::Vector4;
use futures::{
    executor::block_on,
    future::{ready, BoxFuture},
    FutureExt,
};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::sync::Arc;
use tokio::sync::{mpsc::Sender, RwLock};

pub mod opts;

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
    fn min_views_hint(&self) -> BoxFuture<anyhow::Result<usize>> {
        ready(Ok(self.thread_count)).boxed()
    }

    fn start_generation(
        &self,
        views: &[View],
        sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> BoxFuture<Result<Box<dyn FractalGeneratorInstance>, anyhow::Error>> {
        let views = views.to_vec();
        async move {
            let boxed: Box<dyn FractalGeneratorInstance> = Box::new(
                CpuFractalGeneratorInstance::start(
                    self.thread_pool.clone(),
                    views,
                    sender,
                    self.opts,
                )
                .await,
            );
            Ok(boxed)
        }
        .boxed()
    }
}

struct CpuFractalGeneratorInstance {
    view_count: usize,
    completed: Arc<RwLock<usize>>,
}

impl CpuFractalGeneratorInstance {
    async fn start(
        thread_pool: Arc<ThreadPool>,
        views: Vec<View>,
        sender: Sender<anyhow::Result<PixelBlock>>,
        opts: FractalOpts,
    ) -> CpuFractalGeneratorInstance {
        info!("Starting new CPU fractal generator...");
        let view_count = views.len();
        let completed = Arc::new(RwLock::new(0));
        let async_completed = completed.clone();

        let sample_count = opts.multisampling.sample_count();
        let sample_count_f32 = sample_count as f32;
        let sample_count = sample_count as usize;
        let offsets = Arc::new(opts.multisampling.offsets());

        tokio::spawn(async move {
            for view in views {
                let spawn_offsets = offsets.clone();
                let spawn_completed = async_completed.clone();
                let spawn_tx = sender.clone().reserve_owned().await.unwrap();
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

                    block_on(async {
                        *spawn_completed.write().await += 1;
                    });

                    spawn_tx.send(Ok(PixelBlock {
                        view,
                        image: image.into_boxed_slice(),
                    }));
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
    fn progress(&self) -> BoxFuture<anyhow::Result<f32>> {
        async move { Ok(*self.completed.read().await as f32 / self.view_count as f32) }.boxed()
    }

    fn running(&self) -> BoxFuture<anyhow::Result<bool>> {
        async move { Ok(*self.completed.read().await < self.view_count) }.boxed()
    }
}

#[derive(Error, Debug)]
pub enum CpuGenError {
    #[error("Error building thread pool")]
    ThreadPoolBuildError(#[from] rayon::ThreadPoolBuildError),
}
