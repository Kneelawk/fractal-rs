//! gui/mod.rs - This is where the GUI-based core application logic happens.

use crate::gui::flow::{Flow, FlowModel};
use imgui::{Condition, FontConfig, FontSource, MenuItem, MouseCursor};
use imgui_wgpu::{Renderer, RendererConfig};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::{sync::Arc, time::Duration};
use tokio::runtime::Runtime;
use wgpu::{
    Color, CommandEncoderDescriptor, Device, LoadOp, Operations, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, TextureFormat, TextureView,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    window::Window,
};

mod flow;
mod viewer;

/// Launches the application as a GUI application.
pub fn start_gui_application() -> ! {
    let mut flow = Flow::new();
    flow.title = "Fractal-RS 2".to_string();

    flow.start::<FractalRSGuiMain>()
        .expect("Error starting Flow!")
}

struct FractalRSGuiMain {
    // initialization state
    runtime: Arc<Runtime>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    window: Arc<Window>,
    imgui: imgui::Context,
    platform: WinitPlatform,
    renderer: Renderer,

    // running state
    last_cursor: Option<Option<MouseCursor>>,
    state: UIState,
}

#[derive(Clone)]
struct UIState {
    close_requested: bool,
}

impl FlowModel for FractalRSGuiMain {
    fn init(
        runtime: Arc<Runtime>,
        device: Arc<Device>,
        queue: Arc<Queue>,
        window: Arc<Window>,
        frame_format: TextureFormat,
    ) -> Self {
        info!("Setting up UI...");

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

        FractalRSGuiMain {
            runtime,
            device,
            queue,
            window,
            imgui,
            platform,
            renderer,
            last_cursor: None,
            state: UIState {
                close_requested: false,
            },
        }
    }

    fn event(&mut self, event: &WindowEvent<'_>) -> Option<ControlFlow> {
        None
    }

    fn all_events(&mut self, event: &Event<()>) {
        self.platform
            .handle_event(self.imgui.io_mut(), &self.window, event);
    }

    fn update(&mut self, update_delta: Duration) -> Option<ControlFlow> {
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
            let window = imgui::Window::new("Hello World!");
            window
                .size([400.0, 500.0], Condition::FirstUseEver)
                .build(&ui, || {
                    ui.text("Hello World!");
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

        self.queue.submit([encoder.finish()]);
    }

    fn shutdown(self) {}
}
