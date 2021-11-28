use crate::{
    generator::{
        args::{Multisampling, Smoothing},
        manager::{GeneratorManager, PollError, WriteError},
        view::View,
        FractalGeneratorFactory, FractalOpts,
    },
    gpu::GPUContext,
    gui::ui::{file_dialog::FileDialogWrapper, widgets::viewer::FractalViewer, UIRenderContext},
    util::result::ResultExt,
};
use egui::{vec2, DragValue, ProgressBar, TextEdit, Ui};
use egui_wgpu_backend::RenderPass;
use num_complex::Complex32;
use rfd::AsyncFileDialog;
use std::{borrow::Cow, path::PathBuf, sync::Arc};
use tokio::runtime::Handle;

const DEFAULT_GENERATION_MESSAGE: &str = "Not Generating";
const DEFAULT_WRITER_MESSAGE: &str = "Not Writing Image";

/// The UI is broken up into instances, much like how PhotoShop has open files.
/// These instances manage most of the UI and the actual fractal generation.
pub struct UIInstance {
    // instance stuff
    pub name: String,
    present: GPUContext,
    manager: GeneratorManager,

    // open windows
    show_generator_controls: bool,
    show_viewer_controls: bool,

    // generator controls
    generate_fractal: Option<GenerationType>,
    generation_running: bool,
    generation_fraction: f32,
    generation_message: Cow<'static, str>,
    writer_fraction: f32,
    writer_message: Cow<'static, str>,

    // image controls
    edit_viewer_width: usize,
    edit_viewer_height: usize,
    output_location: String,
    edit_image_width: usize,
    edit_image_height: usize,
    file_dialog_wrapper: FileDialogWrapper,

    // complex plane controls
    edit_fractal_plane_width: f32,
    edit_fractal_plane_centered: bool,
    edit_fractal_plane_center_x: f32,
    edit_fractal_plane_center_y: f32,

    // mandelbrot & julia/fatou set controls
    mandelbrot: bool,
    c: Complex32,
    iterations: u32,

    // fractal viewers
    viewer: FractalViewer,
}

/// Struct holding all the information needed when creating a new UIInstance.
pub struct UIInstanceCreationContext<'a, S: ToString> {
    /// The name of this ui instance.
    pub name: S,
    /// Runtime handle for running async tasks.
    pub handle: Handle,
    /// Presentable context.
    pub present: GPUContext,
    /// The current fractal generator factory.
    pub factory: Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
    /// WGPU Egui Render Pass reference for managing textures.
    pub render_pass: &'a mut RenderPass,
    /// UIInstance initial settings.
    pub initial_settings: UIInstanceInitialSettings,
}

pub struct UIInstanceInitialSettings {
    /// Fractal view settings at the time of UI state creation.
    pub view: View,
    /// Whether this instance should start off as a mandelbrot set or as a
    /// julia/fatou set.
    pub mandelbrot: bool,
    /// Complex value added to `z` on every iteration of the complex function.
    pub c: Complex32,
    /// The number of times the complex iterative function should be run on `z`.
    pub iterations: u32,
}

pub struct UIInstanceUpdateContext {
    /// The maximum size of generation chunks.
    pub chunk_size: usize,
}

impl Default for UIInstanceInitialSettings {
    fn default() -> Self {
        Self {
            view: View::new_centered_uniform(1024, 1024, 3.0),
            mandelbrot: true,
            c: Complex32 {
                re: 0.16611,
                im: 0.59419,
            },
            iterations: 200,
        }
    }
}

impl UIInstanceInitialSettings {
    pub fn from_instance(instance: &UIInstance) -> Self {
        Self {
            view: instance.viewer_view(),
            mandelbrot: instance.mandelbrot,
            c: instance.c,
            iterations: instance.iterations,
        }
    }
}

impl UIInstance {
    pub fn new(ctx: UIInstanceCreationContext<'_, impl ToString>) -> UIInstance {
        // obtain original values from view
        let plane_width =
            ctx.initial_settings.view.image_width as f32 * ctx.initial_settings.view.image_scale_x;
        let plane_height =
            ctx.initial_settings.view.image_height as f32 * ctx.initial_settings.view.image_scale_y;
        let center_x = ctx.initial_settings.view.plane_start_x + plane_width / 2.0;
        let center_y = ctx.initial_settings.view.plane_start_y + plane_height / 2.0;

        let manager = GeneratorManager::new(ctx.handle.clone(), ctx.factory);

        let viewer = FractalViewer::new(
            &ctx.present.device,
            ctx.render_pass,
            ctx.initial_settings.view,
        );

        UIInstance {
            name: ctx.name.to_string(),
            present: ctx.present,
            manager,
            show_generator_controls: true,
            show_viewer_controls: true,
            generate_fractal: None,
            generation_running: false,
            generation_fraction: 0.0,
            generation_message: Cow::Borrowed(DEFAULT_GENERATION_MESSAGE),
            writer_fraction: 0.0,
            writer_message: Cow::Borrowed(DEFAULT_WRITER_MESSAGE),
            edit_viewer_width: ctx.initial_settings.view.image_width,
            edit_viewer_height: ctx.initial_settings.view.image_height,
            output_location: "".to_string(),
            edit_image_width: 1024,
            edit_image_height: 1024,
            file_dialog_wrapper: FileDialogWrapper::new(ctx.handle),
            edit_fractal_plane_width: plane_width,
            edit_fractal_plane_centered: center_x == 0.0 && center_y == 0.0,
            edit_fractal_plane_center_x: center_x,
            edit_fractal_plane_center_y: center_y,
            mandelbrot: ctx.initial_settings.mandelbrot,
            c: ctx.initial_settings.c,
            iterations: ctx.initial_settings.iterations,
            viewer,
        }
    }

