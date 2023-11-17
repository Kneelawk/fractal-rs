use std::sync::Arc;
use tokio::runtime::Handle;
use wgpu::{
    Adapter, Backends, CreateSurfaceError, Instance, InstanceDescriptor, PowerPreference,
    RequestAdapterOptions, Surface,
};
use winit::window::Window;

pub fn initialize_wgpu(
    window: &Window,
    handle: &Handle,
    power_preference: PowerPreference,
) -> Result<(Arc<Instance>, Surface, Adapter), WgpuInitializationError> {
    let backend = if cfg!(feature = "prefer-dx12") {
        info!("Preferred backend: dx12");
        Backends::DX12
    } else if cfg!(feature = "prefer-metal") {
        info!("Preferred backend: metal");
        Backends::METAL
    } else if cfg!(feature = "prefer-vulkan") {
        info!("Preferred backend: vulkan");
        Backends::VULKAN
    } else {
        info!("No preferred backend, using primary backend.");
        Backends::PRIMARY
    };

    info!("Creating instance...");
    let instance = Arc::new(Instance::new(InstanceDescriptor {
        backends: backend,
        ..Default::default()
    }));

    info!("Creating surface...");
    let surface = unsafe { instance.create_surface(window) }?;

    info!("Requesting adapter...");
    let adapter = handle.block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }));

    if adapter.is_none() {
        warn!("Unable to use preferred backend, attempting to use system default.");

        info!("Creating instance...");
        let instance = Arc::new(Instance::new(InstanceDescriptor {
            backends: Backends::PRIMARY,
            ..Default::default()
        }));

        info!("Creating surface...");
        let surface = unsafe { instance.create_surface(window) }?;

        info!("Requesting adapter...");
        let adapter = handle
            .block_on(instance.request_adapter(&RequestAdapterOptions {
                power_preference,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            }))
            .ok_or(WgpuInitializationError::AdapterUnavailable)?;

        Ok((instance, surface, adapter))
    } else {
        Ok((instance, surface, adapter.unwrap()))
    }
}

#[derive(Debug, Error)]
pub enum WgpuInitializationError {
    #[error("Unable to obtain an adapter")]
    AdapterUnavailable,
    #[error("Unable to create surface")]
    CreateSurfaceError(#[from] CreateSurfaceError),
}
