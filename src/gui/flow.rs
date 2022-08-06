#![allow(dead_code)]

use crate::{
    gpu::{
        util::{
            backend::{initialize_wgpu, WgpuInitializationError},
            get_desired_limits, print_adapter_info,
        },
        GPUContext, GPUContextType,
    },
    gui::util::get_trace_path,
};
use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};
use tokio::{runtime, runtime::Handle, time::sleep};
use wgpu::{
    DeviceDescriptor, Instance, Maintain, PowerPreference, PresentMode, RequestDeviceError,
    SurfaceConfiguration, SurfaceError, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor,
};
use winit::{
    dpi::PhysicalSize,
    error::OsError,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    window::{Fullscreen, Window, WindowBuilder},
};

/// Signal sent by the application to the Flow to control the application flow.
pub enum FlowSignal {
    RequestRedraw,
    Exit,
    Fullscreen(Option<Fullscreen>),
}

/// Contains data to be used when initializing the FlowModel.
pub struct FlowModelInit {
    pub handle: Handle,
    pub instance: Arc<Instance>,
    pub present: GPUContext,
    pub window: Arc<Window>,
    pub window_size: PhysicalSize<u32>,
    pub frame_format: TextureFormat,
    pub event_loop_proxy: EventLoopProxy<FlowSignal>,
}

/// Represents an application's data, allowing the application to receive
/// lifecycle events. This version of `Flow` and `FlowModel` are designed to
/// support an asynchronous application.
pub trait FlowModel: Sized {
    fn init(init: FlowModelInit) -> Self;

    fn event(&mut self, _event: &WindowEvent<'_>) -> Option<FlowSignal>;

    fn all_events(&mut self, _event: &Event<FlowSignal>) {}

    fn update(&mut self, _update_delta: Duration) -> Option<FlowSignal>;

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

    /// Sets this Flow's window title.
    pub fn title(mut self, title: impl ToString) -> Self {
        self.title = title.to_string();
        self
    }

    /// Sets whether this Flow's window is fullscreen.
    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.fullscreen = fullscreen;
        self
    }

    /// Sets this Flow's window's width.
    pub fn width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    /// Sets this Flow's window's height.
    pub fn height(mut self, height: u32) -> Self {
        self.height = height;
        self
    }

    /// Starts the Flow's event loop.
    pub fn start<Model: FlowModel + 'static>(self) -> Result<!, FlowStartError> {
        info!("Creating runtime...");
        let runtime = runtime::Builder::new_multi_thread().enable_all().build()?;

        info!("Creating event loop...");
        let event_loop = EventLoop::<FlowSignal>::with_user_event();

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

        let (instance, surface, adapter) =
            initialize_wgpu(&window, runtime.handle(), PowerPreference::default())?;

        print_adapter_info(&adapter);

        info!("Requesting device...");
        let limits = get_desired_limits(&adapter);
        let trace_path = runtime.block_on(get_trace_path("present", true))?;
        let (device, queue) = runtime.block_on(adapter.request_device(
            &DeviceDescriptor {
                label: Some("Device"),
                limits: limits.clone(),
                features: Default::default(),
            },
            trace_path.as_ref().map(|p| p.as_path()),
        ))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        info!("Creating device poll task");
        let poll_device = device.clone();
        let status = Arc::new(AtomicBool::new(true));
        let poll_status = status.clone();
        let mut poll_task = Some(runtime.spawn(async move {
            while poll_status.load(Ordering::Acquire) {
                poll_device.poll(Maintain::Poll);
                sleep(Duration::from_millis(50)).await;
            }
        }));

        info!("Configuring surface...");
        let supported_formats = surface.get_supported_formats(&adapter);
        info!("Surface supported formats: {:?}", &supported_formats);

        if !supported_formats.contains(&TextureFormat::Bgra8Unorm) {
            warn!("Your system does not support rendering to a Bgra8Unorm format. Fractals will look a little weird.");
            // FIXME: currently everything writes to the sRGB color space,
            //  however fragment shaders expect their output to be in the
            //  linear-sRGB color space. This means that if we try to render to
            //  a Bgra8UnormSrgb surface, it will attempt to convert
            //  already sRGB colors into sRGB.
        }

        let texture_format = TextureFormat::Bgra8Unorm;
        info!("Using surface format: {:?}", &texture_format);
        let mut config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: texture_format,
            width: window_size.width,
            height: window_size.height,
            present_mode: PresentMode::Fifo,
        };

        surface.configure(&device, &config);

        // setup model
        info!("Creating model...");
        let init = FlowModelInit {
            handle: runtime.handle().clone(),
            instance: instance.clone(),
            present: GPUContext {
                device: device.clone(),
                queue: queue.clone(),
                limits,
                ty: GPUContextType::Presentable,
            },
            window: window.clone(),
            window_size,
            frame_format: config.format,
            event_loop_proxy: event_loop.create_proxy(),
        };
        let mut model: Option<Model> = Some(Model::init(init));
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

                    if let Some(signal) = model.as_mut().unwrap().event(event) {
                        match signal {
                            FlowSignal::RequestRedraw => window.request_redraw(),
                            FlowSignal::Exit => *control = ControlFlow::Exit,
                            FlowSignal::Fullscreen(fullscreen) => {
                                window.set_fullscreen(fullscreen);
                            },
                        }
                    }
                },
                Event::MainEventsCleared => {
                    let now = SystemTime::now();
                    let delta = now.duration_since(previous_update).unwrap();
                    previous_update = now;

                    match model.as_mut().unwrap().update(delta) {
                        None | Some(FlowSignal::RequestRedraw) => window.request_redraw(),
                        Some(FlowSignal::Exit) => *control = ControlFlow::Exit,
                        Some(FlowSignal::Fullscreen(fullscreen)) => {
                            window.set_fullscreen(fullscreen);
                            window.request_redraw();
                        },
                    }
                },
                Event::UserEvent(signal) => match signal {
                    FlowSignal::RequestRedraw => window.request_redraw(),
                    FlowSignal::Exit => *control = ControlFlow::Exit,
                    FlowSignal::Fullscreen(fullscreen) => window.set_fullscreen(fullscreen.clone()),
                },
                Event::RedrawRequested(window_id) if *window_id == window.id() => {
                    let now = SystemTime::now();
                    let delta = now.duration_since(previous_render).unwrap();
                    previous_render = now;

                    let frame = match surface.get_current_texture() {
                        Ok(output) => Some(output),
                        Err(SurfaceError::OutOfMemory) => {
                            error!("Unable to obtain surface frame: OutOfMemory! Exiting...");
                            *control = ControlFlow::Exit;

                            None
                        },
                        Err(_) => None,
                    };

                    if let Some(frame) = frame {
                        let view = frame.texture.create_view(&TextureViewDescriptor::default());

                        model.as_mut().unwrap().render(&view, delta);

                        frame.present();
                    }
                },
                Event::LoopDestroyed => {
                    info!("Shutting down...");

                    let runtime = runtime.take().unwrap();

                    let mut model = model.take().unwrap();
                    model.all_events(&event);
                    model.shutdown();

                    status.store(false, Ordering::Release);
                    if let Err(e) = runtime.block_on(poll_task.take().unwrap()) {
                        error!("Error stopping device poll task: {:?}", e);
                    }

                    // shutdown WGPU
                    drop(queue.take());
                    drop(adapter.take());
                    drop(instance.take());

                    // shutdown the runtime
                    drop(runtime);

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
    WgpuInitializationError(#[from] WgpuInitializationError),
    #[error("Error requesting device")]
    RequestDeviceError(#[from] RequestDeviceError),
}
