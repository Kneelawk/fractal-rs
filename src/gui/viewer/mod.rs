//! viewer.rs - This file holds the systems for the fractal image viewer. This
//! means both image managing and rendering.

mod uniforms;

use crate::{
    gpu::{
        buffer::{BufferWrapper, BufferWriteError, Encodable},
        util::create_texture,
    },
    gui::viewer::uniforms::Uniforms,
};
use cgmath::Matrix4;
use std::{borrow::Cow, num::NonZeroU64, sync::Arc};
use wgpu::{
    AddressMode, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, BufferAddress, BufferBinding,
    BufferBindingType, BufferUsages, ColorTargetState, ColorWrites, Device, Face, FilterMode,
    FragmentState, FrontFace, MultisampleState, PipelineLayoutDescriptor, PolygonMode,
    PrimitiveState, PrimitiveTopology, Queue, RenderPipelineDescriptor, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, TextureFormat, TextureSampleType,
    TextureUsages, TextureViewDimension, VertexState,
};

const IMAGE_SOURCE: &str = include_str!("image.wgsl");

pub struct FractalViewer {}

impl FractalViewer {
    pub async fn new(
        device: &Device,
        queue: &Queue,
        frame_format: TextureFormat,
        frame_width: u32,
        frame_height: u32,
        fractal_width: u32,
        fractal_height: u32,
    ) -> Result<FractalViewer, FractalViewerError> {
        //
        // Static Components
        //

        let image_shader = ShaderSource::Wgsl(Cow::Borrowed(IMAGE_SOURCE));
        let image_module = device.create_shader_module(&ShaderModuleDescriptor {
            label: Some("Image Shader"),
            source: image_shader,
        });

        let image_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Image Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(NonZeroU64::new(Uniforms::size() as u64).unwrap()),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Sampler {
                        filtering: true,
                        comparison: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let image_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Image Pipeline Layout"),
            bind_group_layouts: &[&image_bind_group_layout],
            push_constant_ranges: &[],
        });

        let image_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Image Pipeline"),
            layout: Some(&image_pipeline_layout),
            vertex: VertexState {
                module: &image_module,
                entry_point: "vert_main",
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module: &image_module,
                entry_point: "frag_main",
                targets: &[ColorTargetState {
                    format: frame_format,
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
        });

        let image_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Image Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // Note that the buffer itself does not need to be re-created, however its
        // contents to need to be re-written.
        let mut image_uniforms_buffer = BufferWrapper::new(
            &device,
            Uniforms::size() as BufferAddress,
            BufferUsages::UNIFORM,
        );

        //
        // Fractal-Size Dependant Components
        //

        let image_uniforms = Uniforms {
            screen: Matrix4::from_nonuniform_scale(
                1.0 / frame_width as f32,
                1.0 / frame_height as f32,
                1.0,
            ),
            model: Matrix4::from_nonuniform_scale(fractal_width as f32, fractal_height as f32, 1.0),
        };
        let image_uniforms_buffer_cb = image_uniforms_buffer
            .replace_all(&device, &[image_uniforms])
            .await?;

        let (image_texture, image_texture_view) = create_texture(
            device,
            fractal_width,
            fractal_height,
            TextureFormat::Rgba8Unorm,
            TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING,
        );

        let image_texture = Arc::new(image_texture);
        let image_texture_view = Arc::new(image_texture_view);

        let image_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Image Bind Group"),
            layout: &image_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: image_uniforms_buffer.buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&image_sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&image_texture_view),
                },
            ],
        });

        queue.submit([image_uniforms_buffer_cb]);

        Ok(FractalViewer {})
    }
}

#[derive(Debug, Error)]
pub enum FractalViewerError {
    #[error("Buffer Write Error")]
    BufferWriteError(#[from] BufferWriteError),
}
