use crate::{
    generator::view::View,
    gui::{keyboard::KeyboardTracker, viewer::FractalViewer},
    util::result::ResultExt,
};
use egui::{vec2, CtxRef, DragValue, ProgressBar};
use egui_wgpu_backend::RenderPass;
use std::borrow::Cow;
use wgpu::Device;
use winit::event::VirtualKeyCode;

const DEFAULT_GENERATION_MESSAGE: &str = "Not Generating";

/// Struct specifically devoted to UI rendering and state.
pub struct UIState {
    // application flow controls
    pub close_requested: bool,

    // fullscreen controls
    pub previous_fullscreen: bool,
    pub request_fullscreen: bool,

    // open windows
    pub show_generator_controls: bool,
    pub show_viewer_controls: bool,
    pub show_ui_settings: bool,

    // generator controls
    pub generate_fractal: bool,
    pub generation_fraction: f32,
    pub generation_message: Cow<'static, str>,
    pub edit_fractal_width: usize,
    pub edit_fractal_height: usize,
    pub edit_fractal_plane_width: f32,
    pub edit_fractal_plane_centered: bool,
    pub edit_fractal_plane_center_x: f32,
    pub edit_fractal_plane_center_y: f32,
    pub fractal_view: View,

    // fractal viewers
    pub julia_viewer: FractalViewer,
}

/// Struct containing context passed when creating UIState.
pub struct UICreationContext<'a> {
    /// Device reference.
    pub device: &'a Device,
    /// WGPU Egui Render Pass reference for managing textures.
    pub render_pass: &'a mut RenderPass,
    /// Fractal view settings at the time of UI state creation.
    pub initial_fractal_view: View,
}

/// Struct containing context passed to the UI render function.
pub struct UIRenderContext<'a> {
    /// Device reference.
    pub device: &'a Device,
    /// Egui context reference.
    pub ctx: &'a CtxRef,
    /// WGPU Egui Render Pass reference for managing textures.
    pub render_pass: &'a mut RenderPass,
    /// Whether the fractal generator instance is currently running.
    pub not_running: bool,
    /// Tracker for currently pressed keys.
    pub keys: &'a KeyboardTracker,
}

impl UIState {
    /// Create a new UIState, making sure to initialize all required textures
    /// and such.
    pub fn new(ctx: &mut UICreationContext) -> UIState {
        let plane_width =
            ctx.initial_fractal_view.image_width as f32 * ctx.initial_fractal_view.image_scale_x;
        let plane_height =
            ctx.initial_fractal_view.image_height as f32 * ctx.initial_fractal_view.image_scale_y;
        let center_x = ctx.initial_fractal_view.plane_start_x + plane_width / 2.0;
        let center_y = ctx.initial_fractal_view.plane_start_y + plane_height / 2.0;

        UIState {
            close_requested: false,
            previous_fullscreen: false,
            request_fullscreen: false,
            show_generator_controls: true,
            show_viewer_controls: true,
            show_ui_settings: false,
            generate_fractal: false,
            generation_fraction: 0.0,
            generation_message: Cow::Borrowed(DEFAULT_GENERATION_MESSAGE),
            edit_fractal_width: ctx.initial_fractal_view.image_width,
            edit_fractal_height: ctx.initial_fractal_view.image_height,
            edit_fractal_plane_width: plane_width,
            edit_fractal_plane_centered: center_x == 0.0 && center_y == 0.0,
            edit_fractal_plane_center_x: center_x,
            edit_fractal_plane_center_y: center_y,
            fractal_view: ctx.initial_fractal_view,
            julia_viewer: FractalViewer::new(ctx.device, ctx.render_pass, ctx.initial_fractal_view),
        }
    }

    /// Render the current UI state to the Egui context.
    pub fn draw(&mut self, ctx: &mut UIRenderContext) {
        self.handle_keyboard_shortcuts(ctx);
        self.draw_menubar(ctx);
        self.draw_fractal_viewers(ctx);
        self.draw_generator_controls(ctx);
        self.draw_viewer_controls(ctx);
        self.draw_misc_windows(ctx);
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &UIRenderContext) {
        let keys = ctx.keys;

        // Quit keyboard shortcut
        if keys.modifiers().command && keys.was_pressed(VirtualKeyCode::Q) {
            self.close_requested = true;
        }

        // Fullscreen keyboard shortcut
        if keys.was_pressed(VirtualKeyCode::F11) {
            self.request_fullscreen = !self.request_fullscreen;
        }
    }

