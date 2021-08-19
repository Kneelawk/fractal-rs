use crate::generator::view::View;
use bytemuck::{Pod, Zeroable};
use cgmath::Vector2;

#[derive(Copy, Clone, Debug)]
pub struct Uniforms {
    pub view: GpuView,
}

unsafe impl Zeroable for Uniforms {}
unsafe impl Pod for Uniforms {}

#[derive(Copy, Clone, Debug)]
pub struct GpuView {
    pub image_size: Vector2<f32>,
    pub image_scale: Vector2<f32>,
    pub plane_start: Vector2<f32>,
}

impl GpuView {
    pub fn from_view(view: View) -> GpuView {
        GpuView {
            image_size: Vector2 {
                x: view.image_width as f32,
                y: view.image_height as f32,
            },
            image_scale: Vector2 {
                x: view.image_scale_x,
                y: view.image_scale_y,
            },
            plane_start: Vector2 {
                x: view.plane_start_x,
                y: view.plane_start_y,
            },
        }
    }
}

impl From<View> for GpuView {
    fn from(view: View) -> Self {
        GpuView::from_view(view)
    }
}
