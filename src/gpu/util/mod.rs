pub mod backend;

use std::mem::size_of;
use wgpu::{
    Adapter, Buffer, BufferAddress, BufferDescriptor, BufferUsages, Device, Extent3d, Features,
    Limits, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureView,
};

pub fn create_texture(
    device: &Device,
    width: u32,
    height: u32,
    format: TextureFormat,
    usage: TextureUsages,
) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("Framebuffer"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage,
    });
    let texture_view = texture.create_view(&Default::default());

    (texture, texture_view)
}

pub fn create_texture_buffer(
    device: &Device,
    width: u32,
    height: u32,
    usage: BufferUsages,
) -> Buffer {
    let size = width * height * size_of::<u32>() as u32;
    let texture_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Framebuffer Buffer"),
        size: size as BufferAddress,
        usage,
        mapped_at_creation: false,
    });

    texture_buffer
}

pub fn print_adapter_info(adapter: &Adapter) {
    let info = adapter.get_info();
    let limits = adapter.limits();
    let features = adapter.features();

    info!(
        r"Adapter Info:
    Name: {name}
    Backend: {backend:?}
    Device Type: {device_type:?}
    PCI: {vendor:04x}:{device:04x}
    Limits:
        Max Texture Dimension 1D: {dimension_1d}
        Max Texture Dimension 2D: {dimension_2d}
        Max Texture Dimension 3D: {dimension_3d}
        Max Uniform Buffer Size: {uniform_size}
        Max Storage Buffer Size: {storage_size}
        Max Push Constant Size: {push_constant_size}
    Features:
        Buffer Binding Array: .... {buffer_binding_array}
        Clear Commands: .......... {clear_commands}
        Mappable Primary Buffers:  {mappable_primary_buffers}
        Multi Draw Indirect: ..... {multi_draw_indirect}
        Multi Draw Indirect Count: {multi_draw_indirect_count}
        Pipeline Statistics Query: {pipeline_statistics_query}
        Push Constants: .......... {push_constants}
        Shader Float64: .......... {shader_float64}
        Texture Binding Array: ... {texture_binding_array}",
        name = &info.name,
        backend = &info.backend,
        device_type = &info.device_type,
        vendor = info.vendor,
        device = info.device,
        dimension_1d = limits.max_texture_dimension_1d,
        dimension_2d = limits.max_texture_dimension_2d,
        dimension_3d = limits.max_texture_dimension_3d,
        uniform_size = limits.max_uniform_buffer_binding_size,
        storage_size = limits.max_storage_buffer_binding_size,
        push_constant_size = limits.max_push_constant_size,
        buffer_binding_array = features.contains(Features::BUFFER_BINDING_ARRAY),
        clear_commands = features.contains(Features::CLEAR_COMMANDS),
        mappable_primary_buffers = features.contains(Features::MAPPABLE_PRIMARY_BUFFERS),
        multi_draw_indirect = features.contains(Features::MULTI_DRAW_INDIRECT),
        multi_draw_indirect_count = features.contains(Features::MULTI_DRAW_INDIRECT_COUNT),
        pipeline_statistics_query = features.contains(Features::PIPELINE_STATISTICS_QUERY),
        push_constants = features.contains(Features::PUSH_CONSTANTS),
        shader_float64 = features.contains(Features::SHADER_FLOAT64),
        texture_binding_array = features.contains(Features::TEXTURE_BINDING_ARRAY)
    );
}

pub fn get_desired_limits(adapter: &Adapter) -> Limits {
    let limits = adapter.limits();
    Limits {
        max_texture_dimension_2d: limits.max_texture_dimension_2d,
        max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
        ..Default::default()
    }
}
