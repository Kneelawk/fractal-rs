use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use wgpu::{
    Backends, Device, DeviceDescriptor, Instance, PowerPreference,
    PresentMode, Queue, RequestAdapterOptions, SurfaceConfiguration, SurfaceError,
    TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
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
    model_init: Box<dyn Fn(Arc<Device>, Arc<Queue>, PhysicalSize<u32>, TextureFormat) -> Model>,
    event_callback: Option<Box<dyn Fn(&mut Model, WindowEvent) -> ControlFlow>>,
    update_callback: Option<Box<dyn Fn(&mut Model, Duration) -> ControlFlow>>,
    render_callback: Option<Box<dyn Fn(&mut Model, &TextureView, Duration)>>,

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
    pub fn new<
        F: Fn(Arc<Device>, Arc<Queue>, PhysicalSize<u32>, TextureFormat) -> Model + 'static,
    >(
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
    pub fn event<F: Fn(&mut Model, WindowEvent) -> ControlFlow + 'static>(
        &mut self,
        event_callback: F,
    ) {
        self.event_callback = Some(Box::new(event_callback));
    }

    /// Sets the Flow's update callback.
    pub fn update<F: Fn(&mut Model, Duration) -> ControlFlow + 'static>(
        &mut self,
        update_callback: F,
    ) {
        self.update_callback = Some(Box::new(update_callback));
    }

    /// Sets the Flow's render callback.
    pub fn render<F: Fn(&mut Model, &TextureView, Duration) + 'static>(
        &mut self,
        render_callback: F,
    ) {
        self.render_callback = Some(Box::new(render_callback));
    }

    /// Starts the Flow's event loop.
    pub async fn start(self) -> Result<(), FlowStartError> {
        let event_loop = EventLoop::new();

        let window = {
            let mut builder = WindowBuilder::new().with_title(self.title.clone());

            builder = if self.fullscreen {
                builder.with_fullscreen(Some(Fullscreen::Borderless(None)))
            } else {
                builder.with_inner_size(PhysicalSize::new(self.width, self.height))
            };

            builder.build(&event_loop)?
        };

        // setup wgpu
        let window_size = window.inner_size();

        let instance = Instance::new(Backends::all());

        let surface = unsafe { instance.create_surface(&window) };

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                // We will be doing quite a lot of calculation on the GPU, might as well warn it.
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Error getting adapter");

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("Device"),
                    limits: Default::default(),
                    features: Default::default(),
                },
                None,
            )
            .await
            .expect("Error getting device");

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let mut config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            width: window_size.width,
            height: window_size.height,
            present_mode: PresentMode::Fifo,
        };

        surface.configure(&device, &config);

        // setup model
        let mut model =
            (self.model_init)(device.clone(), queue.clone(), window_size, config.format);
        let mut previous_update = SystemTime::now();
        let mut previous_render = SystemTime::now();

        event_loop.run(move |event, _, control| match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => {
                match event {
                    WindowEvent::Resized(size) => {
                        config.width = size.width;
                        config.height = size.height;
                        surface.configure(&device, &config);
                    },
                    WindowEvent::ScaleFactorChanged {
                        ref new_inner_size, ..
                    } => {
                        config.width = new_inner_size.width;
                        config.height = new_inner_size.height;
                        surface.configure(&device, &config);
                    },
                    _ => {},
                }

                if let Some(event_callback) = &self.event_callback {
                    if event_callback(&mut model, event) == ControlFlow::Exit {
                        *control = ControlFlow::Exit;
                    }
                }
            },
            Event::MainEventsCleared => {
                let now = SystemTime::now();
                let delta = now.duration_since(previous_update).unwrap();
                previous_update = now;

                if let Some(update_callback) = &self.update_callback {
                    if update_callback(&mut model, delta) == ControlFlow::Exit {
                        *control = ControlFlow::Exit;
                    }
                }

                if *control != ControlFlow::Exit {
                    window.request_redraw();
                }
            },
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                let now = SystemTime::now();
                let delta = now.duration_since(previous_render).unwrap();
                previous_render = now;

                if let Some(render_callback) = &self.render_callback {
                    let frame = match surface.get_current_frame() {
                        Ok(output) => Some(output),
                        Err(SurfaceError::OutOfMemory) => {
                            error!("Unable to obtain surface frame: OutOfMemory! Exiting...");
                            *control = ControlFlow::Exit;

                            None
                        },
                        Err(_) => None,
                    };

                    if let Some(frame) = frame {
                        let output = frame.output;
                        let view = output
                            .texture
                            .create_view(&TextureViewDescriptor::default());

                        render_callback(&mut model, &view, delta);
                    }
                }
            },
            _ => {},
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
