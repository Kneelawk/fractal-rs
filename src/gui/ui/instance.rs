use crate::{
    generator::{
        args::{Multisampling, Smoothing},
        gpu::GpuFractalGeneratorFactory,
        manager::GeneratorManager,
        view::View,
        FractalOpts,
    },
    gui::ui::{viewer::FractalViewer, UIRenderContext},
    util::result::ResultExt,
};
use egui::{vec2, DragValue, ProgressBar, Ui};
use egui_wgpu_backend::RenderPass;
use num_complex::Complex32;
use std::{borrow::Cow, sync::Arc};
use wgpu::{Device, Queue};

const MAX_CHUNK_WIDTH: usize = 256;
const MAX_CHUNK_HEIGHT: usize = 256;
const DEFAULT_GENERATION_MESSAGE: &str = "Not Generating";

/// The UI is broken up into instances, much like how PhotoShop has open files.
/// These instances manage most of the UI and the actual fractal generation.
pub struct UIInstance {
    // instance stuff
    pub name: String,
    device: Arc<Device>,
    queue: Arc<Queue>,
    manager: GeneratorManager,

    // open windows
    pub show_generator_controls: bool,
    pub show_viewer_controls: bool,

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
    pub viewer: FractalViewer,
}

/// Struct holding all the information needed when creating a new UIInstance.
pub struct UIInstanceCreationContext<'a, S: ToString> {
    /// The name of this ui instance.
    pub name: S,
    /// Device reference.
    pub device: Arc<Device>,
    /// Queue reference.
    pub queue: Arc<Queue>,
    /// WGPU Egui Render Pass reference for managing textures.
    pub render_pass: &'a mut RenderPass,
    /// Fractal view settings at the time of UI state creation.
    pub initial_fractal_view: View,
}

impl UIInstance {
    pub async fn new(ctx: UIInstanceCreationContext<'_, impl ToString>) -> UIInstance {
        // obtain original values from view
        let plane_width =
            ctx.initial_fractal_view.image_width as f32 * ctx.initial_fractal_view.image_scale_x;
        let plane_height =
            ctx.initial_fractal_view.image_height as f32 * ctx.initial_fractal_view.image_scale_y;
        let center_x = ctx.initial_fractal_view.plane_start_x + plane_width / 2.0;
        let center_y = ctx.initial_fractal_view.plane_start_y + plane_height / 2.0;

        // Set up the fractal generator factory
        info!("Creating Fractal Generator Factory...");
        let factory = GpuFractalGeneratorFactory::new(ctx.device.clone(), ctx.queue.clone());

        let manager = GeneratorManager::new(Box::new(factory));

        let viewer = FractalViewer::new(&ctx.device, ctx.render_pass, ctx.initial_fractal_view);

        UIInstance {
            name: ctx.name.to_string(),
            device: ctx.device,
            queue: ctx.queue,
            manager,
            show_generator_controls: true,
            show_viewer_controls: true,
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
            viewer,
        }
    }

    pub fn update(&mut self) {
        if self.generate_fractal {
            self.generate_fractal = false;

            if !self.manager.running() {
                let opts = FractalOpts {
                    mandelbrot: false,
                    iterations: 200,
                    smoothing: Smoothing::from_logarithmic_distance(4.0, 2.0),
                    multisampling: Multisampling::Linear { axial_points: 16 },
                    c: Complex32 {
                        re: 0.16611,
                        im: 0.59419,
                    },
                };
                let views: Vec<_> = self
                    .fractal_view
                    .subdivide_rectangles(MAX_CHUNK_WIDTH, MAX_CHUNK_HEIGHT)
                    .collect();
                self.manager.start_to_gpu(
                    opts,
                    &views,
                    self.device.clone(),
                    self.queue.clone(),
                    self.viewer.get_texture(),
                    self.viewer.get_texture_view()
                ).expect("Attempted to start new fractal generator while one was already running! (This is a bug)");
            }
        }

        self.manager
            .poll()
            .on_err(|e| error!("Error polling instance manager: {:?}", e));

        let gen_progress = self.manager.progress();
        self.generation_fraction = gen_progress;
        self.generation_message = Cow::Owned(format!("{:.1}%", gen_progress * 100.0));
    }

    pub fn draw_window_options(&mut self, _ctx: &UIRenderContext, ui: &mut Ui) {
        ui.checkbox(&mut self.show_generator_controls, "Generator Controls");
        ui.checkbox(&mut self.show_viewer_controls, "Viewer Controls");
    }

    pub fn draw(&mut self, ctx: &mut UIRenderContext) {
        self.draw_fractal_viewers(ctx);
        self.draw_generator_controls(ctx);
        self.draw_viewer_controls(ctx);
    }

    fn draw_fractal_viewers(&mut self, ctx: &UIRenderContext) {
        egui::CentralPanel::default().show(ctx.ctx, |ui| {
            let available_size = ui.available_size_before_wrap();
            ui.add_sized(
                available_size,
                self.viewer.widget().max_size_override(available_size),
            );
        });
    }

    fn draw_generator_controls(&mut self, ctx: &mut UIRenderContext) {
        egui::Window::new("Generator Controls")
            .default_size([250.0, 500.0])
            .open(&mut self.show_generator_controls)
            .show(ctx.ctx, |ui| {
                ui.add_enabled_ui(!self.manager.running(), |ui| {
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
        self.viewer
            .set_fractal_view(&self.device, ctx.render_pass, self.fractal_view)
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
                        self.viewer.zoom_1_to_1();
                    }
                    if ui.button("Zoom Fit").clicked() {
                        self.viewer.zoom_fit();
                    }
                    if ui.button("Zoom Fill").clicked() {
                        self.viewer.zoom_fill();
                    }
                });
                if ui.button("Center View").clicked() {
                    self.viewer.fractal_offset = vec2(0.0, 0.0);
                }

                ui.separator();

                ui.label("Selection");
                if ui.button("Deselect Position").clicked() {
                    self.viewer.selection_pos = None;
                }
            });
    }
}
