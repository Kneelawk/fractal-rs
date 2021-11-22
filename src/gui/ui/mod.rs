mod instance;
mod viewer;
mod file_dialog;

use crate::{
    generator::{
        cpu::CpuFractalGeneratorFactory, gpu::GpuFractalGeneratorFactory, FractalGeneratorFactory,
    },
    gui::{
        keyboard::KeyboardTracker,
        ui::instance::{UIInstance, UIInstanceCreationContext, UIInstanceInitialSettings},
    },
};
use egui::{CtxRef, Layout, ScrollArea};
use egui_wgpu_backend::RenderPass;
use std::sync::Arc;
use tokio::runtime::Handle;
use wgpu::{Device, Queue};
use winit::event::VirtualKeyCode;

/// Struct specifically devoted to UI rendering and state.
pub struct FractalRSUI {
    // needed for creating new instances
    handle: Handle,
    device: Arc<Device>,
    queue: Arc<Queue>,

    // application flow controls
    pub close_requested: bool,

    // fullscreen controls
    pub previous_fullscreen: bool,
    pub request_fullscreen: bool,

    // open windows
    show_app_settings: bool,
    show_ui_settings: bool,

    // settings
    current_generator_type: GeneratorType,
    new_generator_type: GeneratorType,

    // generator stuff
    factory: Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,

    // instances
    instances: Vec<UIInstance>,
    current_instance: usize,
    new_instance_requested: bool,
    instance_name_index: u32,
}

/// Struct containing context passed when creating UIState.
pub struct UICreationContext<'a> {
    /// Runtime handle reference.
    pub handle: Handle,
    /// Device reference.
    pub device: Arc<Device>,
    /// Queue reference.
    pub queue: Arc<Queue>,
    /// WGPU Egui Render Pass reference for managing textures.
    pub render_pass: &'a mut RenderPass,
}

/// Struct containing context passed to the UI render function.
pub struct UIRenderContext<'a> {
    /// Egui context reference.
    pub ctx: &'a CtxRef,
    /// WGPU Egui Render Pass reference for managing textures.
    pub render_pass: &'a mut RenderPass,
    /// Tracker for currently pressed keys.
    pub keys: &'a KeyboardTracker,
}

impl FractalRSUI {
    /// Create a new UIState, making sure to initialize all required textures
    /// and such.
    pub fn new(ctx: UICreationContext<'_>) -> FractalRSUI {
        // Set up the fractal generator factory
        info!("Creating Fractal Generator Factory...");
        let factory: Arc<dyn FractalGeneratorFactory + Send + Sync + 'static> = Arc::new(
            GpuFractalGeneratorFactory::new(ctx.device.clone(), ctx.queue.clone()),
        );

        let first_instance = UIInstance::new(UIInstanceCreationContext {
            name: "Fractal 1",
            handle: ctx.handle.clone(),
            device: ctx.device.clone(),
            queue: ctx.queue.clone(),
            factory: factory.clone(),
            render_pass: ctx.render_pass,
            initial_settings: Default::default(),
        });

