//! no_multisample.rs - Performs a view generation without any multisampling.

use crate::generator::{
    gpu::{
        buffer::{BufferWrapper, Encodable},
        uniforms::Uniforms,
        util::{create_copy_src_texture, create_texture_buffer},
    },
    util::{copy_region, smallest_multiple_containing},
    view::View,
    PixelBlock, BYTES_PER_PIXEL,
};
use cgmath::Vector2;
use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc::Sender;
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, BufferAddress,
    BufferBinding, BufferUsage, Color, CommandEncoderDescriptor, Device, Extent3d, ImageCopyBuffer,
    ImageCopyTexture, ImageDataLayout, LoadOp, MapMode, Operations, Origin3d, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
};

pub fn generate(
    device: Arc<Device>,
    queue: Arc<Queue>,
    uniform_bind_group_layout: &BindGroupLayout,
    render_pipeline: Arc<RenderPipeline>,
    sender: Sender<Result<PixelBlock, anyhow::Error>>,
    views: Vec<View>,
    spawn_completed: Arc<AtomicUsize>,
) {
    info!("Creating uniform buffer...");
    let mut uniforms_buffer = BufferWrapper::<Uniforms>::new(
        &device,
        Uniforms::size() as BufferAddress,
        BufferUsage::UNIFORM,
    );

    let uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Uniform Bind Group"),
        layout: uniform_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: uniforms_buffer.buffer(),
                offset: 0,
                size: None,
            }),
        }],
    });

    info!("Spawning gpu manager task...");
    tokio::spawn(async move {
        let mut buffers = HashMap::new();

        for view in views {
            let texture_size = (
                smallest_multiple_containing::<usize>(view.image_width, 64),
                smallest_multiple_containing::<usize>(view.image_height, 64),
            );
            let texture_width = texture_size.0 as u32;
            let texture_height = texture_size.1 as u32;
            let (texture, texture_view, buffer) =
                buffers.entry(texture_size).or_insert_with(|| {
                    let width = texture_size.0 as u32;
                    let height = texture_size.1 as u32;
                    info!(
                        "Creating new framebuffer with dimensions ({}x{})...",
                        width, height
                    );
                    let (texture, texture_view) =
                        create_copy_src_texture(&device, width as u32, height as u32);
                    let buffer = create_texture_buffer(&device, width as u32, height as u32);
                    (texture, texture_view, buffer)
                });

            info!(
                "Writing uniforms for ({}, {})...",
                view.image_x, view.image_y
            );
            let cb = uniforms_buffer
                .replace_all(&device, &[Uniforms::new(view, Vector2 { x: 0.0, y: 0.0 })])
                .await
                .unwrap();

            {
                info!(
                    "Encoding render command buffer for ({}, {})...",
                    view.image_x, view.image_y
                );
                let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Render Command Encoder"),
                });

                {
                    let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                        label: Some("Render Pass"),
                        color_attachments: &[RenderPassColorAttachment {
                            view: texture_view,
                            resolve_target: None,
                            ops: Operations {
                                load: LoadOp::Clear(Color {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                }),
                                store: true,
                            },
                        }],
                        depth_stencil_attachment: None,
                    });

                    render_pass.set_pipeline(&render_pipeline);
                    render_pass.set_bind_group(0, &uniform_bind_group, &[]);
                    render_pass.draw(0..6, 0..1);
                }

                encoder.copy_texture_to_buffer(
                    ImageCopyTexture {
                        texture,
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                    },
                    ImageCopyBuffer {
                        buffer,
                        layout: ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(
                                NonZeroU32::new(BYTES_PER_PIXEL as u32 * texture_width).unwrap(),
                            ),
                            rows_per_image: Some(NonZeroU32::new(texture_height).unwrap()),
                        },
                    },
                    Extent3d {
                        width: texture_width,
                        height: texture_height,
                        depth_or_array_layers: 1,
                    },
                );

                info!(
                    "Submitting command buffers for ({}, {})...",
                    view.image_x, view.image_y
                );
                queue.submit([cb, encoder.finish()]);
            }

            let mut image_data = vec![0u8; view.image_width * view.image_height * BYTES_PER_PIXEL];
            {
                info!(
                    "Reading framebuffer for ({}, {})...",
                    view.image_x, view.image_y
                );
                let buffer_slice = buffer.slice(..);
                buffer_slice.map_async(MapMode::Read).await.unwrap();

                let data = buffer_slice.get_mapped_range();

                info!("Copying image for ({}, {})...", view.image_x, view.image_y);
                copy_region(
                    data.as_ref(),
                    texture_width as usize,
                    0,
                    0,
                    &mut image_data,
                    view.image_width,
                    0,
                    0,
                    view.image_width,
                    view.image_height,
                );
            }

            info!(
                "Unmapping buffer for ({}, {})...",
                view.image_x, view.image_y
            );
            buffer.unmap();

            spawn_completed.fetch_add(1, Ordering::Relaxed);

            info!(
                "Sending pixel block for ({}, {})...",
                view.image_x, view.image_y
            );
            sender
                .send(Ok(PixelBlock {
                    view,
                    image: image_data.into_boxed_slice(),
                }))
                .await
                .unwrap();
        }
    });
}
