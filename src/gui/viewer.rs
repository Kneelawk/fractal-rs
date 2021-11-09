//! viewer.rs - This file holds the systems for the fractal image viewer. This
//! means both image managing and rendering.

use crate::gpu::util::create_texture;
use cgmath::Vector2;
use egui::{
    paint::Mesh, Align2, Color32, PointerButton, Pos2, Rect, Response, Sense, Shape, TextStyle,
    TextureId, Ui, Vec2, Widget,
};
use egui_wgpu_backend::RenderPass;
use std::sync::Arc;
use wgpu::{
    Device, FilterMode, SamplerDescriptor, Texture, TextureFormat, TextureUsages, TextureView,
};

const IMAGE_UV_RECT: Rect = Rect::from_min_max(Pos2 { x: 0.0, y: 0.0 }, Pos2 { x: 1.0, y: 1.0 });
const POSITION_SELECTION_COLOR: Color32 = Color32::WHITE;

pub struct FractalViewer {
    // Static Components
    texture_id: TextureId,

    // Dynamic Components
    fractal_size_u: Vector2<u32>,
    fractal_size_f: Vec2,
    fractal_offset: Vec2,
    fractal_scale: f32,
    image_texture: Arc<Texture>,
    image_texture_view: Arc<TextureView>,

    // Selection Components
    selection_pos: Option<Vec2>,
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

        let texture_id = render_pass.egui_texture_from_wgpu_texture_with_sampler_options(
            device,
            &image_texture,
            SamplerDescriptor {
                label: Some("viewer image sampler"),
                mag_filter: FilterMode::Nearest,
                min_filter: FilterMode::Linear,
                ..Default::default()
            },
        );

