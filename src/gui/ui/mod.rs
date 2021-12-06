mod file_dialog;
mod instance;
mod widgets;

use crate::{
    generator::{
        cpu::CpuFractalGeneratorFactory, gpu::GpuFractalGeneratorFactory, FractalGeneratorFactory,
    },
    gpu::{GPUContext, GPUContextType},
    gui::{
        keyboard::{tree::ShortcutTreeNode, ShortcutMap, ShortcutName},
        storage::CfgUiSettings,
        ui::{
            instance::{
                UIInstance, UIInstanceCreationContext, UIInstanceGenerationType, UIInstanceInfo,
                UIInstanceInitialSettings, UIInstanceRenderContext, UIInstanceUpdateContext,
            },
            widgets::{
                tab_list::{tab_list, SimpleTab},
                util::{shortcut_button, shortcut_checkbox},
            },
        },
        util::get_trace_path,
    },
    storage::{CfgFractalGeneratorType, CfgGeneral, CfgSingleton},
    util::{future::future_wrapper::FutureWrapper, result::ResultExt, running_guard::RunningGuard},
};
use egui::{vec2, Align, Align2, CtxRef, DragValue, Label, Layout};
use egui_wgpu_backend::RenderPass;
use num_complex::Complex32;
use std::{
    collections::HashMap,
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    runtime::Handle,
    task::{yield_now, JoinHandle},
};
use wgpu::{
    DeviceDescriptor, Instance, Maintain, PowerPreference, RequestAdapterOptions,
    RequestDeviceError,
};
use winit::dpi::PhysicalSize;

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
    start_fullscreen: bool,
    initial_window_width: u32,
    initial_window_height: u32,

    // generator stuff
    instance: Arc<Instance>,
    factory_future: FutureWrapper<
        JoinHandle<
            Result<
                (
                    Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
                    RunningGuard,
                ),
                CreateGpuFactoryError,
            >,
        >,
    >,
    factory: Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
    gpu_poll: Option<RunningGuard>,

    // instances
    instances: HashMap<u64, UIInstance>,
    next_instance_id: u64,
    tabs: Vec<SimpleTab<u64>>,
    dragging_tab: Option<usize>,
    current_tab: usize,
    new_instance_requested: bool,
    next_instance_name_index: u64,
    tab_close_requested: Option<usize>,
    instance_operations: UIOperations,
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

pub struct UIUpdateContext<'a> {
    /// WGPU Egui Render Pass reference for managing textures.
    pub render_pass: &'a mut RenderPass,
    pub shortcuts: &'a mut ShortcutMap,
}

/// Struct containing context passed to the UI render function.
pub struct UIRenderContext<'a> {
    /// Egui context reference.
    pub ctx: &'a CtxRef,
    /// The currently pressed keyboard shortcut if any.
    pub shortcuts: &'a ShortcutMap,
    /// The current inner size of the window.
    pub window_size: PhysicalSize<u32>,
}

#[derive(Default)]
pub struct UIOperations {
    current_id: u64,
    operations: Vec<(u64, UIOperationRequest)>,
}

impl UIOperations {
    pub fn push(&mut self, operation: UIOperationRequest) {
        self.operations.push((self.current_id, operation));
    }
}

/// Used by a UIInstance to indicate any operations it wants the UI to perform.
pub enum UIOperationRequest {
    /// This instance wants the UI to start a separate instance generating a
    /// julia set, and then to switch to that instance.
    StartJuliaSet {
        /// If this instance has a target instance selected, that will be here,
        /// otherwise, create a new one and set this one's target_instance id to
        /// the id of the one created.
        instance_id: Option<u64>,
        /// The C value of the julia set to generate.
        c: Complex32,
    },
    /// This instance wants the UI to stop having it be another instance's
    /// target.
    Detach {
        /// The instance id of the instance who has this instance as its target.
        parent_id: u64,
    },
    /// This instance wants the UI to change what its target instance is.
    SetTarget {
        /// The instance id of the old target instance.
        old_id: Option<u64>,
        /// The instance id of the new target instance.
        new_id: Option<u64>,
    },
    /// This instance wants the UI to switch to another instance's tab.
    SwitchTo {
        /// The instance id of the tab to switch to.
        instance_id: u64,
    },
}

