use crate::{
    generator::{
        args::{Multisampling, Smoothing},
        manager::{GeneratorManager, PollError, WriteError},
        view::View,
        FractalGeneratorFactory, FractalOpts,
    },
    gpu::GPUContext,
    gui::{
        keyboard::{ShortcutMap, ShortcutName},
        ui::{
            file_dialog::FileDialogWrapper, widgets::viewer::FractalViewer, UIOperationRequest,
            UIOperations,
        },
    },
    util::result::ResultExt,
};
use egui::{
    vec2, Button, Color32, ComboBox, CtxRef, DragValue, Label, Layout, ProgressBar, TextEdit,
    TextStyle, Ui,
};
use egui_wgpu_backend::RenderPass;
use num_complex::Complex32;
use num_traits::Zero;
use rfd::AsyncFileDialog;
use std::{borrow::Cow, collections::HashMap, path::PathBuf, sync::Arc};
use tokio::runtime::Handle;

const DEFAULT_GENERATION_MESSAGE: &str = "Not Generating";
const DEFAULT_WRITER_MESSAGE: &str = "Not Writing Image";

/// The UI is broken up into instances, much like how PhotoShop has open files.
/// These instances manage most of the UI and the actual fractal generation.
pub struct UIInstance {
    // instance stuff
    /// This instance's name.
    pub name: String,
    /// Whether this instance has been changed since the last save.
    pub dirty: bool,
    id: u64,
    present: GPUContext,
    manager: GeneratorManager,

    // open windows
    show_generator_controls: bool,
    show_viewer_controls: bool,
    show_project_settings: bool,

    // generator controls
    pub generate_fractal: Option<UIInstanceGenerationType>,
    pub generation_running: bool,
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

    // backup plane values for resets
    init_fractal_plane_width: f32,
    init_fractal_plane_center_x: f32,
    init_fractal_plane_center_y: f32,

    // mandelbrot & julia/fatou set controls
    pub mandelbrot: bool,
    pub c: Complex32,
    iterations: u32,

    // fractal viewers
    viewer: FractalViewer,
    deselected_position: Complex32,

    // julia target stuff
    generate_julia_from_point: bool,
    switch_to_target: bool,
    switch_to_parent: bool,
    detach_requested: bool,
    target_instance: Option<u64>,
    new_target_instance: Option<u64>,
    pub parent_instance: Option<u64>,

    // zoom stuff
    generate_fractal_with_zoom: bool,
    generate_reset_fractal: bool,
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
    /// The ID of this instance.
    pub id: u64,
    /// UIInstance initial settings.
    pub initial_settings: UIInstanceInitialSettings,
}

/// Settings passed to an instance at creation.
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

/// Context passed to a UIInstance when updating.
pub struct UIInstanceUpdateContext<'a> {
    /// WGPU Egui Render Pass reference for managing textures.
    pub render_pass: &'a mut RenderPass,
    /// The maximum size of generation chunks.
    pub chunk_size: usize,
    /// Whether to cache pipelines if starting a new fractal.
    pub cache_generators: bool,
    /// A vec into which operation requests are inserted.
    pub operations: &'a mut UIOperations,
}

/// Context passed to a UIInstance when rendering.
pub struct UIInstanceRenderContext<'a> {
    /// Egui context reference.
    pub ctx: &'a CtxRef,
    /// The currently pressed keyboard shortcut if any.
    pub shortcuts: &'a ShortcutMap,
    /// A list of the tabs this application has open.
    pub tab_list: &'a [u64],
    /// A map from instance ids to instance names.
    pub instance_infos: &'a HashMap<u64, UIInstanceInfo>,
}

/// Info about a UIInstance.
pub struct UIInstanceInfo {
    /// The name of this instance.
    pub name: String,
    /// Whether this instance is running.
    pub running: bool,
}