        FractalViewer {
            texture_id,
            fractal_size_u: Vector2::new(fractal_width, fractal_height),
            fractal_size_f: Vec2::new(fractal_width as f32, fractal_height as f32),
            fractal_offset: Vec2::new(0.0, 0.0),
            fractal_scale: 1.0,
            image_texture,
            image_texture_view,
            selection_pos: None,
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
        // only update everything if the fractal size has changed
        if width != self.fractal_size_u.x || height != self.fractal_size_u.y {
            let old_fractal_size = self.fractal_size_f;

            let (image_texture, image_texture_view) = create_texture(
                device,
                width,
                height,
                TextureFormat::Rgba8UnormSrgb,
                TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
            );

            self.image_texture = Arc::new(image_texture);
            self.image_texture_view = Arc::new(image_texture_view);
            self.fractal_size_f = Vec2::new(width as f32, height as f32);

            render_pass.update_egui_texture_from_wgpu_texture_with_sampler_options(
                device,
                &self.image_texture,
                SamplerDescriptor {
                    label: Some("viewer image sampler"),
                    mag_filter: FilterMode::Nearest,
                    min_filter: FilterMode::Linear,
                    ..Default::default()
                },
                self.texture_id,
            )?;

            // adjust selection pos when fractal size changes
            if let Some(selection_pos) = &mut self.selection_pos {
                selection_pos.x =
                    (selection_pos.x * self.fractal_size_f.x / old_fractal_size.x).floor();
                selection_pos.y =
                    (selection_pos.y * self.fractal_size_f.y / old_fractal_size.y).floor();
            }
        }

        Ok(())
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, opts: &FractalViewerDrawOptions) -> Response {
        let desired_size = opts
            .max_size_override
            .map_or(self.fractal_size_f, |max| max.min(self.fractal_size_f));
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
        let max_offset_x = self.fractal_size_f.x * self.fractal_scale / 2.0;
        let max_offset_y = self.fractal_size_f.y * self.fractal_scale / 2.0;
        if self.fractal_offset.x.abs() > max_offset_x {
            self.fractal_offset.x = self.fractal_offset.x.clamp(-max_offset_x, max_offset_x);
        }
        if self.fractal_offset.y.abs() > max_offset_y {
            self.fractal_offset.y = self.fractal_offset.y.clamp(-max_offset_y, max_offset_y);
        }

        // calculate image position and shape
        let size = rect.size();
        let img_size = self.fractal_size_f * self.fractal_scale;
        let img_start = rect.min + (size - img_size) / 2.0 + self.fractal_offset;
        let img_rect = Rect::from_min_size(img_start, img_size);

        // handle click events
        if response.clicked() {
            if let Some(click) = response.interact_pointer_pos() {
                self.selection_pos = Some(((click - img_start) / self.fractal_scale).floor());
            }
        }

        // render image
        if ui.clip_rect().intersects(rect) {
            let border = ui.visuals().widgets.noninteractive.bg_stroke;
            let border_width = border.width;

            // calculate clip rect
            let clip_rect = Rect::from_min_max(
                rect.min + Vec2::splat(border_width),
                rect.max - Vec2::splat(border_width),
            );

            // draw outline
            ui.painter().rect_stroke(rect, 0.0, border);

            // get clipped painter
            let clip_painter = ui.painter_at(clip_rect);

            // draw image
            let mut mesh = Mesh::with_texture(self.texture_id);
            mesh.add_rect_with_uv(img_rect, IMAGE_UV_RECT, Color32::WHITE);
            clip_painter.add(Shape::Mesh(mesh));

            // draw selection pos
            if let Some(selection_vec) = self.selection_pos {
                // calculate selection highlight position
                let pixel_rect = Rect::from_min_size(
                    img_start + selection_vec * self.fractal_scale,
                    Vec2::splat(self.fractal_scale.max(1.0)),
                );

                if clip_rect.contains(pixel_rect.min) || clip_rect.contains(pixel_rect.max) {
                    // selected pixel is on screen
                    clip_painter.rect_filled(
                        Rect::from_min_max(
                            Pos2 {
                                x: pixel_rect.min.x,
                                y: clip_rect.min.y,
                            },
                            Pos2 {
                                x: pixel_rect.max.x,
                                y: pixel_rect.min.y,
                            },
                        ),
                        0.0,
                        POSITION_SELECTION_COLOR,
                    );
                    clip_painter.rect_filled(
                        Rect::from_min_max(
                            Pos2 {
                                x: pixel_rect.min.x,
                                y: pixel_rect.max.y,
                            },
                            Pos2 {
                                x: pixel_rect.max.x,
                                y: clip_rect.max.y,
                            },
                        ),
                        0.0,
                        POSITION_SELECTION_COLOR,
                    );
                    clip_painter.rect_filled(
                        Rect::from_min_max(
                            Pos2 {
                                x: clip_rect.min.x,
                                y: pixel_rect.min.y,
                            },
                            Pos2 {
                                x: pixel_rect.min.x,
                                y: pixel_rect.max.y,
                            },
                        ),
                        0.0,
                        POSITION_SELECTION_COLOR,
                    );
                    clip_painter.rect_filled(
                        Rect::from_min_max(
                            Pos2 {
                                x: pixel_rect.max.x,
                                y: pixel_rect.min.y,
                            },
                            Pos2 {
                                x: clip_rect.max.x,
                                y: pixel_rect.max.y,
                            },
                        ),
                        0.0,
                        POSITION_SELECTION_COLOR,
                    );
                    clip_painter.text(
                        pixel_rect.right_top(),
                        Align2::LEFT_BOTTOM,
                        format!("({:.0}, {:.0})", selection_vec.x, selection_vec.y),
                        TextStyle::Monospace,
                        POSITION_SELECTION_COLOR,
                    );
                } else if clip_rect.x_range().contains(&pixel_rect.min.x)
                    || clip_rect.x_range().contains(&pixel_rect.max.x)
                {
                    // vertical bar to selected pixel is on screen
                    clip_painter.rect_filled(
                        Rect::from_min_max(
                            Pos2 {
                                x: pixel_rect.min.x,
                                y: clip_rect.min.y,
                            },
                            Pos2 {
                                x: pixel_rect.max.x,
                                y: clip_rect.max.y,
                            },
                        ),
                        0.0,
                        POSITION_SELECTION_COLOR,
                    );

                    // figure out where to draw the text
                    if pixel_rect.max.y < clip_rect.min.y {
                        // pixel is at top (min-y) of screen
                        clip_painter.text(
                            Pos2 {
                                x: pixel_rect.max.x,
                                y: clip_rect.min.y,
                            },
                            Align2::LEFT_TOP,
                            format!("({:.0}, {:.0})", selection_vec.x, selection_vec.y),
                            TextStyle::Monospace,
                            POSITION_SELECTION_COLOR,
                        );
                    } else {
                        // pixel is at bottom (max-y) of screen
                        clip_painter.text(
                            Pos2 {
                                x: pixel_rect.max.x,
                                y: clip_rect.max.y,
                            },
                            Align2::LEFT_BOTTOM,
                            format!("({:.0}, {:.0})", selection_vec.x, selection_vec.y),
                            TextStyle::Monospace,
                            POSITION_SELECTION_COLOR,
                        );
                    }
                } else if clip_rect.y_range().contains(&pixel_rect.min.y)
                    || clip_rect.y_range().contains(&pixel_rect.max.y)
                {
                    // horizontal bar to selected pixel is on screen
                    clip_painter.rect_filled(
                        Rect::from_min_max(
                            Pos2 {
                                x: clip_rect.min.x,
                                y: pixel_rect.min.y,
                            },
                            Pos2 {
                                x: clip_rect.max.x,
                                y: pixel_rect.max.y,
                            },
                        ),
                        0.0,
                        POSITION_SELECTION_COLOR,
                    );

                    // figure out where to draw the text
                    if pixel_rect.max.x < clip_rect.min.x {
                        // pixel is at left (min-x) of screen
                        clip_painter.text(
                            Pos2 {
                                x: clip_rect.min.x,
                                y: pixel_rect.min.y,
                            },
                            Align2::LEFT_BOTTOM,
                            format!("({:.0}, {:.0})", selection_vec.x, selection_vec.y),
                            TextStyle::Monospace,
                            POSITION_SELECTION_COLOR,
                        );
                    } else {
                        // pixel is at right (max-x) of screen
                        clip_painter.text(
                            Pos2 {
                                x: clip_rect.max.x,
                                y: pixel_rect.min.y,
                            },
                            Align2::RIGHT_BOTTOM,
                            format!("({:.0}, {:.0})", selection_vec.x, selection_vec.y),
                            TextStyle::Monospace,
                            POSITION_SELECTION_COLOR,
                        );
                    }
                }
            }
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