        FractalRSUI {
            handle: ctx.handle.clone(),
            device: ctx.device,
            queue: ctx.queue,
            close_requested: false,
            previous_fullscreen: false,
            request_fullscreen: false,
            show_app_settings: false,
            show_ui_settings: false,
            current_generator_type: GeneratorType::GPU,
            new_generator_type: GeneratorType::GPU,
            factory: factory.clone(),
            instances: vec![first_instance],
            current_instance: 0,
            new_instance_requested: false,
            instance_name_index: 2,
        }
    }

    /// Update things associated with the UI but that do not involve rendering.
    pub fn update(&mut self) {
        // check to see if our generator type has changed
        if self.current_generator_type != self.new_generator_type {
            self.current_generator_type = self.new_generator_type;

            self.factory = match self.new_generator_type {
                GeneratorType::CPU => Arc::new(CpuFractalGeneratorFactory::new(num_cpus::get())),
                GeneratorType::GPU => Arc::new(GpuFractalGeneratorFactory::new(
                    self.device.clone(),
                    self.queue.clone(),
                )),
            };

            // update the factories for all existing instances
            for instance in self.instances.iter_mut() {
                instance.set_factory(self.factory.clone());
            }
        }

        // Update all the instances, even the ones that are not currently being
        // rendered.
        for instance in self.instances.iter_mut() {
            instance.update();
        }
    }

    /// Render the current UI state to the Egui context.
    pub fn draw(&mut self, ctx: &mut UIRenderContext) {
        self.handle_keyboard_shortcuts(ctx);
        self.draw_top_bar(ctx);
        if let Some(instance) = self.open_instance() {
            instance.draw(ctx);
        } else {
            self.draw_empty_content(ctx);
        }
        self.draw_settings_window(ctx);
        self.draw_misc_windows(ctx);

        self.handle_new_instance(ctx);
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

        // I've found that I often end up trying to use ESC to leave fullscreen, so I
        // think I'll add that as a shortcut.
        if keys.was_pressed(VirtualKeyCode::Escape) {
            self.request_fullscreen = false;
        }
    }

    fn draw_top_bar(&mut self, ctx: &UIRenderContext) {
        egui::TopBottomPanel::top("Menu Bar").show(ctx.ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::menu::menu(ui, "File", |ui| {
                    if ui.button("New").clicked() {
                        self.new_instance_requested = true;
                    }

                    ui.separator();

                    if ui.button("Quit").clicked() {
                        self.close_requested = true;
                    }
                });
                egui::menu::menu(ui, "Window", |ui| {
                    ui.checkbox(&mut self.request_fullscreen, "Fullscreen");
                    ui.separator();
                    if let Some(instance) = self.open_instance() {
                        instance.draw_window_options(ctx, ui);
                        ui.separator();
                    }
                    ui.checkbox(&mut self.show_app_settings, "App Settings");
                    ui.checkbox(&mut self.show_ui_settings, "UI Settings");
                });
            });
            ui.separator();
            ui.with_layout(Layout::right_to_left(), |ui| {
                ui.add_enabled_ui(!self.instances.is_empty(), |ui| {
                    if ui.button("X").clicked() {
                        if self.current_instance < self.instances.len() {
                            self.instances.remove(self.current_instance);
                            if self.current_instance > 0 {
                                self.current_instance -= 1;
                            }
                        } else {
                            self.current_instance = 0;
                        }
                    }
                });
                ui.with_layout(Layout::left_to_right(), |ui| {
                    ScrollArea::horizontal().show(ui, |ui| {
                        for (index, instance) in self.instances.iter().enumerate() {
                            ui.selectable_value(&mut self.current_instance, index, &instance.name);
                        }
                    });
                });
            });
        });
    }

    fn draw_empty_content(&mut self, ctx: &UIRenderContext) {
        egui::CentralPanel::default().show(ctx.ctx, |_ui| {});
    }

    fn draw_settings_window(&mut self, ctx: &UIRenderContext) {
        egui::Window::new("App Settings")
            .default_size([340.0, 500.0])
            .open(&mut self.show_app_settings)
            .show(ctx.ctx, |ui| {
                ui.label("Generator Type:");
                ui.radio_value(&mut self.new_generator_type, GeneratorType::CPU, "CPU");
                ui.radio_value(&mut self.new_generator_type, GeneratorType::GPU, "GPU");
                ui.label("Note that while the GPU generator is significantly faster on most platforms, it may not run on all platforms. Some Linux/Mesa combinations can lead to application hangs when using the GPU-based generator.")
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

    fn handle_new_instance(&mut self, ctx: &mut UIRenderContext) {
        if self.new_instance_requested {
            self.new_instance_requested = false;

            // get options from currently open instance if any
            let initial_settings = if let Some(instance) = self.open_instance() {
                UIInstanceInitialSettings::from_instance(instance)
            } else {
                Default::default()
            };

            // When a new instance is creates, we add it to the end of the tabs and select
            // it.
            let new_instance = UIInstance::new(UIInstanceCreationContext {
                name: format!("Fractal {}", self.instance_name_index),
                handle: self.handle.clone(),
                device: self.device.clone(),
                queue: self.queue.clone(),
                factory: self.factory.clone(),
                render_pass: ctx.render_pass,
                initial_settings,
            });

            self.instance_name_index += 1;

            self.current_instance = self.instances.len();
            self.instances.push(new_instance);
        }
    }

    fn open_instance(&mut self) -> Option<&mut UIInstance> {
        if self.instances.is_empty() {
            None
        } else {
            if self.current_instance >= self.instances.len() {
                self.current_instance = self.instances.len() - 1;
            }

            self.instances.get_mut(self.current_instance)
        }
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum GeneratorType {
    CPU,
    GPU,
}
