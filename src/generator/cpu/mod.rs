use crate::generator::{
    cpu::opts::CpuFractalOpts, view::View, FractalGenerator, FractalGeneratorInstance, FractalOpts,
    PixelBlock, BYTES_PER_PIXEL,
};
use futures::{
    executor::block_on,
    future::{ready, BoxFuture},
    prelude::stream::BoxStream,
    FutureExt, Stream,
};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{pin::Pin, sync::Arc};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_stream::wrappers::ReceiverStream;

pub mod opts;

const MAX_BACKLOG: usize = 32;

pub struct CpuFractalGenerator {
    opts: FractalOpts,
    thread_pool: Arc<ThreadPool>,
    thread_count: usize,
}

impl CpuFractalGenerator {
    pub fn new(opts: FractalOpts, thread_count: usize) -> CpuGenResult<CpuFractalGenerator> {
        Ok(CpuFractalGenerator {
            opts,
            thread_pool: Arc::new(ThreadPoolBuilder::new().num_threads(thread_count).build()?),
            thread_count,
        })
    }
}

impl FractalGenerator for CpuFractalGenerator {
    fn min_views_hint(&self) -> BoxFuture<'_, usize> {
        ready(self.thread_count).boxed()
    }

    fn start_generation(
        &self,
        views: &[View],
    ) -> BoxFuture<Result<Box<dyn FractalGeneratorInstance>, anyhow::Error>> {
        let views = views.to_vec();
        async move {
            let boxed: Box<dyn FractalGeneratorInstance> = Box::new(
                CpuFractalGeneratorInstance::start(self.thread_pool.clone(), views, self.opts).await,
            );
            Ok(boxed)
        }
        .boxed()
    }
}

struct CpuFractalGeneratorInstance {
    view_count: usize,
    completed: Arc<RwLock<usize>>,
    stream: Arc<Mutex<ReceiverStream<Result<PixelBlock, anyhow::Error>>>>,
}

impl CpuFractalGeneratorInstance {
    async fn start(
        thread_pool: Arc<ThreadPool>,
        views: Vec<View>,
        opts: FractalOpts,
    ) -> CpuFractalGeneratorInstance {
        info!("Starting new CPU fractal generator...");
        let (tx, rx) = mpsc::channel(MAX_BACKLOG);
        let view_count = views.len();
        let completed = Arc::new(RwLock::new(0));
        let async_completed = completed.clone();

        tokio::spawn(async move {
            for view in views {
                let spawn_completed = async_completed.clone();
                let spawn_tx = tx.clone().reserve_owned().await.unwrap();
                thread_pool.spawn(move || {
                    let mut image =
                        vec![0u8; view.image_width * view.image_height * BYTES_PER_PIXEL];

                    for y in 0..view.image_height {
                        for x in 0..view.image_width {
                            let index = x + y * view.image_width;
                            let color: [u8; 4] = opts.gen_pixel(view, x, y).into();
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

        info!("Threads started. Creating stream...");

        let stream = Arc::new(Mutex::new(ReceiverStream::new(rx)));

        CpuFractalGeneratorInstance {
            view_count,
            completed,
            stream,
        }
    }
}

impl FractalGeneratorInstance for CpuFractalGeneratorInstance {
    fn stream(
        &self,
    ) -> Arc<Mutex<dyn Stream<Item = Result<PixelBlock, anyhow::Error>> + Send + Unpin>> {
        self.stream.clone()
    }

    fn running(&self) -> BoxFuture<bool> {
        async move { *self.completed.read().await < self.view_count }.boxed()
    }
}

error_chain! {
    types {
        CpuGenError, CpuGenErrorKind, CpuGenResultExt, CpuGenResult;
    }

    foreign_links {
        ThreadPoolBuild(rayon::ThreadPoolBuildError);
    }
}
