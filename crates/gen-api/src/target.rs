//! GeneratorTarget trait and default impl.

use crate::PixelBlock;
use dyn_clone::DynClone;
use futures::FutureExt;
use futures_core::future::BoxFuture;
use std::fmt::{Debug, Display, Formatter};
use tokio::sync::mpsc;

/// A set of generator targets.
///
/// Only a [`CpuGeneratorTarget`] is guaranteed to be present.
#[derive(Debug, Clone)]
pub struct GeneratorTargetSet {
    extensions: anymap::Map<dyn anymap::any::CloneAny + Send + Sync>,
}

impl GeneratorTargetSet {
    /// Creates a new generator set with just cpu target.
    pub fn new(cpu: CpuGeneratorTarget) -> Self {
        let mut extensions = anymap::Map::new();
        extensions.insert(cpu);
        Self { extensions }
    }

    /// Creates a new generator set from a box of the cpu target.
    pub fn from_boxed(cpu: Box<dyn CpuGeneratorTargetApi>) -> Self {
        Self::new(CpuGeneratorTarget::from_boxed(cpu))
    }

    /// Creates a new generator set from an impl of the cpu target.
    pub fn from_impl(cpu: impl CpuGeneratorTargetApi) -> Self {
        Self::new(CpuGeneratorTarget::new(cpu))
    }

    /// Inserts an extension.
    pub fn insert_extension<T: anymap::any::CloneAny + Send + Sync>(
        &mut self,
        extension: T,
    ) -> Option<T> {
        self.extensions.insert(extension)
    }

    /// Gets an extension.
    pub fn get_extension<T: anymap::any::CloneAny + Send + Sync>(&self) -> Option<&T> {
        self.extensions.get()
    }
}

/// A generator target that can receive CPU-bound pixel blocks.
#[derive(Debug, Clone)]
pub struct CpuGeneratorTarget {
    target: Box<dyn CpuGeneratorTargetApi>,
}

impl CpuGeneratorTarget {
    /// Creates a new CPU-bound generator target from a boxed implementation.
    pub fn from_boxed(target: Box<dyn CpuGeneratorTargetApi>) -> Self {
        Self { target }
    }

    /// Creates a new CPU-bound generator target from an implementation.
    pub fn new(target: impl CpuGeneratorTargetApi) -> Self {
        Self::from_boxed(Box::new(target))
    }

    /// Accepts a pixel block and sends it to the underlying implementation.
    pub fn accept(
        &self,
        pixel_block: anyhow::Result<PixelBlock>,
    ) -> BoxFuture<'static, Result<(), BlockAcceptError>> {
        self.target.accept(pixel_block)
    }
}

/// Something that can have fractal image data written to it.
pub trait CpuGeneratorTargetApi: DynClone + Debug + Send + Sync + 'static {
    /// Accept a pixel block and write it to where ever this target writes its data.
    fn accept(
        &self,
        pixel_block: anyhow::Result<PixelBlock>,
    ) -> BoxFuture<'static, Result<(), BlockAcceptError>>;
}
dyn_clone::clone_trait_object!(CpuGeneratorTargetApi);

/// Simple default implementation of [`CpuGeneratorTargetApi`].
///
/// This does not support any platform-specific extensions, like direct-gpu-writes.
#[derive(Clone)]
pub struct SimpleCpuGeneratorTarget {
    pub sender: mpsc::Sender<anyhow::Result<PixelBlock>>,
}

impl Debug for SimpleCpuGeneratorTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "SimpleCpuGeneratorTarget")
    }
}

impl CpuGeneratorTargetApi for SimpleCpuGeneratorTarget {
    fn accept(
        &self,
        pixel_block: anyhow::Result<PixelBlock>,
    ) -> BoxFuture<'static, Result<(), BlockAcceptError>> {
        let sender = self.sender.clone();
        async move {
            match sender.send(pixel_block).await {
                Ok(_) => Ok(()),
                Err(err) => Err(BlockAcceptError {
                    pixel_block: err.0,
                    error: anyhow::anyhow!("SendError"),
                }),
            }
        }
        .boxed()
    }
}

/// An error potentially produced when attempting to provide a pixel block to a generator target.
pub struct BlockAcceptError {
    /// The pixel block that would have been sent to the generator target.
    pub pixel_block: anyhow::Result<PixelBlock>,
    /// The error the prevented the pixel block from being sent.
    pub error: anyhow::Error,
}

impl Debug for BlockAcceptError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.error, f)
    }
}

impl Display for BlockAcceptError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.error, f)
    }
}

impl From<BlockAcceptError> for anyhow::Error {
    fn from(value: BlockAcceptError) -> Self {
        value.error
    }
}
