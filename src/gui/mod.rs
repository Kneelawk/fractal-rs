//! gui/mod.rs - This is where the GUI-based core application logic happens.

use crate::{
    generator::{
        args::{Multisampling, Smoothing},
        gpu::GpuFractalGenerator,
        instance_manager::InstanceManager,
        view::View,
        FractalGenerator, FractalOpts,
    },
    gui::{
        flow::{Flow, FlowModel, FlowModelInit, FlowSignal},
        keyboard::KeyboardTracker,
        ui::{UICreationContext, UIRenderContext, UIState},
    },
};
use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use num_complex::Complex32;
use std::{
    borrow::Cow,
    sync::Arc,
    time::{Duration, Instant},
};
use wgpu::{Color, CommandBuffer, CommandEncoderDescriptor, Device, Queue, TextureView};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    window::{Fullscreen, Window},
};

mod flow;
mod keyboard;
mod ui;
mod viewer;

const INITIAL_FRACTAL_WIDTH: u32 = 1024;
const INITIAL_FRACTAL_HEIGHT: u32 = 1024;

/// Launches the application as a GUI application.
pub fn start_gui_application() -> ! {
    Flow::new()
        .width(1600)
        .height(900)
        .title("Fractal-RS 2")
        .start::<FractalRSGuiMain>()
        .expect("Error starting Flow!")
}

struct FractalRSGuiMain {
    // initialization state
    device: Arc<Device>,
    queue: Arc<Queue>,
    window: Arc<Window>,
    window_size: PhysicalSize<u32>,
    scale_factor: f64,
    platform: Platform,
    render_pass: RenderPass,
    generator: GpuFractalGenerator,
    instance_manager: InstanceManager,
    keyboard_tracker: KeyboardTracker,
    start_time: Instant,

    // running state
    commands: Vec<CommandBuffer>,
    ui: UIState,
}

#[async_trait]
impl FlowModel for FractalRSGuiMain {
    async fn init(init: FlowModelInit) -> Self {
        let device = init.device;
        let queue = init.queue;
        let window = init.window;
        let window_size = init.window_size;
        let scale_factor = window.scale_factor();
        let frame_format = init.frame_format;

        info!("Setting up UI...");

        // Setup Egui
        let platform = Platform::new(PlatformDescriptor {
            physical_width: window_size.width,
            physical_height: window_size.height,
            scale_factor,
            font_definitions: Default::default(),
            style: Default::default(),
        });

        let mut render_pass = RenderPass::new(&device, frame_format, 1);

        info!("Initializing UI State...");
        let ui = UIState::new(&mut UICreationContext {
            device: &device,
            render_pass: &mut render_pass,
            initial_fractal_width: INITIAL_FRACTAL_WIDTH,
            initial_fractal_height: INITIAL_FRACTAL_HEIGHT,
        });

        // Set up the fractal generator
        info!("Creating Fractal Generator...");
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

        let generator = GpuFractalGenerator::new(opts, device.clone(), queue.clone())
            .await
            .expect("Error creating Fractal Generator");

        FractalRSGuiMain {
            device,
            queue,
            window,
            window_size,
            scale_factor,
            platform,
            render_pass,
            generator,
            instance_manager: InstanceManager::new(),
            keyboard_tracker: KeyboardTracker::new(),
            start_time: Instant::now(),
            commands: vec![],
            ui,
        }
    }

    async fn event(&mut self, event: &WindowEvent<'_>) -> Option<FlowSignal> {
        if let WindowEvent::Resized(new_size) = event {
            self.window_size = *new_size;
        }

        if let WindowEvent::ScaleFactorChanged {
            new_inner_size,
            scale_factor,
        } = event
        {
            self.window_size = **new_inner_size;
            self.scale_factor = *scale_factor;
        }

        if let WindowEvent::KeyboardInput { input, .. } = event {
            self.keyboard_tracker.keyboard_input(input);
        }

        if let WindowEvent::ModifiersChanged(state) = event {
            self.keyboard_tracker.modifiers_changed(state);
        }

        None
    }

    async fn all_events(&mut self, event: &Event<FlowSignal>) {
        self.platform.handle_event(event);
    }

    async fn update(&mut self, _update_delta: Duration) -> Option<FlowSignal> {
        if self.ui.generate_fractal {
            self.ui.generate_fractal = false;

            if !self.instance_manager.running() {
                let view = View::new_centered_uniform(
                    self.ui.fractal_width as usize,
                    self.ui.fractal_height as usize,
                    3.0,
                );
                let views: Vec<_> = view.subdivide_rectangles(256, 256).collect();
                self.instance_manager.start(
                    self.generator.start_generation_to_gpu(
                        &views,
                        self.ui.julia_viewer.get_texture(),
                        self.ui.julia_viewer.get_texture_view()
                        )
                    )
                    .expect("Attempted to start new fractal generator while one was already running! (This is a bug)");
            }
        }

        if let Err(e) = self.instance_manager.poll() {
            error!("Error polling instance manager: {:?}", e);
        }

        let gen_progress = self.instance_manager.progress();
        self.ui.generation_fraction = gen_progress;
        self.ui.generation_message = Cow::Owned(format!("{:.1}%", gen_progress * 100.0));

        if self.ui.close_requested {
            Some(FlowSignal::Exit)
        } else if self.ui.request_fullscreen != self.ui.previous_fullscreen {
            self.ui.previous_fullscreen = self.ui.request_fullscreen;
            if self.ui.request_fullscreen {
                Some(FlowSignal::Fullscreen(Some(Fullscreen::Borderless(None))))
            } else {
                Some(FlowSignal::Fullscreen(None))
            }
        } else {
            None
        }
    }

    async fn render(&mut self, frame_view: &TextureView, _render_delta: Duration) {
        // Setup platform for frame
        self.platform
            .update_time(self.start_time.elapsed().as_secs_f64());

        // Draw UI
        self.platform.begin_frame();

        self.ui.draw(&mut UIRenderContext {
            device: &self.device,
            ctx: &self.platform.context(),
            render_pass: &mut self.render_pass,
            not_running: !self.instance_manager.running(),
            keys: &self.keyboard_tracker,
        });

        let (_output, paint_commands) = self.platform.end_frame(Some(&self.window));

        // Clear keyboard keypress events.
        self.keyboard_tracker.reset_keyboard_input();

        // Encode UI draw commands
        let paint_jobs = self.platform.context().tessellate(paint_commands);

        let screen_descriptor = ScreenDescriptor {
            physical_width: self.window_size.width,
            physical_height: self.window_size.height,
            scale_factor: self.scale_factor as f32,
        };
        self.render_pass.update_texture(
            &self.device,
            &self.queue,
            &self.platform.context().texture(),
        );
        self.render_pass
            .update_user_textures(&self.device, &self.queue);
        self.render_pass
            .update_buffers(&self.device, &self.queue, &paint_jobs, &screen_descriptor);

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("UI Render Encoder"),
            });

        self.render_pass
            .execute(
                &mut encoder,
                frame_view,
                &paint_jobs,
                &screen_descriptor,
                Some(Color {
                    r: 0.1,
                    g: 0.1,
                    b: 0.1,
                    a: 1.0,
                }),
            )
            .unwrap();

        // Add UI render command to list of commands for this frame
        self.commands.push(encoder.finish());

        // Submit all commands encoded since the last frame
        self.queue.submit(self.commands.drain(..));
    }

    async fn shutdown(self) {}
}
