use crate::generator::{
    gpu::{
        buffer::{BufferWrapper, Encodable},
        uniforms::Uniforms,
        util::{create_copy_src_texture, create_texture, create_texture_buffer},
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
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource,
    Buffer, BufferAddress, BufferBinding, BufferUsage, Color, CommandEncoderDescriptor, Device,
    Extent3d, FilterMode, ImageCopyBuffer, ImageCopyTexture, ImageDataLayout, LoadOp, MapMode,
    Operations, Origin3d, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    Sampler, SamplerDescriptor, Texture, TextureUsage, TextureView,
};

pub fn generate(
    device: Arc<Device>,
    queue: Arc<Queue>,
    uniform_bind_group_layout: &BindGroupLayout,
    render_pipeline: Arc<RenderPipeline>,
    multisample_bind_group_layout: Arc<BindGroupLayout>,
    multisample_pipeline: Arc<RenderPipeline>,
    sender: Sender<Result<PixelBlock, anyhow::Error>>,
    views: Vec<View>,
    spawn_completed: Arc<AtomicUsize>,
    offset: f32,
) {
    let offsets = vec![
        Vector2 {
            x: -offset,
            y: -offset,
        },
        Vector2 {
            x: offset,
            y: -offset,
        },
        Vector2 {
            x: -offset,
            y: offset,
        },
        Vector2 {
            x: offset,
            y: offset,
        },
    ];

    info!("Creating uniform buffers...");
    let uniform_size = Uniforms::size() as BufferAddress;
    let mut uniform_buffers = vec![];
    for _ in 0..4 {
        uniform_buffers.push(BufferWrapper::new(
            &device,
            uniform_size * 4,
            BufferUsage::UNIFORM,
        ));
    }

    let mut uniform_bind_groups = vec![];
    for i in 0..4 {
        uniform_bind_groups.push(device.create_bind_group(&BindGroupDescriptor {
            label: Some(&format!("Uniforms Bind Group {}", i)),
            layout: uniform_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: uniform_buffers[i].buffer(),
                    offset: 0,
                    size: None,
                }),
            }],
        }));
    }

    info!("Creating sampler...");
    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("Multisample Sampler"),
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        mipmap_filter: FilterMode::Nearest,
        ..Default::default()
    });

    info!("Spawning multisample gpu manager task...");
    tokio::spawn(async move {
        let mut buffers = HashMap::new();

        for view in views {
            let texture_size = (
                smallest_multiple_containing::<usize>(view.image_width, 64),
                smallest_multiple_containing::<usize>(view.image_height, 64),
            );
            let texture_width = texture_size.0 as u32;
            let texture_height = texture_size.1 as u32;
            let texture_buffers = buffers.entry(texture_size).or_insert_with(|| {
                MultisampleFramebuffer::new(
                    texture_width,
                    texture_height,
                    4,
                    &device,
                    &multisample_bind_group_layout,
                    &sampler,
                )
            });

            info!(
                "Writing uniforms buffer for ({}, {})...",
                view.image_x, view.image_y
            );
            let mut command_buffers = vec![];
            for i in 0..4 {
                command_buffers.push(
                    uniform_buffers[i]
                        .replace_all(&device, &[Uniforms::new(view, offsets[i])])
                        .await
                        .unwrap(),
                );
            }

            {
                info!(
                    "Encoding multisample render command buffer for ({}, {})...",
                    view.image_x, view.image_y
                );
                let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Multisample Command Encoder"),
                });

                for i in 0..4 {
                    let render_pass_name = format!("Sample Render Pass {}", i);
                    let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                        label: Some(&render_pass_name),
                        color_attachments: &[RenderPassColorAttachment {
                            view: &texture_buffers.sample_textures[i].1,
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
                    render_pass.set_bind_group(0, &uniform_bind_groups[i], &[]);
                    render_pass.draw(0..6, 0..1);
                }

                {
                    let mut multisample_render_pass =
                        encoder.begin_render_pass(&RenderPassDescriptor {
                            label: Some("Multisample Render Pass"),
                            color_attachments: &[RenderPassColorAttachment {
                                view: &texture_buffers.final_texture_view,
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

                    multisample_render_pass.set_pipeline(&multisample_pipeline);
                    multisample_render_pass.set_bind_group(
                        0,
                        &texture_buffers.samples_bind_group,
                        &[],
                    );
                    multisample_render_pass.draw(0..6, 0..1);
                }

                encoder.copy_texture_to_buffer(
                    ImageCopyTexture {
                        texture: &texture_buffers.final_texture,
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                    },
                    ImageCopyBuffer {
                        buffer: &texture_buffers.final_buffer,
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
                    "Submitting multisample render command buffer for ({}, {})...",
                    view.image_x, view.image_y
                );
                command_buffers.push(encoder.finish());
                queue.submit(command_buffers);
            }

            let mut image_data = vec![0u8; view.image_width * view.image_height * BYTES_PER_PIXEL];
            {
                info!(
                    "Reading framebuffer for ({}, {})...",
                    view.image_x, view.image_y
                );
                let buffer_slice = texture_buffers.final_buffer.slice(..);
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
            texture_buffers.final_buffer.unmap();

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

struct MultisampleFramebuffer {
    sample_textures: Vec<(Texture, TextureView)>,
    samples_bind_group: BindGroup,
    final_texture: Texture,
    final_texture_view: TextureView,
    final_buffer: Buffer,
}

impl MultisampleFramebuffer {
    fn new(
        width: u32,
        height: u32,
        samples: u32,
        device: &Device,
        layout: &BindGroupLayout,
        sampler: &Sampler,
    ) -> MultisampleFramebuffer {
        let width = width as u32;
        let height = height as u32;

        info!(
            "Creating new {}-sample framebuffer with dimensions ({}x{})...",
            samples, width, height
        );
        let mut sample_textures = vec![];
        let mut bind_group_entries = vec![BindGroupEntry {
            binding: 0,
            resource: BindingResource::Sampler(sampler),
        }];

        for _ in 0..samples {
            let (texture, texture_view) = create_texture(
                &device,
                width,
                height,
                TextureUsage::RENDER_ATTACHMENT | TextureUsage::SAMPLED,
            );
            sample_textures.push((texture, texture_view));
        }

        for i in 0..samples {
            bind_group_entries.push(BindGroupEntry {
                binding: i + 1,
                resource: BindingResource::TextureView(&sample_textures[i as usize].1),
            });
        }

        let samples_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Multisample Samples Bind Group"),
            layout,
            entries: &bind_group_entries,
        });

        let (final_texture, final_texture_view) = create_copy_src_texture(&device, width, height);
        let final_buffer = create_texture_buffer(&device, width, height);

        MultisampleFramebuffer {
            sample_textures,
            samples_bind_group,
            final_texture,
            final_texture_view,
            final_buffer,
        }
    }
}
