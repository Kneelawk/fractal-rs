//! This module contains the [`GeneratorManager`] and everything associated with
//! it.
#![allow(dead_code)]

use crate::{
    generator::{
        row_stitcher::RowStitcher, view::View, FractalGenerator, FractalGeneratorFactory,
        FractalGeneratorInstance, FractalOpts, PixelBlock,
    },
    gpu::GPUContext,
    util::future::{future_wrapper::FutureWrapper, poll_join_result, poll_optional, RunningState},
};
use mtpng::{encoder, ColorType, Header};
use std::{
    fmt::Debug,
    fs::File,
    io::BufWriter,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    task::Poll,
};
use tokio::{
    runtime::Handle,
    sync::{mpsc, mpsc::Receiver},
    task::{JoinError, JoinHandle},
};
use wgpu::{Texture, TextureView};

const MAX_CHUNK_BACKLOG: usize = 32;

/// Handles the gritty details of polling generator & instance futures.
pub struct GeneratorManager {
    // runtime handle
    handle: Handle,

    // general state stuff
    cancel: Arc<AtomicBool>,

    // stuff for use when creating new generators and managing generators
    factory: Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
    generator_future: Option<(
        StartArgs,
        JoinHandle<anyhow::Result<Box<dyn FractalGenerator + Send + 'static>>>,
    )>,
    current_generator: Option<(FractalOpts, Box<dyn FractalGenerator + Send + 'static>)>,

    // stuff for managing a running instance
    current_instance: RunningState<
        Box<dyn FractalGeneratorInstance + Send + 'static>,
        JoinHandle<anyhow::Result<Box<dyn FractalGeneratorInstance + Send + 'static>>>,
    >,
    progress_future: Option<JoinHandle<anyhow::Result<f32>>>,
    running_future: Option<JoinHandle<anyhow::Result<bool>>>,
    progress: f32,
    instance_canceled: bool,

    // image writer stuff
    current_image_writer: FutureWrapper<JoinHandle<Result<(), WriteError>>>,
    image_max_y: usize,
    image_writer_progress: Arc<AtomicUsize>,
}

impl GeneratorManager {
    /// Creates a new InstanceManager without any managed instance.
    pub fn new(
        handle: Handle,
        factory: Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
    ) -> GeneratorManager {
        GeneratorManager {
            handle,
            factory,
            generator_future: None,
            current_generator: None,
            current_instance: RunningState::NotStarted,
            progress_future: None,
            running_future: None,
            progress: 0.0,
            cancel: Arc::new(AtomicBool::new(false)),
            instance_canceled: false,
            current_image_writer: Default::default(),
            image_max_y: 0,
            image_writer_progress: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Checks to see if this InstanceManager is already running an instance.
    pub fn running(&self) -> bool {
        self.current_instance.is_started()
            || self.generator_future.is_some()
            || self.current_image_writer.contains_future()
    }

    /// Gets this InstanceManager's FractalGeneratorInstance's current
    /// generation progress.
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Gets this manager's writer's current writing progress.
    pub fn writer_progress(&self) -> f32 {
        if self.image_max_y > 0 {
            self.image_writer_progress.load(Ordering::Acquire) as f32 / self.image_max_y as f32
        } else {
            0.0
        }
    }

    /// Sets this `GeneratorManager`'s [`FractalGeneratorFactory`].
    ///
    /// [`FractalGeneratorFactory`]: crate::generator::FractalGeneratorFactory
    pub fn set_factory(
        &mut self,
        factory: Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
    ) {
        self.factory = factory;
        self.current_generator = None;
    }

    /// Cancels any running fractal generator associated with this manager.
    pub fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }

    /// Starts this `InstanceManager` managing an instance if it is not already
    /// doing so. This `start` variant starts the generator generating to a PNG
    /// in the filesystem, calling [`start_generation_to_cpu`].
    ///
    /// First this `InstanceManager` checks to make sure it has a
    /// [`FractalGenerator`] with the correct [`FractalOpts`], creating a new
    /// one if needed.
    ///
    /// [`FractalGenerator`]: crate::generator::FractalGenerator
    /// [`FractalOpts`]: crate::generator::FractalOpts
    /// [`start_generation_to_cpu`]:
    ///   crate::generator::FractalGenerator::start_generation_to_cpu
    pub fn start_to_image(
        &mut self,
        opts: FractalOpts,
        parent_view: View,
        child_views: Vec<View>,
        output: PathBuf,
    ) -> Result<(), ImageStartError> {
        // make sure we're not currently running
        if self.running() {
            return Err(ImageStartError::AlreadyRunning { opts });
        }

        // make sure the path isn't empty at least
        if output.as_os_str().is_empty() {
            return Err(ImageStartError::PathIsEmpty);
        }

        self.cancel.store(false, Ordering::Release);
        self.instance_canceled = false;

        // check to see if we need to create a new generator
        if self.current_generator.is_none() || self.current_generator.as_ref().unwrap().0 != opts {
            // we need to create a new generator
            self.start_with_new_generator(StartArgs::CPU {
                opts,
                parent_view,
                child_views,
                output,
            });
        } else {
            self.image_max_y = parent_view.image_height;
            self.image_writer_progress.store(0, Ordering::Release);

            let (sender, receiver) = mpsc::channel(MAX_CHUNK_BACKLOG);

            // we can start the generator now
            self.current_instance = RunningState::Starting(
                self.handle.spawn(
                    self.current_generator
                        .as_ref()
                        .unwrap()
                        .1
                        .start_generation_to_cpu(&child_views, sender),
                ),
            );

            // start the image writer too
            self.current_image_writer
                .insert_spawn(
                    &self.handle,
                    write_to_image(
                        self.cancel.clone(),
                        self.image_writer_progress.clone(),
                        receiver,
                        parent_view,
                        child_views,
                        output,
                    ),
                )
                .unwrap();
        }

        Ok(())
    }

    /// Starts this `InstanceManager` managing an instance if it is not already
    /// doing so. This `start` variant starts the generator generating to the
    /// GPU, calling [`start_generation_to_gpu`].
    ///
    /// First this `InstanceManager` checks to make sure it has a
    /// [`FractalGenerator`] with the correct [`FractalOpts`], creating a new
    /// one if needed.
    ///
    /// [`FractalGenerator`]: crate::generator::FractalGenerator
    /// [`FractalOpts`]: crate::generator::FractalOpts
    /// [`start_generation_to_gpu`]:
    ///   crate::generator::FractalGenerator::start_generation_to_gpu()
    pub fn start_to_gui(
        &mut self,
        opts: FractalOpts,
        views: Vec<View>,
        present: GPUContext,
        texture: Arc<Texture>,
        texture_view: Arc<TextureView>,
    ) -> Result<(), ViewerStartError> {
        // make sure we're not currently running
        if self.running() {
            return Err(ViewerStartError::AlreadyRunning { opts });
        }

        self.cancel.store(false, Ordering::Release);
        self.instance_canceled = false;

        // check to see if we need to create a new generator
        if self.current_generator.is_none() || self.current_generator.as_ref().unwrap().0 != opts {
            // we need to create a new generator
            self.start_with_new_generator(StartArgs::GPU {
                opts,
                views,
                present,
                texture,
                texture_view,
            });
        } else {
            // we can start the generator now
            self.current_instance = RunningState::Starting(
                self.handle.spawn(
                    self.current_generator
                        .as_ref()
                        .unwrap()
                        .1
                        .start_generation_to_gpu(&views, present, texture, texture_view),
                ),
            );
        }

        Ok(())
    }

    fn start_with_new_generator(&mut self, args: StartArgs) {
        let opts = match &args {
            StartArgs::CPU { opts, .. } => opts.clone(),
            StartArgs::GPU { opts, .. } => opts.clone(),
        };

        info!("Creating new Fractal Generator...");
        self.generator_future =
            Some((args, self.handle.spawn(self.factory.create_generator(opts))));
    }

    /// Polls the instance and futures currently being managed by this
    /// InstanceManager.
    pub fn poll(&mut self) -> Result<(), PollError> {
        if let Some((args, mut future)) = self.generator_future.take() {
            if let Some(future_res) = poll_join_result(&self.handle, &mut future) {
                let generator = future_res?;

                // We're starting here because if `generator_future` is
                // ever Some(...), it's safe to assume that we're
                // starting a fractal generator.
                let opts = match args {
                    StartArgs::CPU {
                        opts,
                        parent_view,
                        child_views,
                        output,
                    } => {
                        if !self.cancel.load(Ordering::Acquire) {
                            self.image_max_y = parent_view.image_height;
                            self.image_writer_progress.store(0, Ordering::Release);

                            let (sender, receiver) = mpsc::channel(MAX_CHUNK_BACKLOG);

                            self.current_instance = RunningState::Starting(
                                self.handle
                                    .spawn(generator.start_generation_to_cpu(&child_views, sender)),
                            );

                            self.current_image_writer
                                .insert_spawn(
                                    &self.handle,
                                    write_to_image(
                                        self.cancel.clone(),
                                        self.image_writer_progress.clone(),
                                        receiver,
                                        parent_view,
                                        child_views,
                                        output,
                                    ),
                                )
                                .unwrap();
                        }

                        opts
                    },
                    StartArgs::GPU {
                        opts,
                        views,
                        present,
                        texture,
                        texture_view,
                    } => {
                        if !self.cancel.load(Ordering::Acquire) {
                            self.current_instance = RunningState::Starting(self.handle.spawn(
                                generator.start_generation_to_gpu(
                                    &views,
                                    present,
                                    texture,
                                    texture_view,
                                ),
                            ));
                        }

                        opts
                    },
                };

                self.current_generator = Some((opts, generator));
            } else {
                // put the args and the future back in the option
                self.generator_future = Some((args, future));
            }
        }

        // poll the RunningState of the instance
        self.current_instance.poll_starting(&self.handle)?;

        // reset values
        if let RunningState::Starting(_) = &self.current_instance {
            self.progress = 0.0;
        }

        // check if we're canceled
        if let RunningState::Running(instance) = &self.current_instance {
            if self.cancel.load(Ordering::Acquire) && !self.instance_canceled {
                instance.cancel();

                // Set instance_canceled to make sure we don't sent the cancel signal twice
                self.instance_canceled = true;
            }
        }

        // poll the running future optional
        let running = poll_optional(&self.handle, &mut self.running_future, || {
            if let RunningState::Running(instance) = &self.current_instance {
                Some(self.handle.spawn(instance.running()))
            } else {
                None
            }
        });

        // poll the progress future optional
        let progress = poll_optional(&self.handle, &mut self.progress_future, || {
            if let RunningState::Running(instance) = &self.current_instance {
                Some(self.handle.spawn(instance.progress()))
            } else {
                None
            }
        });

        // apply running value
        if let Some(running) = running {
            let running = running?;
            if !running {
                self.current_instance = RunningState::NotStarted;
            }
        }

        // apply progress value
        if let Some(progress) = progress {
            self.progress = progress?;
        }

        // poll image writer join handle
        if let Some(writer_res) = self.current_image_writer.poll_join_result(&self.handle) {
            writer_res?;
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ViewerStartError {
    #[error("instance manager already running an instance")]
    AlreadyRunning { opts: FractalOpts },
}

#[derive(Debug, Error)]
pub enum ImageStartError {
    #[error("instance manager already running an instance")]
    AlreadyRunning { opts: FractalOpts },
    #[error("output file path is empty")]
    PathIsEmpty,
}

#[derive(Debug, Error)]
pub enum PollError {
    #[error(transparent)]
    WriteError(#[from] WriteError),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("IO error while writing image to file")]
    IOError(#[from] std::io::Error),
    #[error("image write was canceled")]
    Canceled,
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error("JoinError while writing image to file")]
    JoinError(#[from] JoinError),
}

async fn write_to_image(
    canceled: Arc<AtomicBool>,
    progress: Arc<AtomicUsize>,
    mut receiver: Receiver<anyhow::Result<PixelBlock>>,
    parent_view: View,
    child_views: Vec<View>,
    output: PathBuf,
) -> Result<(), WriteError> {
    if canceled.load(Ordering::Acquire) {
        return Err(WriteError::Canceled);
    }

    info!("Creating output file...");
    let stream_writer: Result<_, WriteError> = tokio::task::spawn_blocking(move || {
        // we have to do blocking file operations because MTPNG doesn't like
        // non-blocking ones
        let output_file = File::create(output)?;
        let file_writer = BufWriter::new(output_file);
        let options = encoder::Options::new();
        let mut encoder = encoder::Encoder::new(file_writer, &options);
        let mut header = Header::new();
        header.set_size(
            parent_view.image_width as u32,
            parent_view.image_height as u32,
        )?;
        header.set_color(ColorType::TruecolorAlpha, 8)?;
        encoder.write_header(&header)?;
        Ok(encoder)
    })
    .await
    .expect("Something panicked while opening the output image file");

    let mut stream_writer = Some(stream_writer?);

    let mut row_stitcher = RowStitcher::new(parent_view, &child_views);

    info!("Starting image writer loop...");
    // while let Some(block) = receiver.recv().await
    loop {
        tokio::select! {
            biased;
            poll_block = receiver.recv() => {
                if canceled.load(Ordering::Acquire) {
                    // This will probably return an error since image writing could be incomplete.
                    // We'll just ignore it
                    tokio::task::spawn_blocking(move || stream_writer.unwrap().flush())
                        .await
                        .expect("Something panicked while flushing the encoder")
                        .ok();
                    return Err(WriteError::Canceled);
                }

                let block: anyhow::Result<PixelBlock> = if let Some(block) = poll_block {
                    block
                } else {
                    break;
                };

                // we're using a match here because mtpng does not handle being dropped in the
                // middle of encoding very well, so we need to shut it down first
                let block = match block {
                    Ok(b) => b,
                    Err(e) => {
                        // This will probably return an error since image writing could be incomplete.
                        // We'll just ignore it
                        tokio::task::spawn_blocking(move || stream_writer.unwrap().flush())
                            .await
                            .expect("Something panicked while flushing the encoder")
                            .ok();
                        return Err(e.into());
                    }
                };

                info!(
                    "Received block at ({}, {})",
                    block.view.image_x, block.view.image_y
                );
                row_stitcher.insert(block);

                while let Poll::Ready(Some(row)) = row_stitcher.stitch() {
                    let image_y = row.view.image_y;
                    let image_height = row.view.image_height;
                    let mut moved_writer = stream_writer.take().unwrap();
                    info!("Writing row at y={}", image_y);

                    let moved_writer: Result<_, WriteError> = tokio::task::spawn_blocking(move || {
                        moved_writer.write_image_rows(&row.image)?;
                        Ok(moved_writer)
                    })
                    .await
                    .expect("Something panicked while writing a row of the output PNG");

                    // we're using a match here because mtpng does not handle being dropped in the
                    // middle of encoding very well, so we need to shut it down first
                    stream_writer = Some(match moved_writer {
                        Ok(b) => b,
                        Err(e) => {
                            // This will probably return an error since image writing could be incomplete.
                            // We'll just ignore it
                            tokio::task::spawn_blocking(move || stream_writer.unwrap().flush())
                                .await
                                .expect("Something panicked while flushing the encoder")
                                .ok();
                            return Err(e.into());
                        }
                    });

                    progress.store(image_y + image_height, Ordering::Release);
                }
            },
            else => {
                if canceled.load(Ordering::Acquire) {
                    // This will probably return an error since image writing could be incomplete.
                    // We'll just ignore it
                    tokio::task::spawn_blocking(move || stream_writer.unwrap().flush())
                        .await
                        .expect("Something panicked while flushing the encoder")
                        .ok();
                    return Err(WriteError::Canceled);
                }
            }
        }
    }

    if canceled.load(Ordering::Acquire) {
        // This will probably return an error since image writing could be incomplete.
        // We'll just ignore it
        tokio::task::spawn_blocking(move || stream_writer.unwrap().flush())
            .await
            .expect("Something panicked while flushing the encoder")
            .ok();
        return Err(WriteError::Canceled);
    }

    info!("Finishing output file...");
    tokio::task::spawn_blocking(move || stream_writer.unwrap().finish())
        .await
        .expect("Something panicked while finishing writing the image")?;

    info!("Finished writing PNG");

    Ok(())
}

enum StartArgs {
    CPU {
        opts: FractalOpts,
        parent_view: View,
        child_views: Vec<View>,
        output: PathBuf,
    },
    GPU {
        opts: FractalOpts,
        views: Vec<View>,
        present: GPUContext,
        texture: Arc<Texture>,
        texture_view: Arc<TextureView>,
    },
}
