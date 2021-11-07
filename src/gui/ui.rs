use crate::gui::{keyboard::KeyboardTracker, viewer::FractalViewer};
use egui::{CtxRef, ProgressBar};
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
    pub show_ui_settings: bool,

    // generator controls
    pub generate_fractal: bool,
    pub generation_fraction: f32,
    pub generation_message: Cow<'static, str>,

    // fractal viewers
    pub julia_viewer: FractalViewer,
}

/// Struct containing context passed when creating UIState.
pub struct UICreationContext<'a> {
    /// Device reference.
    pub device: &'a Device,
    /// WGPU Egui Render Pass reference for managing textures.
    pub render_pass: &'a mut RenderPass,
    /// Fractal image width at the time of UIState creation.
    pub initial_fractal_width: u32,
    /// Fractal image height at the time of UIState creation.
    pub initial_fractal_height: u32,
}

/// Struct containing context passed to the UI render function.
pub struct UIRenderContext<'a> {
    /// Egui context reference.
    pub ctx: &'a CtxRef,
    /// Whether the fractal generator instance is currently running.
    pub not_running: bool,
    /// Tracker for currently pressed keys.
    pub keys: &'a KeyboardTracker,
}

impl UIState {
    /// Create a new UIState, making sure to initialize all required textures
    /// and such.
    pub fn new(ctx: &mut UICreationContext) -> UIState {
        UIState {
            close_requested: false,
            previous_fullscreen: false,
            request_fullscreen: false,
            show_generator_controls: true,
            show_ui_settings: false,
            generate_fractal: false,
            generation_fraction: 0.0,
            generation_message: Cow::Borrowed(DEFAULT_GENERATION_MESSAGE),
            julia_viewer: FractalViewer::new(
                ctx.device,
                ctx.render_pass,
                ctx.initial_fractal_width,
                ctx.initial_fractal_height,
            ),
        }
    }

    /// Render the current UI state to the Egui context.
    pub fn draw(&mut self, ctx: &UIRenderContext) {
        self.handle_keyboard_shortcuts(ctx);
        self.draw_menubar(ctx);
        self.draw_fractal_viewers(ctx);
        self.draw_generator_controls(ctx);
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
                    ui.checkbox(&mut self.show_ui_settings, "UI Settings");
                });
            });
        });
    }

    fn draw_fractal_viewers(&mut self, ctx: &UIRenderContext) {
        egui::CentralPanel::default().show(ctx.ctx, |ui| {
            let available_size = ui.available_size_before_wrap();
            ui.add_sized(available_size, self.julia_viewer.widget());
        });
    }

    fn draw_generator_controls(&mut self, ctx: &UIRenderContext) {
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
