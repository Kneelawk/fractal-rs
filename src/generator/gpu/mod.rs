use crate::generator::{
    args::Multisampling,
    gpu::{buffer::Encodable, shader::load_template, uniforms::Uniforms},
    view::View,
    FractalGenerator, FractalGeneratorInstance, FractalOpts, PixelBlock,
};
use cgmath::Vector2;
use futures::{
    future::{ready, BoxFuture},
    FutureExt,
};
use std::{
    num::NonZeroU64,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc::Sender;
use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState,
    BufferBindingType, ColorTargetState, ColorWrites, Device, Face, FragmentState, FrontFace,
    MultisampleState, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology,
    Queue, RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderStages,
    TextureFormat, VertexState,
};

mod buffer;
mod multisample;
mod no_multisample;
mod shader;
mod uniforms;
mod util;

pub struct GpuFractalGenerator {
    opts: FractalOpts,
    device: Arc<Device>,
    queue: Arc<Queue>,
    uniform_bind_group_layout: BindGroupLayout,
    render_pipeline: Arc<RenderPipeline>,
    multisample_bind_group_layout: Option<Arc<BindGroupLayout>>,
    multisample_render_pipeline: Option<Arc<RenderPipeline>>,
}

impl GpuFractalGenerator {
    pub async fn new(
        opts: FractalOpts,
        device: Arc<Device>,
        queue: Arc<Queue>,
    ) -> anyhow::Result<GpuFractalGenerator> {
        info!("Creating shader module...");
        let shader = load_template(opts).await?;
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

        let (multisample_bind_group_layout, multisample_render_pipeline) = match opts.multisampling
        {
            Multisampling::None => (None, None),
            Multisampling::FourPoints { .. } => {
                multisample::create_layout_and_pipeline(&device, 4).await?
            },
            Multisampling::Linear { axial_points } => {
                multisample::create_layout_and_pipeline(&device, axial_points * axial_points)
                    .await?
            },
        };

        Ok(GpuFractalGenerator {
            opts,
            device,
            queue,
            uniform_bind_group_layout,
            render_pipeline,
            multisample_bind_group_layout,
            multisample_render_pipeline,
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
                    self.multisample_bind_group_layout.clone(),
                    self.multisample_render_pipeline.clone(),
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
        opts: FractalOpts,
        device: Arc<Device>,
        queue: Arc<Queue>,
        uniform_bind_group_layout: &BindGroupLayout,
        render_pipeline: Arc<RenderPipeline>,
        multisample_bind_group_layout: Option<Arc<BindGroupLayout>>,
        multisample_render_pipeline: Option<Arc<RenderPipeline>>,
        views: Vec<View>,
        sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> GpuFractalGeneratorInstance {
        let view_count = views.len();
        let completed = Arc::new(AtomicUsize::new(0));
        let spawn_completed = completed.clone();

        match opts.multisampling {
            Multisampling::None => {
                no_multisample::generate(
                    device,
                    queue,
                    uniform_bind_group_layout,
                    render_pipeline,
                    sender,
                    views,
                    spawn_completed,
                );
            },
            Multisampling::FourPoints { offset } => {
                multisample::generate(
                    device,
                    queue,
                    uniform_bind_group_layout,
                    render_pipeline,
                    multisample_bind_group_layout.unwrap(),
                    multisample_render_pipeline.unwrap(),
                    sender,
                    views,
                    spawn_completed,
                    build_four_points_offsets(offset),
                );
            },
            Multisampling::Linear { axial_points } => {
                multisample::generate(
                    device,
                    queue,
                    uniform_bind_group_layout,
                    render_pipeline,
                    multisample_bind_group_layout.unwrap(),
                    multisample_render_pipeline.unwrap(),
                    sender,
                    views,
                    spawn_completed,
                    build_linear_offsets(axial_points),
                );
            },
        }

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

fn build_four_points_offsets(offset: f32) -> Vec<Vector2<f32>> {
    vec![
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
    ]
}

fn build_linear_offsets(axial_points: u32) -> Vec<Vector2<f32>> {
    let mut vec = vec![];

    let offset = 1.0 / axial_points as f32;
    let initial_offset = offset / 2.0;

    for y in 0..axial_points {
        for x in 0..axial_points {
            vec.push(Vector2 {
                x: x as f32 * offset + initial_offset - 0.5,
                y: y as f32 * offset + initial_offset - 0.5,
            })
        }
    }

    vec
}
