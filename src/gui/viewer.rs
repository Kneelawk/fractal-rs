//! viewer.rs - This file holds the systems for the fractal image viewer. This
//! means both image managing and rendering.

use crate::gpu::util::create_texture;
use egui::TextureId;
use egui_wgpu_backend::RenderPass;
use std::sync::Arc;
use wgpu::{Device, FilterMode, Texture, TextureFormat, TextureUsages, TextureView};

pub struct FractalViewer {
    // Static Components
    texture_id: TextureId,

    // Dynamic Components
    fractal_width: u32,
    fractal_height: u32,
    image_texture: Arc<Texture>,
    image_texture_view: Arc<TextureView>,
}

impl FractalViewer {
    pub fn new(
        device: &Device,
        render_pass: &mut RenderPass,
        fractal_width: u32,
        fractal_height: u32,
    ) -> FractalViewer {
        let (image_texture, image_texture_view) = create_texture(
            device,
            fractal_width,
            fractal_height,
            TextureFormat::Rgba8UnormSrgb,
            TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
        );

        let image_texture = Arc::new(image_texture);
        let image_texture_view = Arc::new(image_texture_view);

        let texture_id =
            render_pass.egui_texture_from_wgpu_texture(&device, &image_texture, FilterMode::Linear);

        FractalViewer {
            texture_id,
            fractal_width,
            fractal_height,
            image_texture,
            image_texture_view,
        }
    }

    pub fn get_texture(&self) -> Arc<Texture> {
        self.image_texture.clone()
    }

    pub fn get_texture_view(&self) -> Arc<TextureView> {
        self.image_texture_view.clone()
    }

    pub fn set_fractal_size(
        &mut self,
        device: &Device,
        render_pass: &mut RenderPass,
        width: u32,
        height: u32,
    ) -> Result<(), FractalViewerError> {
        let (image_texture, image_texture_view) = create_texture(
            device,
            width,
            height,
            TextureFormat::Rgba8UnormSrgb,
            TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
        );

        self.image_texture = Arc::new(image_texture);
        self.image_texture_view = Arc::new(image_texture_view);
        self.fractal_width = width;
        self.fractal_height = height;

        render_pass.update_egui_texture_from_wgpu_texture(
            &device,
            &self.image_texture,
            FilterMode::Linear,
            self.texture_id,
        )?;

        Ok(())
    }

    pub fn render(&self, ui: &mut egui::Ui) {
        ui.image(
            self.texture_id,
            [self.fractal_width as f32, self.fractal_height as f32],
        );
    }
}

#[derive(Debug, Error)]
pub enum FractalViewerError {
    #[error("Egui WGPU Backend Error")]
    BackendError(#[from] egui_wgpu_backend::BackendError),
}
