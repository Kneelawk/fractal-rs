use std::mem::size_of;
use wgpu::{
    Buffer, BufferAddress, BufferDescriptor, BufferUsage, Device, Extent3d, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsage, TextureView,
};

pub fn create_copy_src_texture(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
    create_texture(
        device,
        width,
        height,
        TextureUsage::COPY_SRC | TextureUsage::RENDER_ATTACHMENT,
    )
}

pub fn create_texture(
    device: &Device,
    width: u32,
    height: u32,
    usage: TextureUsage,
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
        format: TextureFormat::Rgba8Unorm,
        usage,
    });
    let texture_view = texture.create_view(&Default::default());

    (texture, texture_view)
}

pub fn create_texture_buffer(device: &Device, width: u32, height: u32) -> Buffer {
    let size = width * height * size_of::<u32>() as u32;
    let texture_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Framebuffer Buffer"),
        size: size as BufferAddress,
        usage: BufferUsage::COPY_DST | BufferUsage::MAP_READ,
        mapped_at_creation: false,
    });

    texture_buffer
}
