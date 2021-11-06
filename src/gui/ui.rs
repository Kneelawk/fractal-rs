use egui::{CtxRef, ProgressBar};
use std::borrow::Cow;

const DEFAULT_GENERATION_MESSAGE: &str = "Not Generating";

/// Struct specifically devoted to UI rendering and state.
#[derive(Clone)]
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
}

/// Struct containing context passed to the UI render function.
pub struct UIRenderContext<'a> {
    /// Egui context reference.
    pub ctx: &'a CtxRef,
    /// Whether the fractal generator instance is currently running.
    pub is_stopped: bool,
}

impl Default for UIState {
    fn default() -> Self {
        UIState {
            close_requested: false,
            previous_fullscreen: false,
            request_fullscreen: false,
            show_generator_controls: true,
            show_ui_settings: false,
            generate_fractal: false,
            generation_fraction: 0.0,
            generation_message: Cow::Borrowed(DEFAULT_GENERATION_MESSAGE),
        }
    }
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
                    ui.separator();
                    ui.checkbox(&mut self.show_generator_controls, "Generator Controls");
                    ui.checkbox(&mut self.show_ui_settings, "UI Settings");
                });
            });
        });

        egui::Window::new("Generator Controls")
            .default_size([250.0, 500.0])
            .open(&mut self.show_generator_controls)
            .show(ctx.ctx, |ui| {
                ui.add_enabled_ui(ctx.is_stopped, |ui| {
                    if ui.button("Generate!").clicked() {
                        self.generate_fractal = true;
                    }
                });

                ui.add(ProgressBar::new(self.generation_fraction).text(&self.generation_message));
            });

        egui::Window::new("UI Settings")
            .open(&mut self.show_ui_settings)
            .show(ctx.ctx, |ui| {
                ctx.ctx.settings_ui(ui);
            });
    }
}
