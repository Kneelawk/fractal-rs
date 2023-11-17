use crate::{
    generator::{
        gpu::{
            shader::load_shaders,
            uniforms::{GpuView, Uniforms},
        },
        util::{copy_region, smallest_multiple_containing},
        view::View,
        FractalGenerator, FractalGeneratorFactory, FractalGeneratorInstance, FractalOpts,
        PixelBlock, BYTES_PER_PIXEL,
    },
    gpu::{
        buffer::{BufferWrapper, Encodable},
        util::{create_texture, create_texture_buffer},
        GPUContext, GPUContextType,
    },
    util::{display_duration, result::ResultExt, running_guard::RunningGuard},
};
use anyhow::Context;
use chrono::{DateTime, Utc};
use futures::{
    future::{ready, BoxFuture},
    FutureExt,
};
use std::{
    collections::HashMap,
    num::NonZeroU64,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
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
    MultisampleState, Operations, Origin3d, PipelineLayout, PipelineLayoutDescriptor, PolygonMode,
    PrimitiveState, PrimitiveTopology, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderStages, StoreOp,
    Texture, TextureAspect, TextureFormat, TextureUsages, TextureView, VertexState,
};

mod shader;
mod uniforms;

pub struct GpuFractalGeneratorFactory {
    gpu: GPUContext,
    uniform_bind_group_layout: Arc<BindGroupLayout>,
    render_pipeline_layout: Arc<PipelineLayout>,
}

impl GpuFractalGeneratorFactory {
    pub fn new(gpu: GPUContext) -> GpuFractalGeneratorFactory {
        info!("Creating uniform bind group layout...");
        let uniform_bind_group_layout = Arc::new(gpu.device.create_bind_group_layout(
            &BindGroupLayoutDescriptor {
                label: Some("Uniform Bind Group Layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(NonZeroU64::new(Uniforms::size() as u64).unwrap()),
                    },
                    count: None,
                }],
            },
        ));

        info!("Creating render pipeline layout...");
        let render_pipeline_layout = Arc::new(gpu.device.create_pipeline_layout(
            &PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            },
        ));

        GpuFractalGeneratorFactory {
            gpu,
            uniform_bind_group_layout,
            render_pipeline_layout,
        }
    }
}

impl FractalGeneratorFactory for GpuFractalGeneratorFactory {
    fn create_generator(
        &self,
        opts: FractalOpts,
    ) -> BoxFuture<'static, anyhow::Result<Box<dyn FractalGenerator + Send + 'static>>> {
        let gpu = self.gpu.clone();
        let uniform_bind_group_layout = self.uniform_bind_group_layout.clone();
        let render_pipeline_layout = self.render_pipeline_layout.clone();

        async move {
            let boxed: Box<dyn FractalGenerator + Send> = Box::new(
                GpuFractalGenerator::new(
                    opts,
                    gpu,
                    uniform_bind_group_layout,
                    render_pipeline_layout,
                )
                .await?,
            );
            Ok(boxed)
        }
        .boxed()
    }
}

pub struct GpuFractalGenerator {
    opts: FractalOpts,
    gpu: GPUContext,
    uniform_bind_group_layout: Arc<BindGroupLayout>,
    render_pipeline: Arc<RenderPipeline>,
}

