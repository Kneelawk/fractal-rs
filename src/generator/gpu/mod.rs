use crate::{
    generator::{
        gpu::{
            shader::load_shaders,
            uniforms::{GpuView, Uniforms},
        },
        util::{copy_region, smallest_multiple_containing},
        view::View,
        FractalGenerator, FractalGeneratorInstance, FractalOpts, PixelBlock, BYTES_PER_PIXEL,
    },
    gpu::{
        buffer::{BufferWrapper, Encodable},
        util::{create_texture, create_texture_buffer},
    },
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
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferAddress,
    BufferBinding, BufferBindingType, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandBuffer, CommandEncoder, CommandEncoderDescriptor, Device, Extent3d, Face, FragmentState,
    FrontFace, ImageCopyBuffer, ImageCopyTexture, ImageDataLayout, LoadOp, MapMode,
    MultisampleState, Operations, Origin3d, PipelineLayoutDescriptor, PolygonMode, PrimitiveState,
    PrimitiveTopology, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderStages, Texture, TextureAspect,
    TextureFormat, TextureUsages, TextureView, VertexState,
};

mod shader;
mod uniforms;

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

#[async_trait]
impl FractalGenerator for GpuFractalGenerator {
    async fn min_views_hint(&self) -> anyhow::Result<usize> {
        Ok(1)
    }

    async fn start_generation_to_cpu(
        &self,
        views: &[View],
        sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> anyhow::Result<Box<dyn FractalGeneratorInstance>> {
        Ok(Box::new(GpuFractalGeneratorInstance::start_to_cpu(
            self.opts,
            self.device.clone(),
            self.queue.clone(),
            &self.uniform_bind_group_layout,
            self.render_pipeline.clone(),
            views.to_vec(),
            sender,
        )))
    }

    async fn start_generation_to_gpu(
        &self,
        views: &[View],
        texture: Arc<Texture>,
        texture_view: Arc<TextureView>,
    ) -> anyhow::Result<Box<dyn FractalGeneratorInstance>> {
        Ok(Box::new(GpuFractalGeneratorInstance::start_to_gpu(
            self.opts,
            self.device.clone(),
            self.queue.clone(),
            &self.uniform_bind_group_layout,
            self.render_pipeline.clone(),
            views.to_vec(),
            texture,
            texture_view,
        )))
    }
}

struct GpuFractalGeneratorInstance {
    view_count: usize,
    completed: Arc<AtomicUsize>,
}

impl GpuFractalGeneratorInstance {
    fn start_to_cpu(
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

        let (mut uniforms_buffer, uniform_bind_group) =
            setup_uniforms(&device, uniform_bind_group_layout);

        info!("Spawning gpu manager task...");

        tokio::spawn(async move {
            let mut buffers = HashMap::new();

            for view in views {
                let (texture_width, texture_height, texture, texture_view, buffer) =
                    find_texture_buffer_for_view(&device, &mut buffers, view);

                let uniforms_cb = write_uniforms(&device, &mut uniforms_buffer, view).await;

                {
                    info!(
                        "Encoding render command buffer for ({}, {})...",
                        view.image_x, view.image_y
                    );
                    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("Render Command Encoder"),
                    });

                    encode_render_pass(
                        &render_pipeline,
                        &uniform_bind_group,
                        texture_view,
                        &mut encoder,
                    );

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
                    queue.submit([uniforms_cb, encoder.finish()]);
                }

                let mut image_data =
                    vec![0u8; view.image_width * view.image_height * BYTES_PER_PIXEL];
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

        GpuFractalGeneratorInstance {
            view_count,
            completed,
        }
    }

    fn start_to_gpu(
        _opts: FractalOpts,
        device: Arc<Device>,
        queue: Arc<Queue>,
        uniform_bind_group_layout: &BindGroupLayout,
        render_pipeline: Arc<RenderPipeline>,
        views: Vec<View>,
        out_texture: Arc<Texture>,
        out_texture_view: Arc<TextureView>,
    ) -> GpuFractalGeneratorInstance {
        let view_count = views.len();
        let completed = Arc::new(AtomicUsize::new(0));
        let spawn_completed = completed.clone();

        let (uniforms_buffer, uniform_bind_group) =
            setup_uniforms(&device, uniform_bind_group_layout);

        info!("Spawning gpu manager task...");

        tokio::spawn(async move {
            if view_count == 1 {
                GpuFractalGeneratorInstance::start_to_gpu_single_view(
                    _opts,
                    device,
                    queue,
                    render_pipeline,
                    views.into_iter().next().unwrap(),
                    uniforms_buffer,
                    uniform_bind_group,
                    out_texture_view,
                )
                .await;

                spawn_completed.fetch_add(1, Ordering::Relaxed);
            } else {
                GpuFractalGeneratorInstance::start_to_gpu_multi_view(
                    _opts,
                    device,
                    queue,
                    render_pipeline,
                    views,
                    uniforms_buffer,
                    uniform_bind_group,
                    out_texture,
                    spawn_completed,
                )
                .await;
            }
        });

        GpuFractalGeneratorInstance {
            view_count,
            completed,
        }
    }

    async fn start_to_gpu_single_view(
        _opts: FractalOpts,
        device: Arc<Device>,
        queue: Arc<Queue>,
        render_pipeline: Arc<RenderPipeline>,
        view: View,
        mut uniforms_buffer: BufferWrapper<Uniforms>,
        uniform_bind_group: BindGroup,
        out_texture_view: Arc<TextureView>,
    ) {
        let uniforms_cb = write_uniforms(&device, &mut uniforms_buffer, view).await;

        {
            info!(
                "Encoding render command buffer for ({}, {})...",
                view.image_x, view.image_y
            );
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render Command Encoder"),
            });

            encode_render_pass(
                &render_pipeline,
                &uniform_bind_group,
                &out_texture_view,
                &mut encoder,
            );

            info!(
                "Submitting command buffers for ({}, {})...",
                view.image_x, view.image_y
            );
            queue.submit([uniforms_cb, encoder.finish()]);
        }
    }

