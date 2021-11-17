//! This module contains the [`GeneratorManager`] and everything associated with
//! it.
#![allow(dead_code)]

use crate::{
    generator::{
        view::View, FractalGenerator, FractalGeneratorFactory, FractalGeneratorInstance,
        FractalOpts, PixelBlock,
    },
    util::{poll_join_result, poll_optional, RunningState},
};
use std::{fmt::Debug, sync::Arc};
use tokio::{sync::mpsc::Sender, task::JoinHandle};
use wgpu::{Device, Queue, Texture, TextureView};

/// Handles the gritty details of polling generator & instance futures.
pub struct GeneratorManager {
    factory: Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
    generator_future: Option<(
        StartArgs,
        JoinHandle<anyhow::Result<Box<dyn FractalGenerator + Send + 'static>>>,
    )>,
    current_generator: Option<(FractalOpts, Box<dyn FractalGenerator + Send + 'static>)>,
    current_instance: RunningState<
        Box<dyn FractalGeneratorInstance + Send + 'static>,
        JoinHandle<anyhow::Result<Box<dyn FractalGeneratorInstance + Send + 'static>>>,
    >,
    progress_future: Option<JoinHandle<anyhow::Result<f32>>>,
    running_future: Option<JoinHandle<anyhow::Result<bool>>>,
    progress: f32,
}

impl GeneratorManager {
    /// Creates a new InstanceManager without any managed instance.
    pub fn new(
        factory: Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
    ) -> GeneratorManager {
        GeneratorManager {
            factory,
            generator_future: None,
            current_generator: None,
            current_instance: RunningState::NotStarted,
            progress_future: None,
            running_future: None,
            progress: 0.0,
        }
    }

    /// Checks to see if this InstanceManager is already running an instance.
    pub fn running(&self) -> bool {
        self.current_instance.is_started() || self.generator_future.is_some()
    }

    /// Gets this InstanceManager's FractalGeneratorInstance's current
    /// generation progress.
    pub fn progress(&self) -> f32 {
        self.progress
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

    /// Starts this `InstanceManager` managing an instance if it is not already
    /// doing so. This `start` variant starts the generator generating to the
    /// CPU, calling [`start_generation_to_cpu`].
    ///
    /// First this `InstanceManager` checks to make sure it has a
    /// [`FractalGenerator`] with the correct [`FractalOpts`], creating a new
    /// one if needed.
    ///
    /// [`FractalGenerator`]: crate::generator::FractalGenerator
    /// [`FractalOpts`]: crate::generator::FractalOpts
    /// [`start_generation_to_cpu`]:
    ///   crate::generator::FractalGenerator::start_generation_to_cpu
    pub fn start_to_cpu(
        &mut self,
        opts: FractalOpts,
        views: &[View],
        sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> Result<(), StartError> {
        // make sure we're not currently running
        if self.running() {
            return Err(StartError::AlreadyRunning { opts });
        }

        // check to see if we need to create a new generator
        if self.current_generator.is_none() || self.current_generator.as_ref().unwrap().0 != opts {
            // we need to create a new generator
            self.start_with_new_generator(StartArgs::CPU {
                opts,
                views: views.to_vec(),
                sender,
            });
        } else {
            // we can start the generator now
            self.current_instance = RunningState::Starting(tokio::spawn(
                self.current_generator
                    .as_ref()
                    .unwrap()
                    .1
                    .start_generation_to_cpu(views, sender),
            ));
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
    pub fn start_to_gpu(
        &mut self,
        opts: FractalOpts,
        views: &[View],
        device: Arc<Device>,
        queue: Arc<Queue>,
        texture: Arc<Texture>,
        texture_view: Arc<TextureView>,
    ) -> Result<(), StartError> {
        // make sure we're not currently running
        if self.running() {
            return Err(StartError::AlreadyRunning { opts });
        }

        // check to see if we need to create a new generator
        if self.current_generator.is_none() || self.current_generator.as_ref().unwrap().0 != opts {
            // we need to create a new generator
            self.start_with_new_generator(StartArgs::GPU {
                opts,
                views: views.to_vec(),
                device,
                queue,
                texture,
                texture_view,
            });
        } else {
            // we can start the generator now
            self.current_instance = RunningState::Starting(tokio::spawn(
                self.current_generator
                    .as_ref()
                    .unwrap()
                    .1
                    .start_generation_to_gpu(views, device, queue, texture, texture_view),
            ));
        }

        Ok(())
    }

    fn start_with_new_generator(&mut self, args: StartArgs) {
        let opts = match &args {
            StartArgs::CPU { opts, .. } => opts.clone(),
            StartArgs::GPU { opts, .. } => opts.clone(),
        };

        info!("Creating new Fractal Generator...");
        self.generator_future = Some((args, tokio::spawn(self.factory.create_generator(opts))));
    }

    /// Polls the instance and futures currently being managed by this
    /// InstanceManager.
    pub fn poll(&mut self) -> anyhow::Result<()> {
        if let Some((args, mut future)) = self.generator_future.take() {
            if let Some(future_res) = poll_join_result(&mut future) {
                let generator = future_res?;

                // We're starting here because if `generator_future` is
                // ever Some(...), it's safe to assume that we're
                // starting a fractal generator.
                let opts = match args {
                    StartArgs::CPU {
                        opts,
                        views,
                        sender,
                    } => {
                        self.current_instance = RunningState::Starting(tokio::spawn(
                            generator.start_generation_to_cpu(&views, sender),
                        ));

                        opts
                    },
                    StartArgs::GPU {
                        opts,
                        views,
                        device,
                        queue,
                        texture,
                        texture_view,
                    } => {
                        self.current_instance = RunningState::Starting(tokio::spawn(
                            generator.start_generation_to_gpu(
                                &views,
                                device,
                                queue,
                                texture,
                                texture_view,
                            ),
                        ));

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
        self.current_instance.poll_starting()?;

        // reset values
        if let RunningState::Starting(_) = &self.current_instance {
            self.progress = 0.0;
        }

        // poll the running future optional
        let running = poll_optional(&mut self.running_future, || {
            if let RunningState::Running(instance) = &self.current_instance {
                Some(tokio::spawn(instance.running()))
            } else {
                None
            }
        });

        // poll the progress future optional
        let progress = poll_optional(&mut self.progress_future, || {
            if let RunningState::Running(instance) = &self.current_instance {
                Some(tokio::spawn(instance.progress()))
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

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum StartError {
    #[error("instance manager already running an instance")]
    AlreadyRunning { opts: FractalOpts },
}

enum StartArgs {
    CPU {
        opts: FractalOpts,
        views: Vec<View>,
        sender: Sender<anyhow::Result<PixelBlock>>,
    },
    GPU {
        opts: FractalOpts,
        views: Vec<View>,
        device: Arc<Device>,
        queue: Arc<Queue>,
        texture: Arc<Texture>,
        texture_view: Arc<TextureView>,
    },
}
