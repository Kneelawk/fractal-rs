mod file_dialog;
mod instance;
mod widgets;

use crate::{
    generator::{
        cpu::CpuFractalGeneratorFactory, gpu::GpuFractalGeneratorFactory, FractalGeneratorFactory,
    },
    gpu::{GPUContext, GPUContextType},
    gui::{
        keyboard::KeyboardTracker,
        ui::{
            instance::{
                UIInstance, UIInstanceCreationContext, UIInstanceInitialSettings,
                UIInstanceUpdateContext,
            },
            widgets::tab_list::{tab_list, SimpleTab},
        },
    },
    util::{future::future_wrapper::FutureWrapper, running_guard::RunningGuard},
};
use egui::{CtxRef, DragValue, Label};
use egui_wgpu_backend::RenderPass;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::{
    runtime::Handle,
    task::{yield_now, JoinHandle},
};
use wgpu::{DeviceDescriptor, Instance, Maintain, PowerPreference, RequestAdapterOptions};
use winit::event::VirtualKeyCode;

/// Struct specifically devoted to UI rendering and state.
pub struct FractalRSUI {
    // needed for creating new instances
    handle: Handle,
    present: GPUContext,

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
    chunk_size_power: usize,

    // generator stuff
    instance: Arc<Instance>,
    factory_future: FutureWrapper<
        JoinHandle<(
            Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
            RunningGuard,
        )>,
    >,
    factory: Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
    gpu_poll: Option<RunningGuard>,

    // instances
    instances: Vec<SimpleTab<UIInstance>>,
    dragging_instance: Option<usize>,
    current_instance: usize,
    new_instance_requested: bool,
    instance_name_index: u32,
}

