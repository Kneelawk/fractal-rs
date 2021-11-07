//! viewer.rs - This file holds the systems for the fractal image viewer. This
//! means both image managing and rendering.

use crate::gpu::util::create_texture;
use egui::{
    paint::Mesh, Color32, PointerButton, Pos2, Rect, Response, Sense, Shape, TextureId, Ui, Vec2,
    Widget,
};
use egui_wgpu_backend::RenderPass;
use std::sync::Arc;
use wgpu::{Device, FilterMode, Texture, TextureFormat, TextureUsages, TextureView};

const IMAGE_UV_RECT: Rect = Rect::from_min_max(Pos2 { x: 0.0, y: 0.0 }, Pos2 { x: 1.0, y: 1.0 });

pub struct FractalViewer {
    // Static Components
    texture_id: TextureId,

    // Dynamic Components
    fractal_size: Vec2,
    fractal_offset: Vec2,
    fractal_scale: f32,
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
            fractal_size: Vec2::new(fractal_width as f32, fractal_height as f32),
            fractal_offset: Vec2::new(0.0, 0.0),
            fractal_scale: 1.0,
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
        self.fractal_size = Vec2::new(width as f32, height as f32);

        render_pass.update_egui_texture_from_wgpu_texture(
            &device,
            &self.image_texture,
            FilterMode::Linear,
            self.texture_id,
        )?;

        Ok(())
    }

    pub fn draw(&mut self, ui: &mut egui::Ui) -> Response {
        let (rect, response) = ui.allocate_at_least(self.fractal_size, Sense::click_and_drag());

        // handle move-drag events
        if response.dragged_by(PointerButton::Middle) {
            self.fractal_offset += response.drag_delta().into();
        }

        // render image
        if ui.clip_rect().intersects(rect) {
            let border = ui.visuals().widgets.noninteractive.bg_stroke;
            let border_width = border.width;

            // calculate image shape
            let clip_rect = Rect::from_min_max(
                rect.min + Vec2::splat(border_width),
                rect.max - Vec2::splat(border_width),
            );

            let size = rect.size();
            let img_size = self.fractal_size * self.fractal_scale;
            let img_start = rect.min + (size - img_size) / 2.0 + self.fractal_offset;
            let img_rect = Rect::from_min_size(img_start, img_size);

            // draw outline
            ui.painter().rect_stroke(rect, 0.0, border);

            // draw image
            let mut mesh = Mesh::with_texture(self.texture_id);
            mesh.add_rect_with_uv(img_rect, IMAGE_UV_RECT, Color32::WHITE);
            ui.painter_at(clip_rect).add(Shape::Mesh(mesh));
        }

        response
    }

    pub fn widget(&mut self) -> FractalViewerWidget {
        FractalViewerWidget { viewer: self }
    }
}

/// Single-use widget form of the FractalViewer. Useful for `Ui::add` and
/// derivatives.
pub struct FractalViewerWidget<'a> {
    viewer: &'a mut FractalViewer,
}

impl<'a> Widget for FractalViewerWidget<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.viewer.draw(ui)
    }
}

#[derive(Debug, Error)]
pub enum FractalViewerError {
    #[error("Egui WGPU Backend Error")]
    BackendError(#[from] egui_wgpu_backend::BackendError),
}
