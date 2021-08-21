use crate::convert::{FromPhysicalSize, FromWindowEvent};
use futures::executor::block_on;
use kpipes_core::{
    messages::{FlowControl, FlowEvent, FrameSize},
};
use std::time::{Duration, SystemTime};
use wgpu::{
    Adapter, BackendBit, CommandBuffer, Device, DeviceDescriptor, Extensions, PowerPreference,
    PresentMode, Queue, RequestAdapterOptions, Surface, SwapChainDescriptor, TextureFormat,
    TextureUsage, TextureView,
};
use winit::{
    dpi::PhysicalSize,
    error::OsError,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, WindowBuilder},
};

/// Used to manage an application's control flow as well as integration with the
/// window manager.
pub struct Flow<Model: 'static> {
    model_init: Box<dyn Fn(&Device, &Queue, FrameSize, TextureFormat) -> Model>,
    event_callback: Option<Box<dyn Fn(&mut Model, &Device, FlowEvent) -> FlowControl>>,
    update_callback: Option<Box<dyn Fn(&mut Model, &Device, Duration) -> FlowControl>>,
    render_callback:
        Option<Box<dyn Fn(&mut Model, &Device, &mut Vec<CommandBuffer>, &TextureView, Duration)>>,

    /// The window's title.
    pub title: String,
    /// Whether the window should be fullscreen.
    pub fullscreen: bool,
    /// The window's width if not fullscreen.
    pub width: u32,
    /// The window's height if not fullscreen.
    pub height: u32,
}

impl<Model: 'static> Flow<Model> {
    /// Creates a new Flow designed to handle a specific kind of model.
    ///
    /// This model is instantiated when the Flow is started.
    pub fn new<F: Fn(&Device, &Queue, FrameSize, TextureFormat) -> Model + 'static>(
        model_init: F,
    ) -> Flow<Model> {
        Flow {
            model_init: Box::new(model_init),
            event_callback: None,
            update_callback: None,
            render_callback: None,
            title: "".to_string(),
            fullscreen: false,
            width: 1280,
            height: 720,
        }
    }

    /// Sets the Flow's window event callback.
    pub fn event<F: Fn(&mut Model, &Device, FlowEvent) -> FlowControl + 'static>(
        &mut self,
        event_callback: F,
    ) {
        self.event_callback = Some(Box::new(event_callback));
    }

    /// Sets the Flow's update callback.
    pub fn update<F: Fn(&mut Model, &Device, Duration) -> FlowControl + 'static>(
        &mut self,
        update_callback: F,
    ) {
        self.update_callback = Some(Box::new(update_callback));
    }

    /// Sets the Flow's render callback.
    pub fn render<
        F: Fn(&mut Model, &Device, &mut Vec<CommandBuffer>, &TextureView, Duration) + 'static,
    >(
        &mut self,
        render_callback: F,
    ) {
        self.render_callback = Some(Box::new(render_callback));
    }

    /// Starts the Flow's event loop.
    pub fn start(self) -> Result<(), FlowStartError> {
        let event_loop = EventLoop::new();
        let mut builder = WindowBuilder::new().with_title(self.title.clone());

        builder = if self.fullscreen {
            builder.with_fullscreen(
                event_loop
                    .available_monitors()
                    .next()
                    .map(|m| Fullscreen::Borderless(m)),
            )
        } else {
            builder.with_inner_size(PhysicalSize::new(self.width, self.height))
        };

        let window = builder.build(&event_loop)?;

        // setup wgpu
        let window_size = window.inner_size();

        let surface = Surface::create(&window);

        let adapter = block_on(Adapter::request(
            &RequestAdapterOptions {
                power_preference: PowerPreference::Default,
                compatible_surface: Some(&surface),
            },
            BackendBit::PRIMARY,
        ))
        .expect("Error getting adapter");

        let (device, queue) = block_on(adapter.request_device(&DeviceDescriptor {
            extensions: Extensions {
                anisotropic_filtering: false,
            },
            limits: Default::default(),
        }));

        let mut sc_desc = SwapChainDescriptor {
            usage: TextureUsage::OUTPUT_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            width: window_size.width,
            height: window_size.height,
            present_mode: PresentMode::Fifo,
        };

        let mut swap_chain = device.create_swap_chain(&surface, &sc_desc);

        let mut commands = vec![];

        // setup model
        let mut model = (self.model_init)(
            &device,
            &queue,
            FrameSize::from_physical_size(window_size),
            sc_desc.format,
        );
        let mut previous_update = SystemTime::now();
        let mut previous_render = SystemTime::now();

        event_loop.run(move |event, _, control| match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => {
                if let Some(event_callback) = &self.event_callback {
                    match event {
                        WindowEvent::Resized(size) => {
                            sc_desc.width = size.width;
                            sc_desc.height = size.height;
                            swap_chain = device.create_swap_chain(&surface, &sc_desc);
                        }
                        WindowEvent::ScaleFactorChanged {
                            ref new_inner_size, ..
                        } => {
                            sc_desc.width = new_inner_size.width;
                            sc_desc.height = new_inner_size.height;
                            swap_chain = device.create_swap_chain(&surface, &sc_desc);
                        }
                        _ => {}
                    }

                    if event_callback(&mut model, &device, FlowEvent::from_window_event(event))
                        == FlowControl::Exit
                    {
                        *control = ControlFlow::Exit;
                    }
                }
            }
            Event::MainEventsCleared => {
                let now = SystemTime::now();
                let delta = now.duration_since(previous_update).unwrap();
                previous_update = now;

                if let Some(update_callback) = &self.update_callback {
                    if update_callback(&mut model, &device, delta) == FlowControl::Exit {
                        *control = ControlFlow::Exit;
                    }
                }

                if *control != ControlFlow::Exit {
                    window.request_redraw();
                }
            }
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                let now = SystemTime::now();
                let delta = now.duration_since(previous_render).unwrap();
                previous_render = now;

                if let Some(render_callback) = &self.render_callback {
                    let frame = swap_chain
                        .get_next_texture()
                        .expect("Error getting render frame");

                    render_callback(&mut model, &device, &mut commands, &frame.view, delta);

                    queue.submit(&commands);
                    commands.clear();
                }
            }
            _ => {}
        });
    }
}

#[derive(Debug)]
pub enum FlowStartError {
    OsError(OsError),
}

impl From<OsError> for FlowStartError {
    fn from(e: OsError) -> Self {
        FlowStartError::OsError(e)
    }
}
