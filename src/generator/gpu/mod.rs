use crate::generator::{
    gpu::{
        buffer::{BufferWrapper, Encodable},
        shader::load_shaders,
        uniforms::{GpuView, Uniforms},
        util::{create_texture, create_texture_buffer},
    },
    util::{copy_region, smallest_multiple_containing},
    view::View,
    FractalGenerator, FractalGeneratorInstance, FractalOpts, PixelBlock, BYTES_PER_PIXEL,
};
use futures::{
    future::{ready, BoxFuture},
    FutureExt,
};
use std::{
    collections::HashMap,
    num::{NonZeroU32, NonZeroU64},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc::Sender;
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, BufferAddress, BufferBinding,
    BufferBindingType, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandEncoderDescriptor, Device, Extent3d, Face, FragmentState, FrontFace, ImageCopyBuffer,
    ImageCopyTexture, ImageDataLayout, LoadOp, MapMode, MultisampleState, Operations, Origin3d,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    ShaderModuleDescriptor, ShaderStages, TextureFormat, VertexState,
};

mod buffer;
mod shader;
mod uniforms;
mod util;

pub struct GpuFractalGenerator {
    opts: FractalOpts,
    device: Arc<Device>,
    queue: Arc<Queue>,
    uniform_bind_group_layout: BindGroupLayout,
    render_pipeline: Arc<RenderPipeline>,
}

impl GpuFractalGenerator {
    pub async fn new(
        opts: FractalOpts,
        device: Arc<Device>,
        queue: Arc<Queue>,
    ) -> anyhow::Result<GpuFractalGenerator> {
        info!("Creating shader module...");
        let shader = load_shaders(opts).await?;
        let module = device.create_shader_module(&ShaderModuleDescriptor {
            label: Some("Vertex Shader"),
            source: shader,
        });

        info!("Creating uniform bind group layout...");
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Uniform Bind Group Layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(NonZeroU64::new(Uniforms::size() as u64).unwrap()),
                    },
                    count: None,
                }],
            });

        info!("Creating render pipeline...");
        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = Arc::new(device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &module,
                entry_point: "vert_main",
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module: &module,
                entry_point: "frag_main",
                targets: &[ColorTargetState {
                    format: TextureFormat::Rgba8Unorm,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                }],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                clamp_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        }));

        Ok(GpuFractalGenerator {
            opts,
            device,
            queue,
            uniform_bind_group_layout,
            render_pipeline,
        })
    }
}

impl FractalGenerator for GpuFractalGenerator {
    fn min_views_hint(&self) -> BoxFuture<anyhow::Result<usize>> {
        ready(Ok(1)).boxed()
    }

    fn start_generation(
        &self,
        views: &[View],
        sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> BoxFuture<anyhow::Result<Box<dyn FractalGeneratorInstance>>> {
        let views = views.to_vec();
        async move {
            let boxed: Box<dyn FractalGeneratorInstance> =
                Box::new(GpuFractalGeneratorInstance::start(
                    self.opts,
                    self.device.clone(),
                    self.queue.clone(),
                    &self.uniform_bind_group_layout,
                    self.render_pipeline.clone(),
                    views,
                    sender,
                ));
            Ok(boxed)
        }
        .boxed()
    }
}

struct GpuFractalGeneratorInstance {
    view_count: usize,
    completed: Arc<AtomicUsize>,
}

impl GpuFractalGeneratorInstance {
    fn start(
        _opts: FractalOpts,
        device: Arc<Device>,
        queue: Arc<Queue>,
        uniform_bind_group_layout: &BindGroupLayout,
        render_pipeline: Arc<RenderPipeline>,
        views: Vec<View>,
        sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> GpuFractalGeneratorInstance {
        let view_count = views.len();
        let completed = Arc::new(AtomicUsize::new(0));
        let spawn_completed = completed.clone();

        info!("Creating uniform buffer...");
        let mut uniforms_buffer = BufferWrapper::<Uniforms>::new(
            &device,
            Uniforms::size() as BufferAddress,
            BufferUsages::UNIFORM,
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
                            create_texture(&device, width as u32, height as u32);
                        let buffer = create_texture_buffer(&device, width as u32, height as u32);
                        (texture, texture_view, buffer)
                    });

                info!(
                    "Writing uniforms for ({}, {})...",
                    view.image_x, view.image_y
                );
                let cb = uniforms_buffer
                    .replace_all(
                        &device,
                        &[Uniforms {
                            view: GpuView::from_view(view),
                        }],
                    )
                    .await
                    .unwrap();

                {
                    info!(
                        "Encoding render command buffer for ({}, {})...",
                        view.image_x, view.image_y
                    );
                    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                        label: Some(&format!(
                            "Render Command Encoder for ({}, {})",
                            view.image_x, view.image_y
                        )),
                    });

                    {
                        let render_pass_label =
                            format!("Render Pass for ({}, {})", view.image_x, view.image_y);
                        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                            label: Some(&render_pass_label),
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
                            aspect: Default::default(),
                        },
                        ImageCopyBuffer {
                            buffer,
                            layout: ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(
                                    NonZeroU32::new(BYTES_PER_PIXEL as u32 * texture_width)
                                        .unwrap(),
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

                let mut image_data =
                    vec![0u8; view.image_width * view.image_height * BYTES_PER_PIXEL];
                {
                    info!(
                        "Reading framebuffer for ({}, {})...",
                        view.image_x, view.image_y
                    );
                    info!("Getting buffer slice...");
                    let buffer_slice = buffer.slice(..);
                    info!("Mapping buffer...");
                    buffer_slice.map_async(MapMode::Read).await.unwrap();

                    info!("Getting buffer mapped range...");
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

        GpuFractalGeneratorInstance {
            view_count,
            completed,
        }
    }
}

impl FractalGeneratorInstance for GpuFractalGeneratorInstance {
    fn progress(&self) -> BoxFuture<anyhow::Result<f32>> {
        ready(Ok(
            self.completed.load(Ordering::Relaxed) as f32 / self.view_count as f32
        ))
        .boxed()
    }

    fn running(&self) -> BoxFuture<anyhow::Result<bool>> {
        ready(Ok(self.completed.load(Ordering::Relaxed) < self.view_count)).boxed()
    }
}
