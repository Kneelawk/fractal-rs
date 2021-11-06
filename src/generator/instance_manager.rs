use crate::{
    generator::FractalGeneratorInstance,
    util::{poll_optional, RunningState},
};
use futures::future::BoxFuture;
use std::fmt::{Debug, Formatter};
use tokio::task::JoinHandle;

/// Handles the gritty details of polling instance futures.
pub struct InstanceManager {
    current_instance: RunningState<
        Box<dyn FractalGeneratorInstance + Send + 'static>,
        JoinHandle<anyhow::Result<Box<dyn FractalGeneratorInstance + Send + 'static>>>,
    >,
    progress_future: Option<JoinHandle<anyhow::Result<f32>>>,
    running_future: Option<JoinHandle<anyhow::Result<bool>>>,
    progress: f32,
}

impl InstanceManager {
    /// Creates a new InstanceManager without any managed instance.
    pub fn new() -> InstanceManager {
        InstanceManager {
            current_instance: RunningState::NotStarted,
            progress_future: None,
            running_future: None,
            progress: 0.0,
        }
    }

    /// Checks to see if this InstanceManager is already running an instance.
    pub fn running(&self) -> bool {
        self.current_instance.is_started()
    }

    /// Gets this InstanceManager's FractalGeneratorInstance's current
    /// generation progress.
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Starts this InstanceManager managing an instance if it is not already
    /// doing so.
    pub fn start(
        &mut self,
        instance_future: BoxFuture<
            'static,
            anyhow::Result<Box<dyn FractalGeneratorInstance + Send + 'static>>,
        >,
    ) -> Result<(), StartError> {
        if self.current_instance.is_started() {
            return Err(StartError::AlreadyRunning {
                attempted_start: instance_future,
            });
        }

        self.current_instance = RunningState::Starting(tokio::spawn(instance_future));

        Ok(())
    }

    /// Polls the instance and futures currently being managed by this
    /// InstanceManager.
    pub fn poll(&mut self) -> anyhow::Result<()> {
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

#[derive(Error)]
pub enum StartError {
    #[error("instance manager already running an instance")]
    AlreadyRunning {
        attempted_start:
            BoxFuture<'static, anyhow::Result<Box<dyn FractalGeneratorInstance + Send + 'static>>>,
    },
}

impl Debug for StartError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StartError::AlreadyRunning { .. } => f.write_str("StartError::AlreadyRunning"),
        }
    }
}
