use std::mem::size_of;
use wgpu::{
    Buffer, BufferAddress, BufferDescriptor, BufferUsages, Device, Extent3d, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
};

pub fn create_texture(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
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
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT,
    });
    let texture_view = texture.create_view(&Default::default());

    (texture, texture_view)
}

pub fn create_texture_buffer(device: &Device, width: u32, height: u32) -> Buffer {
    let size = width * height * size_of::<u32>() as u32;
    let texture_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Framebuffer Buffer"),
        size: size as BufferAddress,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    texture_buffer
}