    fn draw_menubar(&mut self, ctx: &UIRenderContext) {
        egui::TopBottomPanel::top("Menu Bar").show(ctx.ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::menu::menu(ui, "File", |ui| {
                    if ui.button("Quit").clicked() {
                        self.close_requested = true;
                    }
                });
                egui::menu::menu(ui, "Window", |ui| {
                    ui.checkbox(&mut self.request_fullscreen, "Fullscreen");
                    ui.separator();
                    ui.checkbox(&mut self.show_generator_controls, "Generator Controls");
                    ui.checkbox(&mut self.show_viewer_controls, "Viewer Controls");
                    ui.checkbox(&mut self.show_ui_settings, "UI Settings");
                });
            });
        });
    }

    fn draw_fractal_viewers(&mut self, ctx: &UIRenderContext) {
        egui::CentralPanel::default().show(ctx.ctx, |ui| {
            let available_size = ui.available_size_before_wrap();
            ui.add_sized(
                available_size,
                self.julia_viewer.widget().max_size_override(available_size),
            );
        });
    }

    fn draw_generator_controls(&mut self, ctx: &mut UIRenderContext) {
        egui::Window::new("Generator Controls")
            .default_size([250.0, 500.0])
            .open(&mut self.show_generator_controls)
            .show(ctx.ctx, |ui| {
                ui.add_enabled_ui(ctx.not_running, |ui| {
                    if ui.button("Generate!").clicked() {
                        self.generate_fractal = true;
                    }
                });

                ui.add(ProgressBar::new(self.generation_fraction).text(&self.generation_message));

                ui.separator();

                // actual generator settings
                egui::Grid::new("generator settings").show(ui, |ui| {
                    ui.label("Image Width:");
                    ui.add(
                        DragValue::new(&mut self.edit_fractal_width)
                            .speed(1.0)
                            .clamp_range(64..=4096),
                    );
                    ui.end_row();

                    ui.label("Image Height:");
                    ui.add(
                        DragValue::new(&mut self.edit_fractal_height)
                            .speed(1.0)
                            .clamp_range(64..=4096),
                    );
                    ui.end_row();

                    ui.label("Plane Width:");
                    ui.add(
                        DragValue::new(&mut self.edit_fractal_plane_width)
                            .clamp_range(0.0..=10.0)
                            .speed(0.03125),
                    );
                    ui.end_row();

                    ui.checkbox(
                        &mut self.edit_fractal_plane_centered,
                        "Centered at (0 + 0i)",
                    );
                    ui.end_row();

                    ui.label("Plane Real Center:");
                    ui.add_enabled(
                        !self.edit_fractal_plane_centered,
                        DragValue::new(&mut self.edit_fractal_plane_center_x)
                            .clamp_range(-10.0..=10.0)
                            .speed(0.0625),
                    );
                    ui.end_row();

                    ui.label("Plane Imaginary Center:");
                    ui.add_enabled(
                        !self.edit_fractal_plane_centered,
                        DragValue::new(&mut self.edit_fractal_plane_center_y)
                            .clamp_range(-10.0..=10.0)
                            .speed(0.0625),
                    );
                    ui.end_row();
                });
            });

        if self.generate_fractal {
            self.apply_generator_settings(ctx);
        }
    }

    fn apply_generator_settings(&mut self, ctx: &mut UIRenderContext) {
        // apply fractal size
        self.fractal_view = if self.edit_fractal_plane_centered {
            View::new_centered_uniform(
                self.edit_fractal_width,
                self.edit_fractal_height,
                self.edit_fractal_plane_width,
            )
        } else {
            View::new_uniform(
                self.edit_fractal_width,
                self.edit_fractal_height,
                self.edit_fractal_plane_width,
                self.edit_fractal_plane_center_x,
                self.edit_fractal_plane_center_y,
            )
        };
        self.julia_viewer
            .set_fractal_view(ctx.device, ctx.render_pass, self.fractal_view)
            .on_err(|e| error!("Error resizing fractal image: {:?}", e));
    }

    fn draw_viewer_controls(&mut self, ctx: &UIRenderContext) {
        egui::Window::new("Viewer Controls")
            .default_size([250.0, 500.0])
            .open(&mut self.show_viewer_controls)
            .show(&ctx.ctx, |ui| {
                ui.label("Zoom & Center");
                ui.horizontal(|ui| {
                    if ui.button("Zoom 1:1").clicked() {
                        self.julia_viewer.zoom_1_to_1();
                    }
                    if ui.button("Zoom Fit").clicked() {
                        self.julia_viewer.zoom_fit();
                    }
                    if ui.button("Zoom Fill").clicked() {
                        self.julia_viewer.zoom_fill();
                    }
                });
                if ui.button("Center View").clicked() {
                    self.julia_viewer.fractal_offset = vec2(0.0, 0.0);
                }

                ui.separator();

                ui.label("Selection");
                if ui.button("Deselect Position").clicked() {
                    self.julia_viewer.selection_pos = None;
                }
            });
    }

    fn draw_misc_windows(&mut self, ctx: &UIRenderContext) {
        egui::Window::new("UI Settings")
            .open(&mut self.show_ui_settings)
            .show(ctx.ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ctx.ctx.settings_ui(ui);
                });
            });
    }
}
