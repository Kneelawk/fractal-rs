//! GeneratorTarget trait and default impl.

use crate::PixelBlock;
use fractal_rs_3_utils::anymap::{AnyMap, CloneAnySync};
use futures::FutureExt;
use futures_core::future::BoxFuture;
use std::fmt::{Debug, Display, Formatter};
use tokio::sync::mpsc;

/// A set of generator targets.
///
/// Only a [`CpuGeneratorTarget`] is guaranteed to be present.
#[derive(Debug, Clone)]
pub struct GeneratorTargetSet {
    // extensions: anymap::Map<dyn anymap::CloneAny + Send + Sync>,
    // extensions: HashMap<TypeId, Box<dyn GeneratorTarget>>,
    extensions: AnyMap,
}

impl GeneratorTargetSet {
    /// Creates a new generator set with just cpu target.
    pub fn new(cpu: CpuGeneratorTarget) -> Self {
        let mut extensions: AnyMap = AnyMap::new();
        extensions.insert(cpu);
        Self { extensions }
    }

    /// Inserts an extension.
    pub fn insert_extension<T: CloneAnySync>(&mut self, extension: T) -> Option<T> {
        self.extensions.insert(extension)
    }

    /// Gets an extension.
    pub fn get_extension<T: CloneAnySync>(&self) -> Option<&T> {
        self.extensions.get()
    }
}

/// A generator target that can receive CPU-bound pixel blocks.
#[derive(Debug, Clone)]
pub struct CpuGeneratorTarget {
    pub sender: mpsc::Sender<anyhow::Result<PixelBlock>>,
}

impl CpuGeneratorTarget {
    /// Accepts a pixel block and sends it to the underlying implementation.
    pub fn accept(
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
