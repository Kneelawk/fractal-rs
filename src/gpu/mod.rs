use std::sync::Arc;
use wgpu::{Device, Queue};

pub mod buffer;
pub mod util;

/// Describes a Device-Queue set. This can either be a set that can be used to
/// present or a set that can only be used for generation.
///
/// # Cloning
/// This structure is a wrapper around `Arc`s and is designed to be sent to
/// multiple threads via its `clone()` method.
#[derive(Clone)]
pub struct GPUContext {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub ty: GPUContextType,
}

/// Describes whether a Device-Queue set is one that can be used to present or
/// one that can only be used for generation.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum GPUContextType {
    Presentable,
    Dedicated,
}