    /// Sets this `UIInstance`'s [`FractalGeneratorFactory`].
    ///
    /// [`FractalGeneratorFactory`]: crate::generator::FractalGeneratorFactory
    pub fn set_factory(
        &mut self,
        factory: Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
    ) {
        self.manager.set_factory(factory);
    }

    pub fn update(&mut self, ctx: UIInstanceUpdateContext) {
        if self.generate_fractal.is_some() {
            let generation_type = self
                .generate_fractal
                .take()
                .expect("Attempted to start fractal generation with None as type! (This is a bug)");

            if !self.manager.running() {
                let view = match generation_type {
                    GenerationType::Viewer => self.viewer_view(),
                    GenerationType::Image => self.image_view(),
                };

                // construct the FractalOpts from UI settings
                let opts = FractalOpts {
                    mandelbrot: self.mandelbrot,
                    iterations: self.iterations,
                    smoothing: Smoothing::from_logarithmic_distance(4.0, 2.0),
                    multisampling: Multisampling::Linear { axial_points: 16 },
                    c: self.c,
                };

                // subdivide the view
                let views: Vec<_> = view
                    .subdivide_rectangles(ctx.chunk_size, ctx.chunk_size)
                    .collect();

                // start the generator
                match generation_type {
                    GenerationType::Viewer => {
                        self.manager
                            .start_to_gui(
                                opts,
                                views,
                                self.present.clone(),
                                self.viewer.get_texture(),
                                self.viewer.get_texture_view(),
                            )
                            .expect(
                                "Attempted to start new fractal generator while one was \
                                already running! (This is a bug)",
                            );
                    },
                    GenerationType::Image => {
                        self.manager
                            .start_to_image(opts, view, views, PathBuf::from(&self.output_location))
                            .expect(
                                "Attempted to start a new gractal generator while one was \
                                already running! (This is a bug)",
                            );
                    },
                }
            }
        }

        if let Err(e) = self.manager.poll() {
            match e {
                PollError::WriteError(WriteError::Canceled) => {
                    info!("Image writer canceled.");
                },
                e @ _ => {
                    error!("Error polling instance manager: {:?}", e);
                },
            }
        }

        self.generation_running = self.manager.running();
        let gen_progress = self.manager.progress();
        self.generation_fraction = gen_progress;
        self.generation_message = Cow::Owned(format!("{:.1}%", gen_progress * 100.0));
        let writer_progress = self.manager.writer_progress();
        self.writer_fraction = writer_progress;
        self.writer_message = Cow::Owned(format!("{:.1}%", writer_progress * 100.0));

        let res = self.file_dialog_wrapper.poll().flatten();
        if let Some(file) = res {
            // FIXME: This could break hilariously on some platforms but I don't see much
            //  use in supporting non-Unicode right now.
            self.output_location = file.path().to_string_lossy().to_string();
        }
    }

    pub fn draw_window_options(&mut self, _ctx: &UIRenderContext, ui: &mut Ui) {
        ui.checkbox(&mut self.show_generator_controls, "Generator Controls");
        ui.checkbox(&mut self.show_viewer_controls, "Viewer Controls");
    }

    pub fn draw(&mut self, ctx: &mut UIRenderContext) {
        self.draw_fractal_viewers(ctx);
        self.draw_generator_controls(ctx);
        self.draw_viewer_controls(ctx);

        if self.generate_fractal.is_some() {
            self.apply_view_settings(ctx);
        }
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
            .default_size([340.0, 500.0])
            .open(&mut self.show_generator_controls)
            .show(ctx.ctx, |ui| {
                ui.add(ProgressBar::new(self.generation_fraction).text(&self.generation_message));

                ui.add_enabled_ui(self.generation_running, |ui| {
                    if ui.button("Cancel Generation").clicked() {
                        self.manager.cancel();
                    }
                });

                ui.separator();

                egui::CollapsingHeader::new("Generate to Viewer")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!self.generation_running, |ui| {
                            if ui.button("Generate!").clicked() {
                                self.generate_fractal = Some(GenerationType::Viewer);
                            }

                            egui::Grid::new("generate_to_viewer.image_settings.grid").show(
                                ui,
                                |ui| {
                                    ui.label("Image Width:");
                                    ui.add(
                                        DragValue::new(&mut self.edit_viewer_width)
                                            .speed(1.0)
                                            .clamp_range(2..=8192),
                                    );
                                    ui.end_row();

                                    ui.label("Image Height:");
                                    ui.add(
                                        DragValue::new(&mut self.edit_viewer_height)
                                            .speed(1.0)
                                            .clamp_range(2..=8192),
                                    );
                                    ui.end_row();
                                },
                            );
                        });
                    });

