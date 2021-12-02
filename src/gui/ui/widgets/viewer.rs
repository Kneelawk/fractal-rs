//! viewer.rs - This file holds the systems for the fractal image viewer. This
//! means both image managing and rendering.

use crate::{generator::view::View, gpu::util::create_texture};
use egui::{
    paint::Mesh, vec2, Align2, Color32, PointerButton, Pos2, Rect, Response, Sense, Shape,
    TextStyle, TextureId, Ui, Vec2, Widget,
};
use egui_wgpu_backend::RenderPass;
use num_complex::Complex32;
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
    fractal_view: View,
    fractal_size_f: Vec2,
    image_texture: Arc<Texture>,
    image_texture_view: Arc<TextureView>,
    previous_size: Option<Vec2>,

    // View components
    pub fractal_offset: Vec2,
    pub fractal_scale: f32,

    // Selection Components
    pub selection_pos: Option<Complex32>,
}

impl FractalViewer {
    pub fn new(device: &Device, render_pass: &mut RenderPass, fractal_view: View) -> FractalViewer {
        let (image_texture, image_texture_view) = create_texture(
            device,
            fractal_view.image_width as u32,
            fractal_view.image_height as u32,
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
            fractal_view,
            fractal_size_f: Vec2::new(
                fractal_view.image_width as f32,
                fractal_view.image_height as f32,
            ),
            image_texture,
            image_texture_view,
            previous_size: None,
            fractal_offset: Vec2::new(0.0, 0.0),
            fractal_scale: 1.0,
            selection_pos: None,
        }
    }

    pub fn get_texture(&self) -> Arc<Texture> {
        self.image_texture.clone()
    }

    pub fn get_texture_view(&self) -> Arc<TextureView> {
        self.image_texture_view.clone()
    }

    pub fn set_fractal_view(
        &mut self,
        device: &Device,
        render_pass: &mut RenderPass,
        fractal_view: View,
    ) -> Result<(), FractalViewerError> {
        let old_view = self.fractal_view;
        self.fractal_view = fractal_view;

        // only update everything if the fractal size has changed
        if fractal_view.image_width != old_view.image_width
            || fractal_view.image_height != old_view.image_height
        {
            let (image_texture, image_texture_view) = create_texture(
                device,
                fractal_view.image_width as u32,
                fractal_view.image_height as u32,
                TextureFormat::Rgba8UnormSrgb,
                TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
            );

            self.image_texture = Arc::new(image_texture);
            self.image_texture_view = Arc::new(image_texture_view);
            self.fractal_size_f = Vec2::new(
                fractal_view.image_width as f32,
                fractal_view.image_height as f32,
            );

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
        }

        Ok(())
    }

    pub fn zoom_1_to_1(&mut self) {
        let previous_scale = self.fractal_scale;
        self.fractal_scale = 1.0;
        self.fractal_offset *= self.fractal_scale / previous_scale;
    }

    pub fn zoom_fit(&mut self) {
        if let Some(previous_size) = self.previous_size {
            let previous_scale = self.fractal_scale;
            self.fractal_scale = (previous_size.x / self.fractal_size_f.x)
                .min(previous_size.y / self.fractal_size_f.y);
            self.fractal_offset *= self.fractal_scale / previous_scale;
        }
    }

    pub fn zoom_fill(&mut self) {
        if let Some(previous_size) = self.previous_size {
            let previous_scale = self.fractal_scale;
            self.fractal_scale = (previous_size.x / self.fractal_size_f.x)
                .max(previous_size.y / self.fractal_size_f.y);
            self.fractal_offset *= self.fractal_scale / previous_scale;
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, opts: &FractalViewerDrawOptions) -> Response {
        let desired_size = opts
            .max_size_override
            .map_or(self.fractal_size_f, |max| max.min(self.fractal_size_f));
        let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());
        let size = rect.size();
        self.previous_size = Some(size);

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
        let img_size = self.fractal_size_f * self.fractal_scale;
        let img_start = rect.min + (size - img_size) / 2.0 + self.fractal_offset;
        let img_rect = Rect::from_min_size(img_start, img_size);

        // handle click events
        if response.clicked() {
            if let Some(click) = response.interact_pointer_pos() {
                let pixel_selection = ((click - img_start) / self.fractal_scale).floor();
                let complex_selection = self.fractal_view.get_local_plane_coordinates((
                    pixel_selection.x as usize,
                    pixel_selection.y as usize,
                ));
                self.selection_pos = Some(complex_selection);
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
            if let Some(complex_selection) = self.selection_pos {
                // calculate the associated pixel position of the selected complex position
                let pixel_selection = self
                    .fractal_view
                    .get_local_unconstrained_pixel_coordinates(complex_selection);
                let pixel_selection = vec2(pixel_selection.0 as f32, pixel_selection.1 as f32);

                // calculate selection highlight position
                let pixel_rect = Rect::from_min_size(
                    img_start + pixel_selection * self.fractal_scale,
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
                        format!("({:.0}, {:.0})", pixel_selection.x, pixel_selection.y),
                        TextStyle::Monospace,
                        POSITION_SELECTION_COLOR,
                    );
                    clip_painter.text(
                        pixel_rect.left_bottom(),
                        Align2::RIGHT_TOP,
                        format!("({} + {}i)", complex_selection.re, complex_selection.im),
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
                            format!("({:.0}, {:.0})", pixel_selection.x, pixel_selection.y),
                            TextStyle::Monospace,
                            POSITION_SELECTION_COLOR,
                        );
                        clip_painter.text(
                            Pos2 {
                                x: pixel_rect.min.x,
                                y: clip_rect.min.y,
                            },
                            Align2::RIGHT_TOP,
                            format!("({} + {}i)", complex_selection.re, complex_selection.im),
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
                            format!("({:.0}, {:.0})", pixel_selection.x, pixel_selection.y),
                            TextStyle::Monospace,
                            POSITION_SELECTION_COLOR,
                        );
                        clip_painter.text(
                            Pos2 {
                                x: pixel_rect.min.x,
                                y: clip_rect.max.y,
                            },
                            Align2::RIGHT_BOTTOM,
                            format!("({} + {}i)", complex_selection.re, complex_selection.im),
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
                            format!("({:.0}, {:.0})", pixel_selection.x, pixel_selection.y),
                            TextStyle::Monospace,
                            POSITION_SELECTION_COLOR,
                        );
                        clip_painter.text(
                            Pos2 {
                                x: clip_rect.min.x,
                                y: pixel_rect.max.y,
                            },
                            Align2::LEFT_TOP,
                            format!("({} + {}i)", complex_selection.re, complex_selection.im),
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
                            format!("({:.0}, {:.0})", pixel_selection.x, pixel_selection.y),
                            TextStyle::Monospace,
                            POSITION_SELECTION_COLOR,
                        );
                        clip_painter.text(
                            Pos2 {
                                x: clip_rect.max.x,
                                y: pixel_rect.max.y,
                            },
                            Align2::RIGHT_TOP,
                            format!("({} + {}i)", complex_selection.re, complex_selection.im),
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
