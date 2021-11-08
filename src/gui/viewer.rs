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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("viewer image sampler"),
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewer bind group"),
            layout: render_pass.bind_group_layout(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&image_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let texture_id = render_pass.egui_texture_from_wgpu_bind_group(bind_group);

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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("viewer image sampler"),
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewer bind group"),
            layout: render_pass.bind_group_layout(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.image_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        render_pass.update_egui_texture_from_wgpu_bind_group(bind_group, self.texture_id)?;

        Ok(())
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, opts: &FractalViewerDrawOptions) -> Response {
        let desired_size = opts
            .max_size_override
            .map_or(self.fractal_size, |max| max.min(self.fractal_size));
        let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

        // handle move-drag events
        if response.dragged_by(PointerButton::Middle) {
            self.fractal_offset += response.drag_delta().into();
        }

        // handle scroll events, but only if we're being hovered over
        if response.hovered() {
            let scroll = ui.input().scroll_delta.y;
            if scroll > 1.0 {
                self.fractal_scale *= 1.1;
                self.fractal_offset *= 1.1;
            } else if scroll < -1.0 {
                self.fractal_scale /= 1.1;
                self.fractal_offset = self.fractal_offset / 1.1;
            }
        }

        // make sure the fractal offset doesn't have the fractal offscreen
        let max_offset_x = self.fractal_size.x * self.fractal_scale / 2.0;
        let max_offset_y = self.fractal_size.y * self.fractal_scale / 2.0;
        if self.fractal_offset.x.abs() > max_offset_x {
            self.fractal_offset.x = self.fractal_offset.x.clamp(-max_offset_x, max_offset_x);
        }
        if self.fractal_offset.y.abs() > max_offset_y {
            self.fractal_offset.y = self.fractal_offset.y.clamp(-max_offset_y, max_offset_y);
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
        FractalViewerWidget {
            viewer: self,
            opts: Default::default(),
        }
    }
}

/// Single-use widget form of the FractalViewer. Useful for `Ui::add` and
/// derivatives.
pub struct FractalViewerWidget<'a> {
    viewer: &'a mut FractalViewer,
    opts: FractalViewerDrawOptions,
}

impl<'a> FractalViewerWidget<'a> {
    /// Set the maximum size of the fractal viewer widget.
    pub fn max_size_override(mut self, max_size: impl Into<Vec2>) -> Self {
        self.opts.max_size_override = Some(max_size.into());
        self
    }
}

impl<'a> Widget for FractalViewerWidget<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.viewer.draw(ui, &self.opts)
    }
}

/// Options passed to the FractalViewer's draw function.
#[derive(Default)]
pub struct FractalViewerDrawOptions {
    pub max_size_override: Option<Vec2>,
}

#[derive(Debug, Error)]
pub enum FractalViewerError {
    #[error("Egui WGPU Backend Error")]
    BackendError(#[from] egui_wgpu_backend::BackendError),
}
