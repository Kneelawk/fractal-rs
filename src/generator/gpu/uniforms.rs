use crate::generator::gpu::gpu_view::GPUView;
use bytemuck::{Pod, Zeroable};

#[derive(Copy, Clone, Debug)]
pub struct Uniforms {
    pub view: GPUView,
}

unsafe impl Zeroable for Uniforms {}
unsafe impl Pod for Uniforms {}