impl FractalRSUI {
    /// Create a new UIState, making sure to initialize all required textures
    /// and such.
    pub fn new(ctx: UICreationContext<'_>) -> FractalRSUI {
        // Set up the fractal generator factory
        info!("Creating Fractal Generator Factory...");
        let factory = Arc::new(GpuFractalGeneratorFactory::new(ctx.present.clone()));

        let mut instances = HashMap::new();
        let mut next_instance_id = 1;

        let first_instance = UIInstance::new(UIInstanceCreationContext {
            name: "Fractal 1",
            handle: ctx.handle.clone(),
            present: ctx.present.clone(),
            factory: factory.clone(),
            render_pass: ctx.render_pass,
            id: next_instance_id,
            initial_settings: Default::default(),
        });
        let first_tab = SimpleTab::new(next_instance_id);
        instances.insert(next_instance_id, first_instance);
        next_instance_id += 1;

        let general = CfgGeneral::read_clone();
        let ui_settings = CfgUiSettings::read_clone();

        FractalRSUI {
            handle: ctx.handle,
            present: ctx.present,
            close_requested: false,
            previous_fullscreen: false,
            request_fullscreen: false,
            show_app_settings: false,
            show_ui_settings: false,
            current_generator_type: general.fractal_generator_type.into(),
            new_generator_type: general.fractal_generator_type.into(),
            chunk_size_power: general.fractal_chunk_size_power,
            start_fullscreen: ui_settings.start_fullscreen,
            initial_window_width: ui_settings.initial_window_width,
            initial_window_height: ui_settings.initial_window_height,
            instance: ctx.instance,
            factory_future: Default::default(),
            factory,
            gpu_poll: None,
            instances,
            next_instance_id,
            tabs: vec![first_tab],
            dragging_tab: None,
            current_tab: 0,
            new_instance_requested: false,
            next_instance_name_index: 2,
            tab_close_requested: None,
            instance_operations: Default::default(),
        }
    }

    /// Update things associated with the UI but that do not involve rendering.
    pub fn update(&mut self, ctx: &mut UIUpdateContext) {
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
            for instance in self.instances.values_mut() {
                instance.set_factory(self.factory.clone());
            }
        }

        if let Some(factory) = self.factory_future.poll_unpin(&self.handle) {
            let factory = factory.expect("Panic while creating new gpu-based factory.");
            if let Some((factory, gpu_poll)) = factory
                .on_err(|e| error!("Error creating dedicated GPU generator factory: {:?}", e))
            {
                self.factory = factory;
                self.gpu_poll = Some(gpu_poll);
            }
        }

        // Update all the instances, even the ones that are not currently being
        // rendered.
        for (&id, instance) in self.instances.iter_mut() {
            self.instance_operations.current_id = id;
            instance.update(&mut UIInstanceUpdateContext {
                render_pass: ctx.render_pass,
                chunk_size: 1 << self.chunk_size_power,
                operations: &mut self.instance_operations,
            });
        }