                egui::CollapsingHeader::new("Generate to Exported Image")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.add(ProgressBar::new(self.writer_fraction).text(&self.writer_message));

                        ui.add_enabled_ui(!self.generation_running, |ui| {
                            ui.add_enabled_ui(!self.output_location.is_empty(), |ui| {
                                if ui.button("Generate!").clicked() {
                                    self.generate_fractal = Some(GenerationType::Image);
                                }
                            });

                            egui::Grid::new("generate_to_image.image_settings.grid")
                                .min_col_width(100.0)
                                .show(ui, |ui| {
                                    ui.label("Output Location:");
                                    ui.add(
                                        TextEdit::singleline(&mut self.output_location)
                                            .desired_width(100.0),
                                    );
                                    if ui.button("Choose File").clicked() {
                                        self.file_dialog_wrapper
                                            .save_file(
                                                AsyncFileDialog::new()
                                                    .add_filter("PNG Image", &["png"]),
                                            )
                                            .ok();
                                    }
                                    ui.end_row();

                                    ui.label("Image Width:");
                                    ui.add(
                                        DragValue::new(&mut self.edit_image_width)
                                            .speed(1.0)
                                            .clamp_range(2..=65536),
                                    );
                                    ui.end_row();

                                    ui.label("Image Height:");
                                    ui.add(
                                        DragValue::new(&mut self.edit_image_height)
                                            .speed(1.0)
                                            .clamp_range(2..=65536),
                                    );
                                    ui.end_row();
                                });
                        });
                    });

                ui.separator();

                // actual generator settings
                egui::CollapsingHeader::new("Complex Plane Settings")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("complex_plane_settings.grid").show(ui, |ui| {
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
                                    .speed(0.03125),
                            );
                            ui.end_row();

                            ui.label("Plane Imaginary Center:");
                            ui.add_enabled(
                                !self.edit_fractal_plane_centered,
                                DragValue::new(&mut self.edit_fractal_plane_center_y)
                                    .clamp_range(-10.0..=10.0)
                                    .speed(0.03125),
                            );
                            ui.end_row();
                        });
                    });

                egui::CollapsingHeader::new("Fractal Options")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("fractal_options.grid").show(ui, |ui| {
                            ui.label("Fractal Type:");
                            ui.end_row();
                            ui.selectable_value(&mut self.mandelbrot, true, "Mandelbrot Set");
                            ui.selectable_value(&mut self.mandelbrot, false, "Julia/Fatou Set");
                            ui.end_row();

                            ui.label("C-real:");
                            ui.add(
                                DragValue::new(&mut self.c.re)
                                    .clamp_range(-10.0..=10.0)
                                    .speed(0.0001),
                            );
                            ui.end_row();

                            ui.label("C-imaginary:");
                            ui.add(
                                DragValue::new(&mut self.c.im)
                                    .clamp_range(-10.0..=10.0)
                                    .speed(0.0001),
                            );
                            ui.end_row();

                            ui.label("Iterations:");
                            ui.add(DragValue::new(&mut self.iterations).clamp_range(1..=1000));
                            ui.end_row();
                        });
                    });
            });
    }

    pub fn viewer_view(&self) -> View {
        if self.edit_fractal_plane_centered {
            View::new_centered_uniform(
                self.edit_viewer_width,
                self.edit_viewer_height,
                self.edit_fractal_plane_width,
            )
        } else {
            View::new_uniform(
                self.edit_viewer_width,
                self.edit_viewer_height,
                self.edit_fractal_plane_width,
                self.edit_fractal_plane_center_x,
                self.edit_fractal_plane_center_y,
            )
        }
    }

    pub fn image_view(&self) -> View {
        if self.edit_fractal_plane_centered {
            View::new_centered_uniform(
                self.edit_image_width,
                self.edit_image_height,
                self.edit_fractal_plane_width,
            )
        } else {
            View::new_uniform(
                self.edit_image_width,
                self.edit_image_height,
                self.edit_fractal_plane_width,
                self.edit_fractal_plane_center_x,
                self.edit_fractal_plane_center_y,
            )
        }
    }

    fn apply_view_settings(&mut self, ctx: &mut UIRenderContext) {
        if let Some(GenerationType::Viewer) = self.generate_fractal {
            self.viewer
                .set_fractal_view(&self.present.device, ctx.render_pass, self.viewer_view())
                .on_err(|e| error!("Error resizing fractal image: {:?}", e));
        }
    }

    fn draw_viewer_controls(&mut self, ctx: &UIRenderContext) {
        egui::Window::new("Viewer Controls")
            .default_size([340.0, 500.0])
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

#[derive(Copy, Clone)]
enum GenerationType {
    Viewer,
    Image,
}