impl GpuFractalGenerator {
    async fn new(
        opts: FractalOpts,
        gpu: GPUContext,
        uniform_bind_group_layout: Arc<BindGroupLayout>,
        render_pipeline_layout: Arc<PipelineLayout>,
    ) -> anyhow::Result<GpuFractalGenerator> {
        info!("Creating shader modules...");
        let shaders = load_shaders(opts).await.context("Error loading shaders")?;
        let frag_module = gpu.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Fragment Shader"),
            source: shaders.fragment,
        });

        let vert_module = gpu.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Vertex Shader"),
            source: shaders.vertex,
        });

        info!("Creating render pipeline...");
        let render_pipeline = Arc::new(gpu.device.create_render_pipeline(
            &RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: VertexState {
                    module: &vert_module,
                    entry_point: "vert_main",
                    buffers: &[],
                },
                fragment: Some(FragmentState {
                    module: &frag_module,
                    entry_point: "frag_main",
                    targets: &[Some(ColorTargetState {
                        format: TextureFormat::Rgba8Unorm,
                        blend: Some(BlendState::REPLACE),
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: Some(Face::Back),
                    polygon_mode: PolygonMode::Fill,
                    conservative: false,
                    unclipped_depth: false,
                },
                depth_stencil: None,
                multisample: MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            },
        ));

        Ok(GpuFractalGenerator {
            opts,
            gpu,
            uniform_bind_group_layout,
            render_pipeline,
        })
    }
}

impl FractalGenerator for GpuFractalGenerator {
    fn min_views_hint(&self) -> BoxFuture<'static, anyhow::Result<usize>> {
        ready(Ok(1)).boxed()
    }

    fn start_generation_to_cpu(
        &self,
        views: &[View],
        sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> BoxFuture<'static, anyhow::Result<Box<dyn FractalGeneratorInstance + Send + 'static>>>
    {
        // This future must be 'static so we need to copy everything or use Arcs.
        let opts = self.opts;
        let gpu = self.gpu.clone();
        let uniform_bind_group_layout = self.uniform_bind_group_layout.clone();
        let render_pipeline = self.render_pipeline.clone();
        let views = views.to_vec();

        async move {
            let boxed: Box<dyn FractalGeneratorInstance + Send> =
                Box::new(GpuFractalGeneratorInstance::start_to_cpu(
                    opts,
                    gpu,
                    uniform_bind_group_layout,
                    render_pipeline,
                    views,
                    sender,
                ));
            Ok(boxed)
        }
        .boxed()
    }

    fn start_generation_to_gpu(
        &self,
        views: &[View],
        present: GPUContext,
        texture: Arc<Texture>,
        _texture_view: Arc<TextureView>,
    ) -> BoxFuture<'static, anyhow::Result<Box<dyn FractalGeneratorInstance + Send + 'static>>>
    {
        assert_eq!(
            present.ty,
            GPUContextType::Presentable,
            "To-GPU GPUContext.ty must be GPUContextType::Presentable (this is a bug)"
        );

        let opts = self.opts;
        let gpu = self.gpu.clone();
        let uniform_bind_group_layout = self.uniform_bind_group_layout.clone();
        let render_pipeline = self.render_pipeline.clone();
        let views = views.to_vec();

        async move {
            let boxed: Box<dyn FractalGeneratorInstance + Send> =
                Box::new(GpuFractalGeneratorInstance::start_to_gpu(
                    opts,
                    gpu,
                    present,
                    uniform_bind_group_layout,
                    render_pipeline,
                    views,
                    texture,
                ));
            Ok(boxed)
        }
        .boxed()
    }
}

struct GpuFractalGeneratorInstance {
    view_count: usize,
    completed: Arc<AtomicUsize>,
    running: Arc<AtomicBool>,
    canceled: Arc<AtomicBool>,
}

impl GpuFractalGeneratorInstance {
    fn start_to_cpu(
        _opts: FractalOpts,
        gpu: GPUContext,
        uniform_bind_group_layout: Arc<BindGroupLayout>,
        render_pipeline: Arc<RenderPipeline>,
        views: Vec<View>,
        sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> GpuFractalGeneratorInstance {
        let start_time = Utc::now();
        let view_count = views.len();
        let completed = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicBool::new(true));
        let canceled = Arc::new(AtomicBool::new(false));
        let spawn_completed = completed.clone();
        let spawn_running = running.clone();
        let spawn_canceled = canceled.clone();

        let (mut uniforms_buffer, uniform_bind_group) =
            setup_uniforms(&gpu.device, &uniform_bind_group_layout);

        info!("Spawning gpu manager task...");

        tokio::spawn(async move {
            let _gpu = &gpu;
            let _running_guard = RunningGuard::new(spawn_running);

            let mut buffers = HashMap::new();

            for view in views {
                if spawn_canceled.load(Ordering::Acquire) {
                    info!("Received cancel signal.");
                    return;
                }

                let (texture_width, texture_height, texture, texture_view, buffer) =
                    find_texture_buffer_for_view(&gpu.device, &mut buffers, view);

                let uniforms_cb = write_uniforms(&gpu.device, &mut uniforms_buffer, view).await;

                {
                    info!(
                        "Encoding render command buffer for ({}, {})...",
                        view.image_x, view.image_y
                    );
                    let mut encoder =
                        gpu.device
                            .create_command_encoder(&CommandEncoderDescriptor {
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
                                bytes_per_row: Some(BYTES_PER_PIXEL as u32 * texture_width),
                                rows_per_image: Some(texture_height),
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
                    gpu.queue.submit([uniforms_cb, encoder.finish()]);
                }

                let mut image_data =
                    vec![0u8; view.image_width * view.image_height * BYTES_PER_PIXEL];
                {
                    info!(
                        "Reading framebuffer for ({}, {})...",
                        view.image_x, view.image_y
                    );
                    let buffer_slice = buffer.slice(..);

                    let (tx, rx) = tokio::sync::oneshot::channel();
                    buffer_slice.map_async(MapMode::Read, move |res| {
                        tx.send(res)
                            .on_err(|_| error!("Failed to send map_async completion!"));
                    });
                    rx.await.unwrap().unwrap();

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

                let completed = spawn_completed.fetch_add(1, Ordering::AcqRel) + 1;

                if completed == view_count {
                    display_duration(start_time);
                }

                info!(
                    "Sending pixel block for ({}, {})...",
                    view.image_x, view.image_y
                );
                if let Err(e) = sender
                    .send(Ok(PixelBlock {
                        view,
                        image: image_data.into_boxed_slice(),
                    }))
                    .await
                {
                    warn!("Unable to send pixel block! Error: {:?}", e);
                    return;
                }
            }
        });

        GpuFractalGeneratorInstance {
            view_count,
            completed,
            running,
            canceled,
        }
    }

    fn start_to_gpu(
        _opts: FractalOpts,
        gpu: GPUContext,
        present: GPUContext,
        uniform_bind_group_layout: Arc<BindGroupLayout>,
        render_pipeline: Arc<RenderPipeline>,
        views: Vec<View>,
        out_texture: Arc<Texture>,
    ) -> GpuFractalGeneratorInstance {
        let start_time = Utc::now();
        let view_count = views.len();
        let completed = Arc::new(AtomicUsize::new(0));
        let running = Arc::new(AtomicBool::new(true));
        let canceled = Arc::new(AtomicBool::new(false));
        let spawn_completed = completed.clone();
        let spawn_running = running.clone();
        let spawn_canceled = canceled.clone();

        let (uniforms_buffer, uniform_bind_group) =
            setup_uniforms(&gpu.device, &uniform_bind_group_layout);

        info!("Spawning gpu manager task...");

        tokio::spawn(async move {
            let _gpu = &gpu;
            let _running_guard = RunningGuard::new(spawn_running);

            if gpu.ty == GPUContextType::Presentable {
                Self::generate_to_same_device(
                    gpu,
                    render_pipeline,
                    views,
                    out_texture,
                    start_time,
                    view_count,
                    spawn_completed,
                    spawn_canceled,
                    uniforms_buffer,
                    uniform_bind_group,
                )
                .await;
            } else {
                Self::generate_to_different_device(
                    gpu,
                    present,
                    render_pipeline,
                    views,
                    out_texture,
                    start_time,
                    view_count,
                    spawn_completed,
                    spawn_canceled,
                    uniforms_buffer,
                    uniform_bind_group,
                )
                .await;
            }
        });

        GpuFractalGeneratorInstance {
            view_count,
            completed,
            running,
            canceled,
        }
    }

    async fn generate_to_same_device(
        gpu: GPUContext,
        render_pipeline: Arc<RenderPipeline>,
        views: Vec<View>,
        out_texture: Arc<Texture>,
        start_time: DateTime<Utc>,
        view_count: usize,
        spawn_completed: Arc<AtomicUsize>,
        spawn_canceled: Arc<AtomicBool>,
        mut uniforms_buffer: BufferWrapper<Uniforms>,
        uniform_bind_group: BindGroup,
    ) {
        let mut buffers = HashMap::new();

        for view in views {
            if spawn_canceled.load(Ordering::Acquire) {
                info!("Received cancel signal.");
                return;
            }

            let (texture, texture_view) = find_texture_for_view(&gpu.device, &mut buffers, view);

            let uniforms_cb = write_uniforms(&gpu.device, &mut uniforms_buffer, view).await;

            {
                info!(
                    "Encoding render command buffer for ({}, {})...",
                    view.image_x, view.image_y
                );
                let mut encoder = gpu
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor {
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
                        depth_or_array_layers: 1,
                    },
                );

                gpu.queue.submit([uniforms_cb, encoder.finish()]);
            }

            let completed = spawn_completed.fetch_add(1, Ordering::AcqRel) + 1;

            if completed == view_count {
                display_duration(start_time);
            }
        }
    }

    async fn generate_to_different_device(
        gpu: GPUContext,
        present: GPUContext,
        render_pipeline: Arc<RenderPipeline>,
        views: Vec<View>,
        out_texture: Arc<Texture>,
        start_time: DateTime<Utc>,
        view_count: usize,
        spawn_completed: Arc<AtomicUsize>,
        spawn_canceled: Arc<AtomicBool>,
        mut uniforms_buffer: BufferWrapper<Uniforms>,
        uniform_bind_group: BindGroup,
    ) {
        let mut buffers = HashMap::new();

        for view in views {
            if spawn_canceled.load(Ordering::Acquire) {
                info!("Received cancel signal.");
                return;
            }

            let (texture_width, texture_height, texture, texture_view, buffer) =
                find_texture_buffer_for_view(&gpu.device, &mut buffers, view);

            let uniforms_cb = write_uniforms(&gpu.device, &mut uniforms_buffer, view).await;

            {
                info!(
                    "Encoding render command buffer for ({}, {})...",
                    view.image_x, view.image_y
                );
                let mut encoder = gpu
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor {
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
                            bytes_per_row: Some(BYTES_PER_PIXEL as u32 * texture_width),
                            rows_per_image: Some(texture_height),
                        },
                    },
                    Extent3d {
                        width: texture_width,
                        height: texture_height,
                        depth_or_array_layers: 1,
                    },
                );

                gpu.queue.submit([uniforms_cb, encoder.finish()]);
            }

            {
                info!(
                    "Reading framebuffer for ({}, {})...",
                    view.image_x, view.image_y
                );
                let buffer_slice = buffer.slice(..);

                let (tx, rx) = tokio::sync::oneshot::channel();
                buffer_slice.map_async(MapMode::Read, move |res| {
                    tx.send(res)
                        .on_err(|_| error!("Failed to send map_async completion!"));
                });
                rx.await.unwrap().unwrap();

                let data = buffer_slice.get_mapped_range();

                info!(
                    "Writing image piece to other device for ({}, {})",
                    view.image_x, view.image_y
                );
                present.queue.write_texture(
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
                    data.as_ref(),
                    ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some((view.image_width * BYTES_PER_PIXEL) as u32),
                        rows_per_image: None,
                    },
                    Extent3d {
                        width: view.image_width as u32,
                        height: view.image_height as u32,
                        depth_or_array_layers: 1,
                    },
                )
            }

            buffer.unmap();

            let completed = spawn_completed.fetch_add(1, Ordering::AcqRel) + 1;

            if completed == view_count {
                display_duration(start_time);
            }
        }
    }
}

