//! gui/mod.rs - This is where the GUI-based core application logic happens.

use crate::{
    generator::{
        args::{Multisampling, Smoothing},
        gpu::GpuFractalGenerator,
        FractalGeneratorInstance, FractalOpts,
    },
    gui::{
        flow::{Flow, FlowModel, FlowModelInit, FlowSignal},
        ui::{UIRenderContext, UIState},
        viewer::FractalViewer,
    },
    util::push_or_else,
};
use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use num_complex::Complex32;
use std::{
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
    viewer: FractalViewer,
    generator: GpuFractalGenerator,
    start_time: Instant,

    // running state
    commands: Vec<CommandBuffer>,
    ui: UIState,
    current_instance: Option<Box<dyn FractalGeneratorInstance + Send>>,
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

        let mut commands = vec![];

        // Setup Egui
        let platform = Platform::new(PlatformDescriptor {
            physical_width: window_size.width,
            physical_height: window_size.height,
            scale_factor,
            font_definitions: Default::default(),
            style: Default::default(),
        });

        let render_pass = RenderPass::new(&device, frame_format, 1);

        // Set up the fractal viewer element
        info!("Creating Fractal Viewer element...");
        let (viewer, viewer_cb) = FractalViewer::new(
            &device,
            frame_format,
            window_size.width,
            window_size.height,
            INITIAL_FRACTAL_WIDTH,
            INITIAL_FRACTAL_HEIGHT,
        )
        .await
        .expect("Error creating Fractal Viewer element");
        commands.push(viewer_cb);

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
            viewer,
            generator,
            start_time: Instant::now(),
            commands,
            ui: Default::default(),
            current_instance: None,
        }
    }

    async fn event(&mut self, event: &WindowEvent<'_>) -> Option<FlowSignal> {
        if let WindowEvent::Resized(new_size) = event {
            self.window_size = *new_size;

            push_or_else(
                self.viewer
                    .set_frame_size(&self.device, new_size.width, new_size.height)
                    .await,
                &mut self.commands,
                |e| error!("Error setting frame size: {:?}", e),
            );
        }

        if let WindowEvent::ScaleFactorChanged {
            new_inner_size,
            scale_factor,
        } = event
        {
            self.window_size = **new_inner_size;
            self.scale_factor = *scale_factor;

            push_or_else(
                self.viewer
                    .set_frame_size(&self.device, new_inner_size.width, new_inner_size.height)
                    .await,
                &mut self.commands,
                |e| error!("Error setting frame size: {:?}", e),
            );
        }

        None
    }

    async fn all_events(&mut self, event: &Event<FlowSignal>) {
        self.platform.handle_event(event);
    }

    async fn update(&mut self, _update_delta: Duration) -> Option<FlowSignal> {
        if self.ui.generate_fractal {
            self.ui.generate_fractal = false;

            // TODO: start fractal generator
        }

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

        // Start command encoder for presentation rendering
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Frame Render Encoder"),
            });

        // Draw UI
        self.platform.begin_frame();

        self.ui.render(UIRenderContext {
            ctx: &self.platform.context(),
            is_stopped: self.current_instance.is_none(),
        });

        let (_output, paint_commands) = self.platform.end_frame(Some(&self.window));

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