/// Struct containing context passed when creating UIState.
pub struct UICreationContext<'a> {
    /// Instance reference.
    pub instance: Arc<Instance>,
    /// Runtime handle reference.
    pub handle: Handle,
    /// Presentable context.
    pub present: GPUContext,
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
        let factory = Arc::new(GpuFractalGeneratorFactory::new(ctx.present.clone()));

        let first_instance = UIInstance::new(UIInstanceCreationContext {
            name: "Fractal 1",
            handle: ctx.handle.clone(),
            present: ctx.present.clone(),
            factory: factory.clone(),
            render_pass: ctx.render_pass,
            initial_settings: Default::default(),
        });

        FractalRSUI {
            handle: ctx.handle,
            present: ctx.present,
            close_requested: false,
            previous_fullscreen: false,
            request_fullscreen: false,
            show_app_settings: false,
            show_ui_settings: false,
            current_generator_type: GeneratorType::PresentGPU,
            new_generator_type: GeneratorType::PresentGPU,
            chunk_size_power: 8,
            instance: ctx.instance,
            factory_future: Default::default(),
            factory,
            gpu_poll: None,
            instances: vec![SimpleTab::new(first_instance)],
            dragging_instance: None,
            current_instance: 0,
            new_instance_requested: false,
            instance_name_index: 2,
        }
    }

    /// Update things associated with the UI but that do not involve rendering.
    pub fn update(&mut self) {
        // check to see if our generator type has changed
        if self.current_generator_type != self.new_generator_type && self.factory_future.is_empty()
        {
            self.current_generator_type = self.new_generator_type;

            match self.new_generator_type {
                GeneratorType::CPU => {
                    self.factory = Arc::new(CpuFractalGeneratorFactory::new(num_cpus::get()));
                    self.gpu_poll = None;
                },
                GeneratorType::PresentGPU => {
                    self.factory = Arc::new(GpuFractalGeneratorFactory::new(self.present.clone()));
                    self.gpu_poll = None;
                },
                GeneratorType::DedicatedGPU => {
                    self.factory_future
                        .insert_spawn(&self.handle, create_gpu_factory(self.instance.clone()))
                        .expect(
                            "Error inserting gpu-based factory creation future into wrapper. \
                            (this is a bug)",
                        );
                },
            };

            // update the factories for all existing instances
            for instance in self.instances.iter_mut() {
                let instance = &mut instance.data;
                instance.set_factory(self.factory.clone());
            }
        }

        if let Some(factory) = self.factory_future.poll_unpin(&self.handle) {
            let (factory, gpu_poll) = factory.expect("Panic while creating new gpu-based factory.");
            self.factory = factory;
            self.gpu_poll = Some(gpu_poll);
        }

        // Update all the instances, even the ones that are not currently being
        // rendered.
        for instance in self.instances.iter_mut() {
            let instance = &mut instance.data;
            instance.update(UIInstanceUpdateContext {
                chunk_size: 1 << self.chunk_size_power,
            });
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
        // Draw top bar
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
            tab_list(
                ui,
                &mut self.instances,
                &mut self.current_instance,
                &mut self.dragging_instance,
                |instance| instance.data.name.clone(),
            );
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
                ui.add(Label::new("Generator Type:").heading());
                ui.radio_value(
                    &mut self.new_generator_type,
                    GeneratorType::CPU,
                    "CPU (Slow)",
                );
                ui.radio_value(
                    &mut self.new_generator_type,
                    GeneratorType::PresentGPU,
                    "Display GPU (Faster)",
                );
                ui.radio_value(
                    &mut self.new_generator_type,
                    GeneratorType::DedicatedGPU,
                    "Dedicated GPU (Fastest)",
                );
                ui.label(
                    "Note 1: While the GPU generator is significantly faster on most \
                    platforms, it may not run on all platforms. Some Linux/Mesa combinations can \
                    lead to application hangs when using the GPU-based generator.",
                );
                ui.label(
                    "Note 2: The Dedicated GPU option does not actually require you have \
                    multiple GPUs. All this option does is have the generator use a separate \
                    logical device from the display. This device has a much higher poll-rate, \
                    meaning that it can generate faster, but having it enabled causes the \
                    application to use more CPU.",
                );

                ui.add(Label::new("Chunk Size:").heading());
                ui.horizontal(|ui| {
                    ui.add(Label::new("2^").monospace());
                    ui.add(DragValue::new(&mut self.chunk_size_power).clamp_range(4..=13));
                });
                ui.label(
                    "Note that while larger values are generally faster, some drivers may \
                    crash with values that are too large. Most devices handle 2^8 relatively well. \
                    My GTX1060 timed out when rendering a mandelbrot set at 2^13.",
                );
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
                present: self.present.clone(),
                factory: self.factory.clone(),
                render_pass: ctx.render_pass,
                initial_settings,
            });

            self.instance_name_index += 1;

            self.current_instance = self.instances.len();
            self.instances.push(SimpleTab::new(new_instance));
        }
    }

    fn open_instance(&mut self) -> Option<&mut UIInstance> {
        if self.instances.is_empty() {
            None
        } else {
            if self.current_instance >= self.instances.len() {
                self.current_instance = self.instances.len() - 1;
            }

            self.instances
                .get_mut(self.current_instance)
                .map(|tab| &mut tab.data)
        }
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum GeneratorType {
    CPU,
    PresentGPU,
    DedicatedGPU,
}

async fn create_gpu_factory(
    instance: Arc<Instance>,
) -> (
    Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
    RunningGuard,
) {
    info!("Getting dedicated GPU for fractal generation...");
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .expect("Unable to retrieve high-performance GPUAdapter.");
    let (device, queue) = adapter
        .request_device(
            &DeviceDescriptor {
                label: Some("High-Performance Device"),
                features: Default::default(),
                limits: Default::default(),
            },
            None,
        )
        .await
        .expect("Error requesting dedicated logical device.");

    let device = Arc::new(device);
    let queue = Arc::new(queue);

    info!("Creating device poll task...");
    let poll_device = device.clone();
    let status = Arc::new(AtomicBool::new(true));
    let poll_status = status.clone();
    tokio::spawn(async move {
        while poll_status.load(Ordering::Acquire) {
            poll_device.poll(Maintain::Poll);
            yield_now().await;
        }
    });

    let dedicated = GPUContext {
        device,
        queue,
        ty: GPUContextType::Dedicated,
    };

    (
        Arc::new(GpuFractalGeneratorFactory::new(dedicated)),
        RunningGuard::new(status),
    )
}
