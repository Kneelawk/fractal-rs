//! gui/mod.rs - This is where the GUI-based core application logic happens.

use crate::{
    generator::{
        args::{Multisampling, Smoothing},
        cpu::CpuFractalGenerator,
        gpu::GpuFractalGenerator,
        view::View,
        FractalGenerator, FractalGeneratorInstance, FractalOpts,
    },
    gui::{
        flow::{Flow, FlowModel},
        viewer::FractalViewer,
    },
    util::{poll_join_result, poll_optional, push_or_else, RunningState},
};
use futures::{future::BoxFuture, FutureExt};
use imgui::{Condition, FontConfig, FontSource, MenuItem, MouseCursor, ProgressBar};
use imgui_wgpu::{Renderer, RendererConfig};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use num_complex::Complex32;
use std::{
    borrow::Cow,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};
use tokio::{runtime::Handle, sync::Mutex, task::JoinHandle};
use wgpu::{
    Color, CommandBuffer, CommandEncoderDescriptor, Device, LoadOp, Operations, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, TextureFormat, TextureView,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    window::Window,
};

mod flow;
mod viewer;

const INITIAL_FRACTAL_WIDTH: u32 = 1024;
const INITIAL_FRACTAL_HEIGHT: u32 = 1024;

const DEFAULT_GENERATION_MESSAGE: &str = "Not Generating";

/// Launches the application as a GUI application.
pub fn start_gui_application() -> ! {
    let mut flow = Flow::new();
    flow.title = "Fractal-RS 2".to_string();

    flow.start::<FractalRSGuiMain>()
        .expect("Error starting Flow!")
}

struct FractalRSGuiMain {
    // initialization state
    handle: Handle,
    device: Arc<Device>,
    queue: Arc<Queue>,
    window: Arc<Window>,
    imgui: imgui::Context,
    platform: WinitPlatform,
    renderer: Renderer,
    viewer: FractalViewer,
    generator: GpuFractalGenerator,

    // running state
    commands: Vec<CommandBuffer>,
    last_cursor: Option<Option<MouseCursor>>,
    state: UIState,
    current_instance: Option<Box<dyn FractalGeneratorInstance + Send>>,
}

#[derive(Clone)]
struct UIState {
    close_requested: bool,
    generate_fractal: bool,
    generation_fraction: f32,
    generation_message: Cow<'static, str>,
}

impl FlowModel for FractalRSGuiMain {
    fn init(
        handle: Handle,
        device: Arc<Device>,
        queue: Arc<Queue>,
        window: Arc<Window>,
        frame_format: TextureFormat,
    ) -> Self {
        info!("Setting up UI...");

        let mut commands = vec![];

        // Set up dear imgui
        let mut imgui = imgui::Context::create();
        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);
        imgui.set_ini_filename(None);

        let hdpi_factor = window.scale_factor();
        let font_size = (13.0 * hdpi_factor) as f32;
        imgui.io_mut().font_global_scale = (1.0 / hdpi_factor) as f32;

        imgui.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(FontConfig {
                oversample_h: 1,
                pixel_snap_h: true,
                size_pixels: font_size,
                ..Default::default()
            }),
        }]);

        // Set up the renderer
        let renderer_config = RendererConfig {
            texture_format: frame_format,
            ..Default::default()
        };

        let renderer = Renderer::new(&mut imgui, &device, &queue, renderer_config);

        // Set up the fractal viewer element
        info!("Creating Fractal Viewer element...");
        let window_size = window.inner_size();
        let (viewer, viewer_cb) = handle
            .block_on(FractalViewer::new(
                &device,
                frame_format,
                window_size.width,
                window_size.height,
                INITIAL_FRACTAL_WIDTH,
                INITIAL_FRACTAL_HEIGHT,
            ))
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

        let generator = handle
            .block_on(GpuFractalGenerator::new(
                opts,
                device.clone(),
                queue.clone(),
            ))
            .expect("Error creating Fractal Generator");

        FractalRSGuiMain {
            handle,
            device,
            queue,
            window,
            imgui,
            platform,
            renderer,
            viewer,
            generator,
            commands,
            last_cursor: None,
            state: UIState {
                close_requested: false,
                generate_fractal: false,
                generation_fraction: 0.0,
                generation_message: Cow::Borrowed(DEFAULT_GENERATION_MESSAGE),
            },
            current_instance: None,
        }
    }

    fn event(&mut self, event: &WindowEvent<'_>) -> Option<ControlFlow> {
        if let WindowEvent::Resized(new_size) = event {
            push_or_else(
                self.handle.block_on(self.viewer.set_frame_size(
                    &self.device,
                    new_size.width,
                    new_size.height,
                )),
                &mut self.commands,
                |e| error!("Error setting frame size: {:?}", e),
            );
        }
        if let WindowEvent::ScaleFactorChanged { new_inner_size, .. } = event {
            push_or_else(
                self.handle.block_on(self.viewer.set_frame_size(
                    &self.device,
                    new_inner_size.width,
                    new_inner_size.height,
                )),
                &mut self.commands,
                |e| error!("Error setting frame size: {:?}", e),
            );
        }

        None
    }

    fn all_events(&mut self, event: &Event<()>) {
        self.platform
            .handle_event(self.imgui.io_mut(), &self.window, event);
    }

    fn update(&mut self, update_delta: Duration) -> Option<ControlFlow> {
        if self.state.generate_fractal {
            self.state.generate_fractal = false;

            // TODO: start fractal generator
        }

        if self.state.close_requested {
            Some(ControlFlow::Exit)
        } else {
            None
        }
    }

    fn render(&mut self, frame_view: &TextureView, render_delta: Duration) {
        self.imgui.io_mut().update_delta_time(render_delta);

        match self
            .platform
            .prepare_frame(self.imgui.io_mut(), &self.window)
        {
            Ok(_) => {},
            Err(e) => {
                error!("Error preparing platform: {:?}", e);
                return;
            },
        }

        // State variables to be stored into self
        let mut state = self.state.clone();

        let ui = self.imgui.frame();

        {
            // Draw UI
            let is_started = self.current_instance.is_some();
            let window = imgui::Window::new("Hello World!");
            window
                .size([400.0, 500.0], Condition::FirstUseEver)
                .build(&ui, || {
                    ui.text("Hello World!");
                    ui.disabled(is_started, || {
                        if ui.button("Generate!") {
                            state.generate_fractal = true;
                        }
                    });

                    ProgressBar::new(state.generation_fraction)
                        .overlay_text(&state.generation_message)
                        .build(&ui);
                });

            ui.main_menu_bar(|| {
                ui.menu("File", || {
                    let exit = MenuItem::new("Exit");
                    if exit.build(&ui) {
                        state.close_requested = true;
                    }
                });
            });
        }

        // store the state variables into self
        self.state = state;

        if self.last_cursor != Some(ui.mouse_cursor()) {
            self.last_cursor = Some(ui.mouse_cursor());
            self.platform.prepare_render(&ui, &self.window);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("UI Render Pass Encoder"),
            });
        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("UI Render Pass"),
                color_attachments: &[RenderPassColorAttachment {
                    view: frame_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            match self
                .renderer
                .render(ui.render(), &self.queue, &self.device, &mut render_pass)
            {
                Ok(_) => {},
                Err(e) => {
                    error!("Error rendering UI: {:?}", e);
                    return;
                },
            }
        }

        self.commands.push(encoder.finish());

        self.queue.submit(self.commands.drain(..));
    }

    fn shutdown(self) {}
}