        self.handle_instance_operations(ctx);
        self.handle_new_instance(ctx);
    }

    /// Render the current UI state to the Egui context.
    pub fn draw(&mut self, ctx: &UIRenderContext) {
        let tab_list: Vec<_> = self.tabs.iter().map(|tab| tab.data).collect();
        let instance_infos: HashMap<_, _> = self
            .instances
            .iter()
            .map(|(&id, instance)| {
                (
                    id,
                    UIInstanceInfo {
                        name: instance.name.clone(),
                        running: instance.generation_running,
                    },
                )
            })
            .collect();

        self.handle_keyboard_shortcuts(ctx.shortcuts);
        self.draw_top_bar(ctx);
        if let Some(instance) = self.current_tab() {
            instance.draw(&UIInstanceRenderContext {
                ctx: ctx.ctx,
                shortcuts: ctx.shortcuts,
                tab_list: &tab_list,
                instance_infos: &instance_infos,
            });
        } else {
            self.draw_empty_content(ctx);
        }
        self.draw_settings_window(ctx);
        self.draw_misc_windows(ctx);

        self.handle_tab_close_requested(ctx);
    }

    fn handle_keyboard_shortcuts(&mut self, shortcuts: &ShortcutMap) {
        // Quit keyboard shortcut
        if shortcuts.is_pressed(ShortcutName::App_Quit) {
            self.close_requested = true;
        }

        // New keyboard shortcut
        if shortcuts.is_pressed(ShortcutName::App_New) {
            self.new_instance_requested = true;
        }

        // Close tab keyboard shortcut
        if shortcuts.is_pressed(ShortcutName::App_CloseTab) {
            self.tab_close_requested = Some(self.current_tab);
        }

        // Fullscreen keyboard shortcut
        if shortcuts.is_pressed(ShortcutName::App_Fullscreen) {
            self.request_fullscreen = !self.request_fullscreen;
        }

        // I've found that I often end up trying to use ESC to leave fullscreen, so I
        // think I'll add that as a shortcut.
        if shortcuts.is_pressed(ShortcutName::App_AlternateExitFullscreen) {
            self.request_fullscreen = false;
        }
    }

    fn draw_top_bar(&mut self, ctx: &UIRenderContext) {
        // Draw top bar
        egui::TopBottomPanel::top("Menu Bar").show(ctx.ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::menu::menu(ui, "File", |ui| {
                    if ui.add(shortcut_button!("New", ctx, App_New)).clicked() {
                        self.new_instance_requested = true;
                    }

                    ui.separator();

                    if ui.add(shortcut_button!("Quit", ctx, App_Quit)).clicked() {
                        self.close_requested = true;
                    }
                });
                egui::menu::menu(ui, "Window", |ui| {
                    ui.add(shortcut_checkbox!(
                        &mut self.request_fullscreen,
                        "Fullscreen",
                        ctx,
                        App_Fullscreen
                    ));
                    ui.separator();
                    if ui
                        .add(shortcut_button!("Close Current Tab", ctx, App_CloseTab))
                        .clicked()
                    {
                        self.tab_close_requested = Some(self.current_tab);
                    }
                    ui.separator();
                    if let Some(instance) = self.current_tab() {
                        instance.draw_window_options(ui);
                        ui.separator();
                    }
                    ui.checkbox(&mut self.show_app_settings, "App Settings");
                    ui.checkbox(&mut self.show_ui_settings, "UI Settings");
                });
            });

            ui.separator();

            let res = tab_list(
                ui,
                &mut self.tabs,
                &mut self.current_tab,
                &mut self.dragging_tab,
                |tab| self.instances[&tab.data].name.clone(),
                |ui, tab| ui.make_persistent_id(tab.data),
            );

            if res.close_tab {
                self.tab_close_requested = Some(self.current_tab);
            }
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
                egui::CollapsingHeader::new("Generator Settings")
                    .default_open(true)
                    .show(ui, |ui| {
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
                            platforms, it may not run on all platforms. Some Linux/Mesa \
                            combinations can lead to application hangs when using the GPU-based \
                            generator.",
                        );
                        ui.label(
                            "Note 2: The Dedicated GPU option does not actually require you \
                            have multiple GPUs. All this option does is have the generator use a \
                            separate logical device from the display. This device has a much \
                            higher poll-rate, meaning that it can generate faster, but having it \
                            enabled causes the application to use more CPU.",
                        );

                        ui.add(Label::new("Chunk Size:").heading());
                        ui.horizontal(|ui| {
                            ui.add(Label::new("2^").monospace());
                            ui.add(DragValue::new(&mut self.chunk_size_power).clamp_range(4..=13));
                        });
                        ui.label(
                            "Note that while larger values are generally faster, some drivers \
                            may crash with values that are too large. Most devices handle 2^8 \
                            relatively well. My GTX1060 timed out when rendering a mandelbrot set \
                            at 2^13.",
                        );
                    });

                egui::CollapsingHeader::new("Window Settings")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.checkbox(&mut self.start_fullscreen, "Start Fullscreen");

                        egui::Grid::new("app_settings.window_settings.grid").show(ui, |ui| {
                            ui.label("Initial Window Width:");
                            ui.add(
                                DragValue::new(&mut self.initial_window_width)
                                    .clamp_range(16..=8192),
                            );
                            ui.end_row();

                            ui.label("Initial Window Height:");
                            ui.add(
                                DragValue::new(&mut self.initial_window_height)
                                    .clamp_range(16..=8192),
                            );
                        });
                    });

                egui::CollapsingHeader::new("Keyboard Shortcuts")
                    .default_open(false)
                    .show(ui, |ui| {
                        let mut change_request = None;
                        ShortcutTreeNode::ui(ui, ctx.shortcuts, &mut change_request);
                    });
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

    fn handle_new_instance(&mut self, ctx: &mut UIUpdateContext) {
        if self.new_instance_requested {
            self.new_instance_requested = false;

            // get options from currently open instance if any
            let initial_settings = if let Some(instance) = self.current_tab() {
                UIInstanceInitialSettings::from_instance(instance)
            } else {
                Default::default()
            };

            // When a new instance is creates, we add it to the end of the tabs and select
            // it.
            let new_instance = UIInstance::new(UIInstanceCreationContext {
                name: format!("Fractal {}", self.next_instance_name_index),
                handle: self.handle.clone(),
                present: self.present.clone(),
                factory: self.factory.clone(),
                render_pass: ctx.render_pass,
                id: self.next_instance_id,
                initial_settings,
            });

            let new_tab = SimpleTab::new(self.next_instance_id);
            self.instances.insert(self.next_instance_id, new_instance);
            self.next_instance_name_index = self.next_instance_name_index.wrapping_add(1);
            increment_instance_id(&mut self.next_instance_id, &self.instances);

            self.current_tab = self.tabs.len();
            self.tabs.push(new_tab);
        }
    }

    fn current_tab(&mut self) -> Option<&mut UIInstance> {
        if self.tabs.is_empty() {
            None
        } else {
            if self.current_tab >= self.tabs.len() {
                self.current_tab = self.tabs.len() - 1;
            }

            self.tabs
                .get(self.current_tab)
                .and_then(|tab| self.instances.get_mut(&tab.data))
        }
    }

    fn handle_tab_close_requested(&mut self, ctx: &UIRenderContext) {
        if self.tab_close_requested.is_some() && self.tab_close_requested.unwrap() < self.tabs.len()
        {
            let closing_tab = self.tab_close_requested.unwrap();
            let mut close = true;

            {
                let instance = self
                    .tabs
                    .get(closing_tab)
                    .and_then(|tab| self.instances.get(&tab.data));

                // determine whether we need to show the 'Are you sure?' dialog
                if instance.is_some() && instance.unwrap().dirty {
                    close = false;

                    let instance = instance.unwrap();
                    egui::Window::new("Are you sure?")
                        .resizable(false)
                        .collapsible(false)
                        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                        .show(ctx.ctx, |ui| {
                            ui.label(format!(
                                "Are you sure you want to close the tab: {}",
                                &instance.name
                            ));
                            ui.label("This tab has unsaved changes.");
                            ui.add_space(20.0);
                            ui.with_layout(
                                Layout::right_to_left().with_cross_align(Align::Min),
                                |ui| {
                                    if ui.button("Close Tab").clicked() {
                                        close = true;
                                    }
                                    if ui.button("Keep Tab Open").clicked() {
                                        self.tab_close_requested = None;
                                    }
                                },
                            );
                        });
                }
            }

            if close {
                self.tab_close_requested = None;
                let tab = self.tabs.remove(closing_tab);
                self.instances.remove(&tab.data);

                if self.current_tab >= closing_tab && self.current_tab > 0 {
                    self.current_tab -= 1;
                }
            }
        } else {
            self.tab_close_requested = None;
        }
    }

    fn handle_instance_operations(&mut self, ctx: &mut UIUpdateContext) {
        for (id, operation) in self.instance_operations.operations.drain(..) {
            match operation {
                UIOperationRequest::StartJuliaSet { instance_id, c } => {
                    if let Some(instance) = instance_id
                        .as_ref()
                        .and_then(|id| self.instances.get_mut(id))
                    {
                        let instance_id = instance_id.unwrap();

                        instance.c = c;
                        instance.mandelbrot = false;
                        instance.parent_instance = Some(id);
                        if !instance.generation_running {
                            instance.generate_fractal = Some(UIInstanceGenerationType::Viewer);
                        }

                        // TODO: figure out a more efficient way to find the tab of the selected
                        //  instance
                        for (index, tab) in self.tabs.iter().enumerate() {
                            if instance_id == tab.data {
                                self.current_tab = index;
                            }
                        }
                    } else {
                        // Create a new instance for generating this julia set
                        let initial_settings = UIInstanceInitialSettings {
                            c,
                            mandelbrot: false,
                            ..Default::default()
                        };

                        // There's a lot of duplicated code here, but until I can use disjoint
                        // methods, there isn't really a good way around it.
                        let mut new_instance = UIInstance::new(UIInstanceCreationContext {
                            name: format!("Julia {}", self.next_instance_name_index),
                            handle: self.handle.clone(),
                            present: self.present.clone(),
                            factory: self.factory.clone(),
                            render_pass: ctx.render_pass,
                            id: self.next_instance_id,
                            initial_settings,
                        });

                        new_instance.c = c;
                        new_instance.generate_fractal = Some(UIInstanceGenerationType::Viewer);
                        new_instance.parent_instance = Some(id);

                        self.instances
                            .get_mut(&id)
                            .expect(
                                "Unable to get instance sending StartJulia request (this is a bug)",
                            )
                            .set_target_instance(Some(self.next_instance_id));

                        let new_tab = SimpleTab::new(self.next_instance_id);
                        self.instances.insert(self.next_instance_id, new_instance);
                        self.next_instance_name_index =
                            self.next_instance_name_index.wrapping_add(1);
                        increment_instance_id(&mut self.next_instance_id, &self.instances);

                        self.current_tab = self.tabs.len();
                        self.tabs.push(new_tab);
                    }
                },
                UIOperationRequest::Detach { parent_id } => {
                    if self.instances.contains_key(&parent_id) {
                        // this should never be none
                        self.instances
                            .get_mut(&parent_id)
                            .unwrap()
                            .set_target_instance(None);
                        self.instances
                            .get_mut(&id)
                            .expect("Unable to get instance sending Detach request (this is a bug)")
                            .parent_instance = None;
                    }
                },
                UIOperationRequest::SetTarget { old_id, new_id } => {
                    if let Some(old_instance) = old_id.and_then(|id| self.instances.get_mut(&id)) {
                        old_instance.parent_instance = None;
                    }

                    let mut old_parent_instance = None;
                    if let Some(new_instance) = new_id.and_then(|id| self.instances.get_mut(&id)) {
                        old_parent_instance = new_instance.parent_instance;
                        new_instance.parent_instance = Some(id);
                    }

                    // Make sure no other instances are pointing to the new instance
                    if let Some(old_parent_instance) = old_parent_instance
                        .as_ref()
                        .and_then(|id| self.instances.get_mut(id))
                    {
                        old_parent_instance.set_target_instance(None);
                    }

                    self.instances
                        .get_mut(&id)
                        .expect("Unable to get instance sending SetTarget request (this is a bug)")
                        .set_target_instance(new_id);
                },
                UIOperationRequest::SwitchTo { instance_id } => {
                    // TODO: figure out a more efficient way to find the tab of the selected
                    //  instance
                    for (index, tab) in self.tabs.iter().enumerate() {
                        if instance_id == tab.data {
                            self.current_tab = index;
                        }
                    }
                },
            }
        }
    }

    pub fn store_settings(&self) {
        {
            let mut cfg = CfgGeneral::write();
            cfg.fractal_generator_type = self.current_generator_type.into();
            cfg.fractal_chunk_size_power = self.chunk_size_power;
        }
        {
            let mut cfg = CfgUiSettings::write();
            cfg.start_fullscreen = self.start_fullscreen;
            cfg.initial_window_width = self.initial_window_width;
            cfg.initial_window_height = self.initial_window_height;
        }
    }
}