/// The type of generation the UIInstance should perform.
#[derive(Copy, Clone)]
pub enum UIInstanceGenerationType {
    Viewer,
    Image,
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
    pub fn new(ctx: UIInstanceCreationContext<impl ToString>) -> UIInstance {
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
            dirty: false,
            id: ctx.id,
            present: ctx.present,
            manager,
            show_generator_controls: true,
            show_viewer_controls: true,
            show_project_settings: true,
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
            init_fractal_plane_width: plane_width,
            init_fractal_plane_center_x: center_x,
            init_fractal_plane_center_y: center_y,
            mandelbrot: ctx.initial_settings.mandelbrot,
            c: ctx.initial_settings.c,
            iterations: ctx.initial_settings.iterations,
            viewer,
            deselected_position: Default::default(),
            generate_julia_from_point: false,
            switch_to_target: false,
            switch_to_parent: false,
            detach_requested: false,
            target_instance: None,
            new_target_instance: None,
            parent_instance: None,
            generate_fractal_with_zoom: false,
            generate_reset_fractal: false,
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

    pub fn update(&mut self, ctx: &mut UIInstanceUpdateContext) {
        if self.generate_fractal.is_some() {
            self.apply_view_settings(ctx);

            let generation_type = self
                .generate_fractal
                .take()
                .expect("Attempted to start fractal generation with None as type! (This is a bug)");

            if !self.manager.running() {
                let view = match generation_type {
                    UIInstanceGenerationType::Viewer => self.viewer_view(),
                    UIInstanceGenerationType::Image => self.image_view(),
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
                    UIInstanceGenerationType::Viewer => {
                        self.manager
                            .start_to_gui(
                                opts,
                                views,
                                ctx.cache_generators,
                                self.present.clone(),
                                self.viewer.get_texture(),
                                self.viewer.get_texture_view(),
                            )
                            .expect(
                                "Attempted to start new fractal generator while one was \
                                already running! (This is a bug)",
                            );
                    },
                    UIInstanceGenerationType::Image => {
                        self.manager
                            .start_to_image(
                                opts,
                                view,
                                views,
                                ctx.cache_generators,
                                PathBuf::from(&self.output_location),
                            )
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

        // If something's selected, let's update the deselected position for when it
        // gets deselected.
        if let Some(selected_position) = self.viewer.selection_pos {
            self.deselected_position = selected_position;
        }

        // If we're wanting to start a julia set, then we need to request that.
        if self.generate_julia_from_point {
            ctx.operations.push(UIOperationRequest::StartJuliaSet {
                instance_id: self.target_instance,
                c: self.deselected_position,
            });
        }
        self.generate_julia_from_point = false;

        // If we're wanting to start generating a zoomed-in fractal, let's set up the
        // settings and set ourselves to start that on the next update() call.
        if self.generate_fractal_with_zoom && !self.generation_running {
            if let Some(new_plane_width) = self.viewer.new_plane_width {
                self.edit_fractal_plane_width = new_plane_width;
            }

            if self.deselected_position != Complex32::zero() {
                self.edit_fractal_plane_centered = false;
                self.edit_fractal_plane_center_x = self.deselected_position.re;
                self.edit_fractal_plane_center_y = self.deselected_position.im;
            } else {
                self.edit_fractal_plane_centered = true;
                self.edit_fractal_plane_center_x = 0.0;
                self.edit_fractal_plane_center_y = 0.0;
            }

            self.generate_fractal = Some(UIInstanceGenerationType::Viewer);

            self.viewer.clear_potential_plane_scale();
            self.viewer.fractal_offset = vec2(0.0, 0.0);
        }
        self.generate_fractal_with_zoom = false;

        // If we're wanting to start generating a reset-zoom fractal, let's set up the
        // settings and set ourselves to start that on the next update() call.
        if self.generate_reset_fractal && !self.generation_running {
            self.edit_fractal_plane_width = self.init_fractal_plane_width;
            self.edit_fractal_plane_center_x = self.init_fractal_plane_center_x;
            self.edit_fractal_plane_center_y = self.init_fractal_plane_center_y;
            self.edit_fractal_plane_centered =
                self.init_fractal_plane_center_y == 0.0 && self.init_fractal_plane_center_x == 0.0;

            self.generate_fractal = Some(UIInstanceGenerationType::Viewer);

            self.viewer.clear_potential_plane_scale();
            self.viewer.fractal_offset = vec2(0.0, 0.0);
        }
        self.generate_reset_fractal = false;

        if self.target_instance != self.new_target_instance {
            // This operation will set target instance, so we don't do it here
            ctx.operations.push(UIOperationRequest::SetTarget {
                old_id: self.target_instance,
                new_id: self.new_target_instance,
            });
        }

        if self.detach_requested && self.parent_instance.is_some() {
            ctx.operations.push(UIOperationRequest::Detach {
                parent_id: self.parent_instance.unwrap(),
            });
        }
        self.detach_requested = false;

        if self.switch_to_target && self.target_instance.is_some() {
            ctx.operations.push(UIOperationRequest::SwitchTo {
                instance_id: self.target_instance.unwrap(),
            });
        }
        self.switch_to_target = false;

        if self.switch_to_parent && self.parent_instance.is_some() {
            ctx.operations.push(UIOperationRequest::SwitchTo {
                instance_id: self.parent_instance.unwrap(),
            });
        }
        self.switch_to_parent = false;
    }

    pub fn draw_window_options(&mut self, ui: &mut Ui) {
        ui.checkbox(&mut self.show_generator_controls, "Generator Controls");
        ui.checkbox(&mut self.show_viewer_controls, "Viewer Controls");
        ui.checkbox(&mut self.show_project_settings, "Project Settings");
    }

    pub fn handle_keyboard_shortcuts(&mut self, ctx: &UIInstanceRenderContext) {
        let shortcuts = ctx.shortcuts;

        // Handle Deselect shortcut
        if shortcuts.is_pressed(ShortcutName::Tab_DeselectPosition) {
            self.viewer.selection_pos = None;
        }

        // Handle Generate shortcut
        if shortcuts.is_pressed(ShortcutName::Tab_Generate) && !self.generation_running {
            self.generate_fractal = Some(UIInstanceGenerationType::Viewer);
        }

        // Handle Julia keyboard shortcut
        if shortcuts.is_pressed(ShortcutName::Tab_SpawnJulia)
            && self.mandelbrot
            && !self
                .target_instance
                .as_ref()
                .and_then(|id| ctx.instance_infos.get(id).map(|info| info.running))
                .unwrap_or(false)
        {
            self.generate_julia_from_point = true;
        }

        // Handle Switch to Julia shortcut
        if shortcuts.is_pressed(ShortcutName::Tab_SwitchToJulia) && self.target_instance.is_some() {
            self.switch_to_target = true;
        }

        // Handle Switch to Mandelbrot shortcut
        if shortcuts.is_pressed(ShortcutName::Tab_SwitchToMandelbrot)
            && self.parent_instance.is_some()
        {
            self.switch_to_parent = true;
        }

        // Handle alternating between scrolling changing the size of the current view or
        // the potential new one.
        if shortcuts.is_pressed(ShortcutName::Tab_ViewerScrollNewOrCurrent) {
            if self.viewer.is_plane_scrolling() {
                self.viewer.switch_to_image_scrolling();
            } else {
                self.viewer.switch_to_plane_scrolling();
            }
        }

        // Handle clearing the potential new zoom value.
        if shortcuts.is_pressed(ShortcutName::Tab_ClearNewZoom) {
            self.viewer.clear_potential_plane_scale();
        }

        // Handle applying the new zoom value.
        if shortcuts.is_pressed(ShortcutName::Tab_ApplyNewZoom) && !self.generation_running {
            self.generate_fractal_with_zoom = true;
        }

        // Handle resetting the fractal zoom and center.
        if shortcuts.is_pressed(ShortcutName::Tab_ApplyResetZoom) && !self.generation_running {
            self.generate_reset_fractal = true;
        }
    }

    pub fn draw(&mut self, ctx: &UIInstanceRenderContext) {
        self.handle_keyboard_shortcuts(ctx);

        self.draw_fractal_viewers(ctx);
        self.draw_generator_controls(ctx);
        self.draw_viewer_controls(ctx);
        self.draw_project_settings(ctx);
    }

    fn draw_fractal_viewers(&mut self, ctx: &UIInstanceRenderContext) {
        egui::CentralPanel::default().show(ctx.ctx, |ui| {
            let available_size = ui.available_size_before_wrap();
            ui.add_sized(
                available_size,
                self.viewer.widget().max_size_override(available_size),
            );
        });
    }

    fn draw_generator_controls(&mut self, ctx: &UIInstanceRenderContext) {
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
                            if ui
                                .button("Generate!")
                                .on_hover_text(format!(
                                    "Shortcut: {}",
                                    ctx.shortcuts.keys_for(&ShortcutName::Tab_Generate)
                                ))
                                .clicked()
                            {
                                self.generate_fractal = Some(UIInstanceGenerationType::Viewer);
                            }

                            egui::Grid::new("generate_to_viewer.image_settings.grid").show(
                                ui,
                                |ui| {
                                    ui.label("Image Width:");
                                    ui.add_sized(
                                        vec2(80.0, ui.spacing().interact_size.y),
                                        DragValue::new(&mut self.edit_viewer_width)
                                            .speed(1.0)
                                            .clamp_range(
                                                2..=self.present.limits.max_texture_dimension_2d,
                                            ),
                                    );
                                    ui.end_row();

                                    ui.label("Image Height:");
                                    ui.add_sized(
                                        vec2(80.0, ui.spacing().interact_size.y),
                                        DragValue::new(&mut self.edit_viewer_height)
                                            .speed(1.0)
                                            .clamp_range(
                                                2..=self.present.limits.max_texture_dimension_2d,
                                            ),
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
                                    self.generate_fractal = Some(UIInstanceGenerationType::Image);
                                }
                            });

                            ui.label("Output Location:");
                            ui.add(
                                TextEdit::singleline(&mut self.output_location)
                                    .desired_width(ui.available_width()),
                            );
                            if ui.button("Choose File").clicked() {
                                self.file_dialog_wrapper
                                    .save_file(
                                        AsyncFileDialog::new().add_filter("PNG Image", &["png"]),
                                    )
                                    .ok();
                            }

                            egui::Grid::new("generate_to_image.image_settings.grid").show(
                                ui,
                                |ui| {
                                    ui.label("Image Width:");
                                    ui.add_sized(
                                        vec2(80.0, ui.spacing().interact_size.y),
                                        DragValue::new(&mut self.edit_image_width)
                                            .speed(1.0)
                                            .clamp_range(2..=65536),
                                    );
                                    ui.end_row();

                                    ui.label("Image Height:");
                                    ui.add_sized(
                                        vec2(80.0, ui.spacing().interact_size.y),
                                        DragValue::new(&mut self.edit_image_height)
                                            .speed(1.0)
                                            .clamp_range(2..=65536),
                                    );
                                    ui.end_row();
                                },
                            );
                        });
                    });

                ui.separator();

                // actual generator settings
                egui::CollapsingHeader::new("Complex Plane Settings")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("complex_plane_settings.grid").show(ui, |ui| {
                            ui.label("Plane Width:");
                            ui.add_sized(
                                vec2(80.0, ui.spacing().interact_size.y),
                                DragValue::new(&mut self.edit_fractal_plane_width)
                                    .clamp_range(0.0..=10.0)
                                    .speed(0.00001),
                            );
                            ui.end_row();

                            ui.checkbox(
                                &mut self.edit_fractal_plane_centered,
                                "Centered at (0 + 0i)",
                            );
                            ui.end_row();

                            ui.label("Plane Real Center:");
                            ui.allocate_ui_with_layout(
                                vec2(80.0, ui.spacing().interact_size.y),
                                Layout::centered_and_justified(ui.layout().main_dir()),
                                |ui| {
                                    ui.set_enabled(!self.edit_fractal_plane_centered);
                                    ui.add(
                                        DragValue::new(&mut self.edit_fractal_plane_center_x)
                                            .clamp_range(-10.0..=10.0)
                                            .speed(0.00001),
                                    );
                                },
                            );
                            ui.end_row();

                            ui.label("Plane Imaginary Center:");
                            ui.allocate_ui_with_layout(
                                vec2(80.0, ui.spacing().interact_size.y),
                                Layout::centered_and_justified(ui.layout().main_dir()),
                                |ui| {
                                    ui.set_enabled(!self.edit_fractal_plane_centered);
                                    ui.add(
                                        DragValue::new(&mut self.edit_fractal_plane_center_y)
                                            .clamp_range(-10.0..=10.0)
                                            .speed(0.00001),
                                    );
                                },
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
                            ui.add_sized(
                                vec2(80.0, ui.spacing().interact_size.y),
                                DragValue::new(&mut self.c.re)
                                    .clamp_range(-10.0..=10.0)
                                    .speed(0.00001)
                                    .min_decimals(7)
                                    .max_decimals(45),
                            );
                            ui.end_row();

                            ui.label("C-imaginary:");
                            ui.add_sized(
                                vec2(80.0, ui.spacing().interact_size.y),
                                DragValue::new(&mut self.c.im)
                                    .clamp_range(-10.0..=10.0)
                                    .speed(0.00001)
                                    .min_decimals(7)
                                    .max_decimals(45),
                            );
                            ui.end_row();

                            ui.label("Iterations:");
                            ui.add_sized(
                                vec2(80.0, ui.spacing().interact_size.y),
                                DragValue::new(&mut self.iterations).clamp_range(1..=1000),
                            );
                            ui.end_row();
                        });
                    });
            });
    }

    fn draw_viewer_controls(&mut self, ctx: &UIInstanceRenderContext) {
        egui::Window::new("Viewer Controls")
            .default_size([340.0, 500.0])
            .open(&mut self.show_viewer_controls)
            .show(&ctx.ctx, |ui| {
                egui::CollapsingHeader::new("Viewer Movement")
                    .default_open(true)
                    .show(ui, |ui| {
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
                    });

                egui::CollapsingHeader::new("Selection")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Deselect Position").clicked() {
                                self.viewer.selection_pos = None;
                            }
                            if ui.button("Select Position").clicked() {
                                self.viewer.selection_pos = Some(self.deselected_position);
                            }
                        });

                        ui.label("Selection Position:");
                        egui::Grid::new("viewer_controls.selection.grid").show(ui, |ui| {
                            let selection_pos =
                                if let Some(selection_pos) = &mut self.viewer.selection_pos {
                                    selection_pos
                                } else {
                                    &mut self.deselected_position
                                };

                            ui.label("Real:");
                            ui.add_sized(
                                vec2(80.0, ui.spacing().interact_size.y),
                                DragValue::new(&mut selection_pos.re)
                                    .speed(0.00001)
                                    .min_decimals(7)
                                    .max_decimals(45),
                            );
                            ui.end_row();

                            ui.label("Imaginary:");
                            ui.add_sized(
                                vec2(80.0, ui.spacing().interact_size.y),
                                DragValue::new(&mut selection_pos.im)
                                    .speed(0.00001)
                                    .min_decimals(7)
                                    .max_decimals(45),
                            );
                            ui.end_row();
                        });

                        ui.add_enabled_ui(
                            self.mandelbrot
                                && !self
                                    .target_instance
                                    .as_ref()
                                    .and_then(|id| {
                                        ctx.instance_infos.get(id).map(|info| info.running)
                                    })
                                    .unwrap_or(false),
                            |ui| {
                                if ui
                                    .button("Generate Julia/Fatou Set at Position")
                                    .on_hover_text(format!(
                                        "Shortcut: {}",
                                        ctx.shortcuts.keys_for(&ShortcutName::Tab_SpawnJulia)
                                    ))
                                    .clicked()
                                {
                                    self.generate_julia_from_point = true;
                                }
                            },
                        );
                    });

                egui::CollapsingHeader::new("New Fractal Zoom")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label("Scroll Mode:");
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(
                                    !self.viewer.is_plane_scrolling(),
                                    "Current Image",
                                )
                                .clicked()
                            {
                                self.viewer.switch_to_image_scrolling();
                            }

                            if ui
                                .selectable_label(self.viewer.is_plane_scrolling(), "New Image")
                                .clicked()
                            {
                                self.viewer.switch_to_plane_scrolling();
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("New Plane width:");

                            ui.allocate_ui_with_layout(
                                vec2(80.0, ui.spacing().interact_size.y),
                                Layout::centered_and_justified(ui.layout().main_dir()),
                                |ui| {
                                    ui.set_enabled(self.viewer.new_plane_width.is_some());

                                    let new_plane_width = if let Some(new_plane_width) =
                                        &mut self.viewer.new_plane_width
                                    {
                                        new_plane_width
                                    } else {
                                        &mut self.edit_fractal_plane_width
                                    };

                                    ui.add(
                                        DragValue::new(new_plane_width)
                                            .clamp_range(-10.0..=10.0)
                                            .speed(0.00001)
                                            .min_decimals(7)
                                            .max_decimals(45),
                                    );
                                },
                            );
                        });

                        ui.horizontal(|ui| {
                            if ui.button("Clear Plane Width").clicked() {
                                self.viewer.clear_potential_plane_scale();
                            }

                            if ui.button("Reset Plane Width").clicked() {
                                self.viewer.reset_potential_plane_scale();
                            }
                        });

                        ui.add_enabled_ui(!self.generation_running, |ui| {
                            if ui
                                .button("Generate New Fractal With Selected Plane")
                                .clicked()
                            {
                                self.generate_fractal_with_zoom = true;
                            }

                            if ui.button("Generate Reset Fractal").clicked() {
                                self.generate_reset_fractal = true;
                            }
                        });
                    });
            });
    }

    fn draw_project_settings(&mut self, ctx: &UIInstanceRenderContext) {
        egui::Window::new("Project Settings")
            .default_size([340.0, 500.0])
            .open(&mut self.show_project_settings)
            .show(ctx.ctx, |ui| {
                ui.label("Project name:");
                ui.add(TextEdit::singleline(&mut self.name).desired_width(ui.available_width()));

                ui.separator();

                egui::CollapsingHeader::new("Generate To/From").show(ui, |ui| {
                    ui.label("Tab to generate selected Julia/Fatou sets in:");
                    ComboBox::from_id_source("project_settings.target_instance")
                        .selected_text(
                            self.target_instance
                                .as_ref()
                                .and_then(|id| {
                                    ctx.instance_infos.get(id).map(|info| info.name.clone())
                                })
                                .unwrap_or("None".to_string()),
                        )
                        .show_ui(ui, |ui| {
                            if ui
                                .add(
                                    Button::new("None")
                                        .text_color(Color32::BLUE)
                                        .text_style(TextStyle::Monospace),
                                )
                                .clicked()
                            {
                                self.new_target_instance = None;
                            }

                            for &id in ctx.tab_list.iter() {
                                let info = &ctx.instance_infos[&id];
                                if id != self.id && ui.button(&info.name).clicked() {
                                    self.new_target_instance = Some(id);
                                }
                            }
                        });
                    ui.add_enabled_ui(self.target_instance.is_some(), |ui| {
                        if ui
                            .button("Switch to Julia/Fatou tab")
                            .on_hover_text(format!(
                                "Shortcut: {}",
                                ctx.shortcuts.keys_for(&ShortcutName::Tab_SwitchToJulia)
                            ))
                            .clicked()
                        {
                            self.switch_to_target = true;
                        }
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("This tab is generated to by:");

                        let parent = self.parent_instance.as_ref().and_then(|id| {
                            ctx.instance_infos.get(id).map(|info| info.name.clone())
                        });
                        let lacks_parent = parent.is_none();

                        let mut label = Label::new(parent.unwrap_or("None".to_string()));
                        if lacks_parent {
                            label = label
                                .text_color(Color32::BLUE)
                                .text_style(TextStyle::Monospace);
                        }

                        ui.add(label);
                    });

                    ui.add_enabled_ui(self.parent_instance.is_some(), |ui| {
                        if ui.button("Disconnect from Mandelbrot tab").clicked() {
                            self.detach_requested = true;
                        }
                        if ui
                            .button("Switch to Mandelbrot tab")
                            .on_hover_text(format!(
                                "Shortcut: {}",
                                ctx.shortcuts
                                    .keys_for(&ShortcutName::Tab_SwitchToMandelbrot)
                            ))
                            .clicked()
                        {
                            self.switch_to_parent = true;
                        }
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

    pub fn set_target_instance(&mut self, target_instance: Option<u64>) {
        self.target_instance = target_instance;
        self.new_target_instance = target_instance;
    }

    fn apply_view_settings(&mut self, ctx: &mut UIInstanceUpdateContext) {
        if let Some(UIInstanceGenerationType::Viewer) = self.generate_fractal {
            self.viewer
                .set_fractal_view(&self.present.device, ctx.render_pass, self.viewer_view())
                .on_err(|e| error!("Error resizing fractal image: {:?}", e));
        }
    }
}
