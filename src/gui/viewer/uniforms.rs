use bytemuck::{Pod, Zeroable};
use cgmath::Matrix4;

#[derive(Copy, Clone, Debug)]
pub struct Uniforms {
    pub screen: Matrix4<f32>,
    pub model: Matrix4<f32>,
}

unsafe impl Zeroable for Uniforms {}
unsafe impl Pod for Uniforms {}