fn increment_instance_id(next_instance_id: &mut u64, instances: &HashMap<u64, UIInstance>) {
    *next_instance_id = next_instance_id.wrapping_add(1);
    while instances.contains_key(next_instance_id) {
        *next_instance_id = next_instance_id.wrapping_add(1);
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum GeneratorType {
    CPU,
    PresentGPU,
    DedicatedGPU,
}

impl From<CfgFractalGeneratorType> for GeneratorType {
    fn from(source: CfgFractalGeneratorType) -> Self {
        match source {
            CfgFractalGeneratorType::Cpu => Self::CPU,
            CfgFractalGeneratorType::Gpu => Self::PresentGPU,
            CfgFractalGeneratorType::GpuDedicated => Self::DedicatedGPU,
        }
    }
}

impl From<GeneratorType> for CfgFractalGeneratorType {
    fn from(source: GeneratorType) -> Self {
        match source {
            GeneratorType::CPU => Self::Cpu,
            GeneratorType::PresentGPU => Self::Gpu,
            GeneratorType::DedicatedGPU => Self::GpuDedicated,
        }
    }
}

async fn create_gpu_factory(
    instance: Arc<Instance>,
) -> Result<
    (
        Arc<dyn FractalGeneratorFactory + Send + Sync + 'static>,
        RunningGuard,
    ),
    CreateGpuFactoryError,
> {
    info!("Getting dedicated GPU for fractal generation...");
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .ok_or(CreateGpuFactoryError::RequestAdapterError)?;
    let trace_path = get_trace_path("dedicated", false).await?;
    let (device, queue) = adapter
        .request_device(
            &DeviceDescriptor {
                label: Some("High-Performance Device"),
                features: Default::default(),
                limits: Default::default(),
            },
            trace_path.as_ref().map(|p| p.as_path()),
        )
        .await?;

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

    Ok((
        Arc::new(GpuFractalGeneratorFactory::new(dedicated)),
        RunningGuard::new(status),
    ))
}

#[derive(Debug, Error)]
enum CreateGpuFactoryError {
    #[error("IO error")]
    IOError(#[from] io::Error),
    #[error("Unable to retrieve high-performance GPUAdapter")]
    RequestAdapterError,
    #[error("Error requesting dedicated logical device")]
    RequestDeviceError(#[from] RequestDeviceError),
}
