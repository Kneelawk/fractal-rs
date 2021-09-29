use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};
use tokio::{
    runtime,
    runtime::{Handle, Runtime},
    task,
};
use wgpu::{
    Backends, Device, DeviceDescriptor, Instance, Maintain, PowerPreference, PresentMode, Queue,
    RequestAdapterOptions, RequestDeviceError, SurfaceConfiguration, SurfaceError, TextureFormat,
    TextureUsages, TextureView, TextureViewDescriptor,
};
use winit::{
    dpi::PhysicalSize,
    error::OsError,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowBuilder},
};

/// Represents an application's data, allowing the application to receive
/// lifecycle events. This version of `Flow` and `FlowModel` are designed to
/// support an asynchronous application.
pub trait FlowModel: Sized {
    fn init(
        handle: Handle,
        device: Arc<Device>,
        queue: Arc<Queue>,
        window: Arc<Window>,
        frame_format: TextureFormat,
    ) -> Self;

    fn event(&mut self, _event: &WindowEvent<'_>) -> Option<ControlFlow>;

    fn all_events(&mut self, _event: &Event<()>) {}

    fn update(&mut self, _update_delta: Duration) -> Option<ControlFlow>;

    fn render(&mut self, _frame_view: &TextureView, _render_delta: Duration);

    fn shutdown(self);
}

/// Used to manage an application's control flow as well as integration with the
/// window manager. This version of `Flow` and `FlowModel` are designed to
/// support an asynchronous application.
pub struct Flow {
    /// The window's title.
    pub title: String,
    /// Whether the window should be fullscreen.
    pub fullscreen: bool,
    /// The window's width if not fullscreen.
    pub width: u32,
    /// The window's height if not fullscreen.
    pub height: u32,
}

impl Flow {
    /// Creates a new Flow designed to handle a specific kind of model.
    ///
    /// This model is instantiated when the Flow is started.
    pub fn new() -> Flow {
        Flow {
            title: "".to_string(),
            fullscreen: false,
            width: 1280,
            height: 720,
        }
    }

    /// Starts the Flow's event loop.
    pub fn start<Model: FlowModel + 'static>(self) -> Result<!, FlowStartError> {
        info!("Creating runtime...");
        let runtime = runtime::Builder::new_multi_thread().enable_all().build()?;

        info!("Creating event loop...");
        let event_loop = EventLoop::new();

        info!("Creating window...");
        let window = {
            let mut builder = WindowBuilder::new().with_title(self.title.clone());

            builder = if self.fullscreen {
                builder.with_fullscreen(Some(Fullscreen::Borderless(None)))
            } else {
                builder.with_inner_size(PhysicalSize::new(self.width, self.height))
            };

            builder.build(&event_loop)?
        };

        let window = Arc::new(window);

        // setup wgpu
        let window_size = window.inner_size();

        info!("Creating instance...");
        let instance = Instance::new(Backends::PRIMARY);

        info!("Creating surface...");
        let surface = unsafe { instance.create_surface(window.as_ref()) };

        info!("Requesting adapter...");
        let adapter = runtime
            .block_on(instance.request_adapter(&RequestAdapterOptions {
                // We will be doing quite a lot of calculation on the GPU, might as well warn it.
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
            }))
            .ok_or(FlowStartError::AdapterRequestError)?;

        info!("Requesting device...");
        let (device, queue) = runtime.block_on(adapter.request_device(
            &DeviceDescriptor {
                label: Some("Device"),
                limits: Default::default(),
                features: Default::default(),
            },
            None,
        ))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        info!("Creating device poll task");
        let poll_device = device.clone();
        let status = Arc::new(AtomicBool::new(true));
        let poll_status = status.clone();
        let mut poll_task = Some(runtime.spawn(async move {
            while poll_status.load(Ordering::Relaxed) {
                poll_device.poll(Maintain::Poll);
                task::yield_now().await;
            }
        }));

        info!("Configuring surface...");
        let mut config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface
                .get_preferred_format(&adapter)
                .unwrap_or(TextureFormat::Bgra8Unorm),
            width: window_size.width,
            height: window_size.height,
            present_mode: PresentMode::Fifo,
        };

        surface.configure(&device, &config);

        // setup model
        info!("Creating model...");
        let mut model: Option<Model> = Some(Model::init(
            runtime.handle().clone(),
            device.clone(),
            queue.clone(),
            window.clone(),
            config.format,
        ));
        let mut previous_update = SystemTime::now();
        let mut previous_render = SystemTime::now();

        let mut runtime = Some(runtime);

        let mut instance = Some(instance);
        let mut adapter = Some(adapter);
        let mut queue = Some(queue);

        info!("Starting event loop...");
        event_loop.run(move |event, _, control| {
            match &event {
                Event::WindowEvent { event, window_id } if *window_id == window.id() => {
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
                        WindowEvent::CloseRequested => {
                            *control = ControlFlow::Exit;
                        },
                        _ => {},
                    }

                    if let Some(new_control) = model.as_mut().unwrap().event(event) {
                        *control = new_control;
                    }
                },
                Event::MainEventsCleared => {
                    let now = SystemTime::now();
                    let delta = now.duration_since(previous_update).unwrap();
                    previous_update = now;

                    if let Some(new_control) = model.as_mut().unwrap().update(delta) {
                        *control = new_control;
                    }

                    if *control != ControlFlow::Exit {
                        window.request_redraw();
                    }
                },
                Event::RedrawRequested(window_id) if *window_id == window.id() => {
                    let now = SystemTime::now();
                    let delta = now.duration_since(previous_render).unwrap();
                    previous_render = now;

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

                        model.as_mut().unwrap().render(&view, delta);
                    }
                },
                Event::LoopDestroyed => {
                    info!("Shutting down...");

                    let mut model = model.take().unwrap();
                    model.all_events(&event);
                    model.shutdown();

                    status.store(false, Ordering::Relaxed);
                    if let Err(e) = runtime
                        .as_ref()
                        .unwrap()
                        .block_on(poll_task.take().unwrap())
                    {
                        error!("Error stopping device poll task: {:?}", e);
                    }

                    // shutdown WGPU
                    drop(queue.take());
                    drop(adapter.take());
                    drop(instance.take());

                    // shutdown the runtime
                    drop(runtime.take());

                    info!("Done.");
                },
                _ => {},
            }

            if let Some(model) = &mut model {
                model.all_events(&event);
            }
        });
    }
}

#[derive(Error, Debug)]
pub enum FlowStartError {
    #[error("IO error")]
    IOError(#[from] io::Error),
    #[error("Window Builder error")]
    OsError(#[from] OsError),
    #[error("Error requesting adapter")]
    AdapterRequestError,
    #[error("Error requesting device")]
    RequestDeviceError(#[from] RequestDeviceError),
}