    async fn start_to_gpu_multi_view(
        _opts: FractalOpts,
        device: Arc<Device>,
        queue: Arc<Queue>,
        render_pipeline: Arc<RenderPipeline>,
        views: Vec<View>,
        mut uniforms_buffer: BufferWrapper<Uniforms>,
        uniform_bind_group: BindGroup,
        out_texture: Arc<Texture>,
        spawn_completed: Arc<AtomicUsize>,
    ) {
        let mut buffers = HashMap::new();

        for view in views {
            let (texture, texture_view) = find_texture_for_view(&device, &mut buffers, view);

            let uniforms_cb = write_uniforms(&device, &mut uniforms_buffer, view).await;

            {
                info!(
                    "Encoding render command buffer for ({}, {})...",
                    view.image_x, view.image_y
                );
                let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Render Command Encoder"),
                });

                encode_render_pass(
                    &render_pipeline,
                    &uniform_bind_group,
                    texture_view,
                    &mut encoder,
                );

                encoder.copy_texture_to_texture(
                    ImageCopyTexture {
                        texture,
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                        aspect: TextureAspect::All,
                    },
                    ImageCopyTexture {
                        texture: &out_texture,
                        mip_level: 0,
                        origin: Origin3d {
                            x: view.image_x as u32,
                            y: view.image_y as u32,
                            z: 0,
                        },
                        aspect: TextureAspect::All,
                    },
                    Extent3d {
                        width: view.image_width as u32,
                        height: view.image_height as u32,
                        depth_or_array_layers: 0,
                    },
                );

                queue.submit([uniforms_cb, encoder.finish()]);
            }

            spawn_completed.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn setup_uniforms(
    device: &Device,
    uniform_bind_group_layout: &BindGroupLayout,
) -> (BufferWrapper<Uniforms>, BindGroup) {
    info!("Creating uniform buffer...");
    let uniforms_buffer = BufferWrapper::<Uniforms>::new(
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
    (uniforms_buffer, uniform_bind_group)
}

fn find_texture_buffer_for_view<'a>(
    device: &Device,
    buffers: &'a mut HashMap<(usize, usize), (Texture, TextureView, Buffer)>,
    view: View,
) -> (
    u32,
    u32,
    &'a mut Texture,
    &'a mut TextureView,
    &'a mut Buffer,
) {
    let texture_size = (
        smallest_multiple_containing::<usize>(view.image_width, 64),
        smallest_multiple_containing::<usize>(view.image_height, 64),
    );
    let texture_width = texture_size.0 as u32;
    let texture_height = texture_size.1 as u32;
    let (texture, texture_view, buffer) = buffers.entry(texture_size).or_insert_with(|| {
        let width = texture_size.0 as u32;
        let height = texture_size.1 as u32;
        info!(
            "Creating new framebuffer with dimensions ({}x{})...",
            width, height
        );
        let (texture, texture_view) = create_texture(
            &device,
            width as u32,
            height as u32,
            TextureFormat::Rgba8Unorm,
            TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT,
        );
        let buffer = create_texture_buffer(
            &device,
            width as u32,
            height as u32,
            BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        );
        (texture, texture_view, buffer)
    });
    (texture_width, texture_height, texture, texture_view, buffer)
}

fn find_texture_for_view<'a>(
    device: &Device,
    buffers: &'a mut HashMap<(usize, usize), (Texture, TextureView)>,
    view: View,
) -> (&'a mut Texture, &'a mut TextureView) {
    let texture_size = (
        smallest_multiple_containing::<usize>(view.image_width, 64),
        smallest_multiple_containing::<usize>(view.image_height, 64),
    );
    let (texture, texture_view) = buffers.entry(texture_size).or_insert_with(|| {
        let width = texture_size.0 as u32;
        let height = texture_size.1 as u32;
        info!(
            "Creating new framebuffer with dimensions ({}x{})...",
            width, height
        );
        let (texture, texture_view) = create_texture(
            &device,
            width as u32,
            height as u32,
            TextureFormat::Rgba8Unorm,
            TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT,
        );
        (texture, texture_view)
    });
    (texture, texture_view)
}

async fn write_uniforms(
    device: &Device,
    uniforms_buffer: &mut BufferWrapper<Uniforms>,
    view: View,
) -> CommandBuffer {
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
    cb
}

fn encode_render_pass(
    render_pipeline: &RenderPipeline,
    uniform_bind_group: &BindGroup,
    texture_view: &TextureView,
    encoder: &mut CommandEncoder,
) {
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

#[async_trait]
impl FractalGeneratorInstance for GpuFractalGeneratorInstance {
    async fn progress(&self) -> anyhow::Result<f32> {
        Ok(self.completed.load(Ordering::Relaxed) as f32 / self.view_count as f32)
    }

    async fn running(&self) -> anyhow::Result<bool> {
        Ok(self.completed.load(Ordering::Relaxed) < self.view_count)
    }
}
