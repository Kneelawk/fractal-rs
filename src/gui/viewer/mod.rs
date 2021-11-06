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
use cgmath::{Matrix4, SquareMatrix};
use std::{borrow::Cow, num::NonZeroU64, sync::Arc};
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, BlendState,
    BufferBinding, BufferBindingType, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandBuffer, CommandEncoderDescriptor, Device, Face, FilterMode, FragmentState, FrontFace,
    LoadOp, MultisampleState, Operations, PipelineLayoutDescriptor, PolygonMode, PrimitiveState,
    PrimitiveTopology, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, Sampler, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource,
    ShaderStages, Texture, TextureFormat, TextureSampleType, TextureUsages, TextureView,
    TextureViewDimension, VertexState,
};

const IMAGE_SOURCE: &str = include_str!("image.wgsl");

pub struct FractalViewer {
    // Static Components
    image_bind_group_layout: BindGroupLayout,
    image_pipeline: RenderPipeline,
    image_sampler: Sampler,
    image_uniforms_buffer: BufferWrapper<Uniforms>,

    // Dynamic Components
    image_uniforms: Uniforms,
    image_texture: Arc<Texture>,
    image_texture_view: Arc<TextureView>,
    image_bind_group: BindGroup,
}

impl FractalViewer {
    pub async fn new(
        device: &Device,
        frame_format: TextureFormat,
        frame_width: u32,
        frame_height: u32,
        fractal_width: u32,
        fractal_height: u32,
    ) -> Result<(FractalViewer, CommandBuffer), FractalViewerError> {
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
        let mut image_uniforms_buffer = BufferWrapper::new(&device, 1, BufferUsages::UNIFORM);

        //
        // Fractal-Size Dependant Components
        //

        let image_uniforms = Uniforms {
            from_screen: Matrix4::from_nonuniform_scale(
                1.0 / frame_width as f32,
                1.0 / frame_height as f32,
                1.0,
            ),
            model: Matrix4::identity(),
            to_screen: Matrix4::from_nonuniform_scale(
                fractal_width as f32,
                fractal_height as f32,
                1.0,
            ),
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

        Ok((
            FractalViewer {
                image_bind_group_layout,
                image_pipeline,
                image_sampler,
                image_uniforms_buffer,
                image_uniforms,
                image_texture,
                image_texture_view,
                image_bind_group,
            },
            image_uniforms_buffer_cb,
        ))
    }

    pub fn get_texture(&self) -> Arc<Texture> {
        self.image_texture.clone()
    }

    pub fn get_texture_view(&self) -> Arc<TextureView> {
        self.image_texture_view.clone()
    }

    pub async fn set_fractal_size(
        &mut self,
        device: &Device,
        width: u32,
        height: u32,
    ) -> Result<CommandBuffer, FractalViewerError> {
        self.image_uniforms.to_screen =
            Matrix4::from_nonuniform_scale(width as f32, height as f32, 1.0);
        let image_uniforms_buffer_cb = self
            .image_uniforms_buffer
            .replace_all(&device, &[self.image_uniforms])
            .await?;

        let (image_texture, image_texture_view) = create_texture(
            device,
            width,
            height,
            TextureFormat::Rgba8Unorm,
            TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING,
        );

        self.image_texture = Arc::new(image_texture);
        self.image_texture_view = Arc::new(image_texture_view);

        self.image_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Image Bind Group"),
            layout: &self.image_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: self.image_uniforms_buffer.buffer(),
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&self.image_sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&self.image_texture_view),
                },
            ],
        });

        Ok(image_uniforms_buffer_cb)
    }

    pub async fn set_frame_size(
        &mut self,
        device: &Device,
        width: u32,
        height: u32,
    ) -> Result<CommandBuffer, FractalViewerError> {
        self.image_uniforms.from_screen =
            Matrix4::from_nonuniform_scale(1.0 / width as f32, 1.0 / height as f32, 1.0);
        Ok(self
            .image_uniforms_buffer
            .replace_all(device, &[self.image_uniforms])
            .await?)
    }

    pub fn render(
        &self,
        device: &Device,
        frame_view: &TextureView,
        load_op: LoadOp<Color>,
    ) -> CommandBuffer {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Fractal Viewer Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Fractal Viewer Render Pass"),
                color_attachments: &[RenderPassColorAttachment {
                    view: frame_view,
                    resolve_target: None,
                    ops: Operations {
                        load: load_op,
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.image_pipeline);
            render_pass.set_bind_group(0, &self.image_bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }

        encoder.finish()
    }
}

#[derive(Debug, Error)]
pub enum FractalViewerError {
    #[error("Buffer Write Error")]
    BufferWriteError(#[from] BufferWriteError),
}