fn setup_uniforms(
    device: &Device,
    uniform_bind_group_layout: &BindGroupLayout,
) -> (BufferWrapper<Uniforms>, BindGroup) {
    info!("Creating uniform buffer...");
    let uniforms_buffer = BufferWrapper::<Uniforms>::new(
        device,
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
            device,
            width as u32,
            height as u32,
            TextureFormat::Rgba8Unorm,
            TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT,
        );
        let buffer = create_texture_buffer(
            device,
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
            device,
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
            device,
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
        color_attachments: &[Some(RenderPassColorAttachment {
            view: texture_view,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }),
                store: StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });

    render_pass.set_pipeline(render_pipeline);
    render_pass.set_bind_group(0, uniform_bind_group, &[]);
    render_pass.draw(0..6, 0..1);
}

impl FractalGeneratorInstance for GpuFractalGeneratorInstance {
    fn cancel(&self) {
        self.canceled.store(true, Ordering::Release);
    }

    fn progress(&self) -> BoxFuture<'static, anyhow::Result<f32>> {
        ready(Ok(
            self.completed.load(Ordering::Acquire) as f32 / self.view_count as f32
        ))
        .boxed()
    }

    fn running(&self) -> BoxFuture<'static, anyhow::Result<bool>> {
        ready(Ok(self.running.load(Ordering::Acquire))).boxed()
    }
}
