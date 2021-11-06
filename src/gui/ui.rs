use egui::{CtxRef, ProgressBar};
use std::borrow::Cow;

/// Struct specifically devoted to UI rendering and state.
#[derive(Clone)]
pub struct UIState {
    pub close_requested: bool,
    pub generate_fractal: bool,
    pub generation_fraction: f32,
    pub generation_message: Cow<'static, str>,
    pub previous_fullscreen: bool,
    pub request_fullscreen: bool,
}

/// Struct containing context passed to the UI render function.
pub struct UIRenderContext<'a> {
    /// Egui context reference.
    pub ctx: &'a CtxRef,
    /// Whether the fractal generator instance is currently running.
    pub is_stopped: bool,
}

impl UIState {
    /// Render the current UI state to the Egui context.
    pub fn render(&mut self, ctx: UIRenderContext) {
        egui::TopBottomPanel::top("Menu Bar").show(ctx.ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::menu::menu(ui, "File", |ui| {
                    if ui.button("Quit").clicked() {
                        self.close_requested = true;
                    }
                });
                egui::menu::menu(ui, "Window", |ui| {
                    ui.checkbox(&mut self.request_fullscreen, "Fullscreen");
                });
            });
        });

        egui::Window::new("Hello World!")
            .default_size([250.0, 500.0])
            .show(ctx.ctx, |ui| {
                ui.label("Hello World!");
                ui.add_enabled_ui(ctx.is_stopped, |ui| {
                    if ui.button("Generate!").clicked() {
                        self.generate_fractal = true;
                    }
                });

                ui.add(ProgressBar::new(self.generation_fraction).text(&self.generation_message));
            });
    }
}
