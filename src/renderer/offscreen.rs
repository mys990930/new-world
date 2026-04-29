use std::fmt;
use std::path::Path;
use std::sync::mpsc;

use bytemuck::cast_slice;
use image::{ImageBuffer, RgbaImage};
use pollster::block_on;
use wgpu::util::DeviceExt;

use super::camera::CameraUniform;
use super::surface::{
    EnvironmentUniform, SunShadowUniform, create_environment_bind_group_layout,
    create_shadow_sampling_bind_group_layout,
};
use super::texture::{
    BlockTextureSet, RenderTextureArraySource, RenderTextureError,
    create_block_texture_bind_group_layout, create_gpu_block_texture_resources,
};
use super::upload::split_chunk_mesh_for_transparency;
use super::{
    CameraGpuState, ClearColor, CpuMesh, MeshVertex, RenderCameraState, RenderConfig,
    RenderEnvironment,
};

#[derive(Debug, Clone, PartialEq)]
pub struct OffscreenRenderRequest {
    pub width: u32,
    pub height: u32,
    pub camera: RenderCameraState,
    pub textures: RenderTextureArraySource,
    pub environment: RenderEnvironment,
    pub chunk_meshes: Vec<CpuMesh>,
    pub clear_color_override: Option<[f32; 4]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffscreenRenderOutput {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub draw_call_count: u32,
}

#[derive(Debug)]
pub enum OffscreenRenderError {
    InvalidDimensions { width: u32, height: u32 },
    Texture(RenderTextureError),
    AdapterRequest(String),
    DeviceRequest(String),
    Camera(super::CameraUpdateError),
    BufferMap(String),
    InvalidImageBuffer,
    Image(image::ImageError),
}

impl fmt::Display for OffscreenRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid offscreen dimensions: {}x{}", width, height)
            }
            Self::Texture(error) => write!(f, "offscreen texture setup failed: {error:?}"),
            Self::AdapterRequest(error) => write!(f, "offscreen adapter request failed: {error}"),
            Self::DeviceRequest(error) => write!(f, "offscreen device request failed: {error}"),
            Self::Camera(error) => write!(f, "offscreen camera update failed: {error:?}"),
            Self::BufferMap(error) => write!(f, "offscreen buffer map failed: {error}"),
            Self::InvalidImageBuffer => write!(f, "offscreen output buffer size was invalid"),
            Self::Image(error) => write!(f, "failed to write offscreen image: {error}"),
        }
    }
}

impl std::error::Error for OffscreenRenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Image(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RenderTextureError> for OffscreenRenderError {
    fn from(value: RenderTextureError) -> Self {
        Self::Texture(value)
    }
}

impl From<super::CameraUpdateError> for OffscreenRenderError {
    fn from(value: super::CameraUpdateError) -> Self {
        Self::Camera(value)
    }
}

impl From<image::ImageError> for OffscreenRenderError {
    fn from(value: image::ImageError) -> Self {
        Self::Image(value)
    }
}

pub fn render_offscreen(
    request: OffscreenRenderRequest,
) -> Result<OffscreenRenderOutput, OffscreenRenderError> {
    if request.width == 0 || request.height == 0 {
        return Err(OffscreenRenderError::InvalidDimensions {
            width: request.width,
            height: request.height,
        });
    }

    let config = RenderConfig::default();
    let textures = BlockTextureSet::from_source(&request.textures)?;
    let instance = wgpu::Instance::default();
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: None,
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
    }))
    .map_err(|error| OffscreenRenderError::AdapterRequest(error.to_string()))?;

    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("offscreen_renderer_device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
    }))
    .map_err(|error| OffscreenRenderError::DeviceRequest(error.to_string()))?;

    let color_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen_color_texture"),
        size: wgpu::Extent3d {
            width: request.width,
            height: request.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let (depth_texture, depth_view) = create_depth_texture(
        &device,
        request.width,
        request.height,
        wgpu::TextureFormat::Depth32Float,
    );
    let _ = depth_texture;

    let terrain_shader = device.create_shader_module(wgpu::include_wgsl!("terrain.wgsl"));
    let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("offscreen_camera_buffer"),
        size: std::mem::size_of::<CameraUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let camera_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("offscreen_camera_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
    let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("offscreen_camera_bind_group"),
        layout: &camera_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: camera_buffer.as_entire_binding(),
        }],
    });

    let environment_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("offscreen_environment_buffer"),
        size: std::mem::size_of::<EnvironmentUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let environment_bind_group_layout = create_environment_bind_group_layout(&device);
    let environment_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("offscreen_environment_bind_group"),
        layout: &environment_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: environment_buffer.as_entire_binding(),
        }],
    });

    let shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("offscreen_shadow_uniform_buffer"),
        size: std::mem::size_of::<SunShadowUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen_shadow_texture"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("offscreen_shadow_sampler"),
        compare: Some(wgpu::CompareFunction::LessEqual),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });
    let shadow_bind_group_layout = create_shadow_sampling_bind_group_layout(&device);
    let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("offscreen_shadow_bind_group"),
        layout: &shadow_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: shadow_uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&shadow_sampler),
            },
        ],
    });

    let block_texture_bind_group_layout = create_block_texture_bind_group_layout(&device);
    let block_textures = create_gpu_block_texture_resources(
        &device,
        &queue,
        &block_texture_bind_group_layout,
        &textures,
    );

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("offscreen_terrain_pipeline_layout"),
        bind_group_layouts: &[
            Some(&camera_bind_group_layout),
            Some(&environment_bind_group_layout),
            Some(&block_texture_bind_group_layout),
            Some(&shadow_bind_group_layout),
        ],
        immediate_size: 0,
    });
    let terrain_pipeline = create_terrain_pipeline(
        &device,
        &pipeline_layout,
        &terrain_shader,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Depth32Float,
        Some(wgpu::BlendState::REPLACE),
        true,
    );
    let water_pipeline = create_terrain_pipeline(
        &device,
        &pipeline_layout,
        &terrain_shader,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Depth32Float,
        Some(wgpu::BlendState::ALPHA_BLENDING),
        false,
    );

    let mut camera_gpu_state = CameraGpuState::default();
    camera_gpu_state.update(
        &request.camera,
        &config.camera_projection,
        request.width,
        request.height,
        0,
    )?;
    let camera_uniform = CameraUniform::from_view_projection_and_eye(
        camera_gpu_state.view_projection,
        camera_gpu_state.eye_position,
        camera_gpu_state.focus_position,
    );
    queue.write_buffer(&camera_buffer, 0, cast_slice(&[camera_uniform]));
    queue.write_buffer(
        &environment_buffer,
        0,
        cast_slice(&[EnvironmentUniform::from_settings(
            &request.environment,
            &config.quality,
        )]),
    );
    queue.write_buffer(
        &shadow_uniform_buffer,
        0,
        cast_slice(&[SunShadowUniform::disabled()]),
    );

    let gpu_meshes = request
        .chunk_meshes
        .into_iter()
        .filter(|mesh| !mesh.vertices.is_empty() && !mesh.indices.is_empty())
        .map(|mesh| {
            let (opaque_mesh, translucent_mesh) = split_chunk_mesh_for_transparency(mesh);
            OffscreenGpuMesh {
                opaque: create_offscreen_mesh_buffers(&device, &opaque_mesh),
                translucent: translucent_mesh
                    .as_ref()
                    .and_then(|mesh| create_offscreen_mesh_buffers(&device, mesh)),
            }
        })
        .collect::<Vec<_>>();

    let padded_bytes_per_row =
        (request.width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("offscreen_output_buffer"),
        size: u64::from(padded_bytes_per_row * request.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let clear_color = request.clear_color_override.unwrap_or_else(|| {
        request
            .environment
            .resolved_clear_color(ClearColor::default())
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("offscreen_encoder"),
    });
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("offscreen_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: clear_color[0] as f64,
                        g: clear_color[1] as f64,
                        b: clear_color[2] as f64,
                        a: clear_color[3] as f64,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        render_pass.set_pipeline(&terrain_pipeline);
        render_pass.set_bind_group(0, &camera_bind_group, &[]);
        render_pass.set_bind_group(1, &environment_bind_group, &[]);
        render_pass.set_bind_group(2, &block_textures.bind_group, &[]);
        render_pass.set_bind_group(3, &shadow_bind_group, &[]);

        for mesh in &gpu_meshes {
            let Some(opaque) = mesh.opaque.as_ref() else {
                continue;
            };
            render_pass.set_vertex_buffer(0, opaque.vertex_buffer.slice(..));
            render_pass.set_index_buffer(opaque.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..opaque.index_count, 0, 0..1);
        }

        render_pass.set_pipeline(&water_pipeline);
        for mesh in &gpu_meshes {
            let Some(translucent) = mesh.translucent.as_ref() else {
                continue;
            };
            render_pass.set_vertex_buffer(0, translucent.vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                translucent.index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.draw_indexed(0..translucent.index_count, 0, 0..1);
        }
    }

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &color_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &output_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(request.height),
            },
        },
        wgpu::Extent3d {
            width: request.width,
            height: request.height,
            depth_or_array_layers: 1,
        },
    );

    let submission_index = queue.submit(Some(encoder.finish()));
    let slice = output_buffer.slice(..);
    let (sender, receiver) = mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission_index),
            timeout: None,
        })
        .map_err(|error| OffscreenRenderError::BufferMap(error.to_string()))?;
    receiver
        .recv()
        .map_err(|error| OffscreenRenderError::BufferMap(error.to_string()))?
        .map_err(|error| OffscreenRenderError::BufferMap(format!("{error:?}")))?;

    let data = slice.get_mapped_range();
    let rgba = strip_padded_rows(&data, request.width, request.height, padded_bytes_per_row);
    drop(data);
    output_buffer.unmap();

    Ok(OffscreenRenderOutput {
        width: request.width,
        height: request.height,
        rgba,
        draw_call_count: gpu_meshes
            .iter()
            .map(|mesh| u32::from(mesh.opaque.is_some()) + u32::from(mesh.translucent.is_some()))
            .sum(),
    })
}

pub fn write_offscreen_png(
    path: impl AsRef<Path>,
    image: &OffscreenRenderOutput,
) -> Result<(), OffscreenRenderError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| OffscreenRenderError::BufferMap(error.to_string()))?;
    }

    let png: RgbaImage = ImageBuffer::from_raw(image.width, image.height, image.rgba.clone())
        .ok_or(OffscreenRenderError::InvalidImageBuffer)?;
    png.save(path)?;
    Ok(())
}

struct OffscreenGpuMesh {
    opaque: Option<OffscreenMeshBuffers>,
    translucent: Option<OffscreenMeshBuffers>,
}

struct OffscreenMeshBuffers {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

fn create_offscreen_mesh_buffers(
    device: &wgpu::Device,
    mesh: &CpuMesh,
) -> Option<OffscreenMeshBuffers> {
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return None;
    }

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("offscreen_chunk_vertex_buffer"),
        contents: cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("offscreen_chunk_index_buffer"),
        contents: cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    Some(OffscreenMeshBuffers {
        vertex_buffer,
        index_buffer,
        index_count: mesh.indices.len() as u32,
    })
}

fn create_terrain_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
    depth_write_enabled: bool,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("offscreen_terrain_pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[MeshVertex::vertex_buffer_layout()],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: Some(depth_write_enabled),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen_depth_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn strip_padded_rows(bytes: &[u8], width: u32, height: u32, padded_bytes_per_row: u32) -> Vec<u8> {
    let tight_bytes_per_row = width as usize * 4;
    let padded_bytes_per_row = padded_bytes_per_row as usize;
    let mut rgba = Vec::with_capacity(tight_bytes_per_row * height as usize);

    for row in 0..height as usize {
        let start = row * padded_bytes_per_row;
        rgba.extend_from_slice(&bytes[start..start + tight_bytes_per_row]);
    }

    rgba
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::{QUARTER_VIEW_VERTICAL_WORLD_SIZE, quarter_view_basis, quarter_view_eye};
    use crate::renderer::{RenderProjectionMode, RenderViewBasis};

    #[test]
    fn strip_padded_rows_removes_alignment_padding() {
        let bytes = [
            1_u8, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 0, 0, 0, 0, //
            9_u8, 10, 11, 12, 13, 14, 15, 16, 0, 0, 0, 0, 0, 0, 0, 0,
        ];

        let rgba = strip_padded_rows(&bytes, 2, 2, 16);

        assert_eq!(
            rgba,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        );
    }

    #[test]
    fn camera_request_can_fit_simple_bounds() {
        let bounds = super::super::RenderBounds {
            min: [-4.0, -1.0, -4.0],
            max: [4.0, 3.0, 4.0],
        };
        let camera = preview_camera_for_bounds(bounds, 1280, 720, 0);
        let basis = quarter_view_basis(0);
        let half_height = match camera.projection_mode {
            RenderProjectionMode::Orthographic {
                vertical_world_size,
            } => vertical_world_size * 0.5,
            RenderProjectionMode::Perspective => unreachable!(),
        };
        let half_width = half_height * (1280.0 / 720.0);
        let corners = bounds_corners(bounds);
        let target = [
            (bounds.min[0] + bounds.max[0]) * 0.5,
            bounds.min[1] + (bounds.max[1] - bounds.min[1]) * 0.4,
            (bounds.min[2] + bounds.max[2]) * 0.5,
        ];

        for corner in corners {
            let delta = [
                corner[0] - target[0],
                corner[1] - target[1],
                corner[2] - target[2],
            ];
            assert!(dot3(delta, basis.right).abs() <= half_width + 0.001);
            assert!(dot3(delta, basis.up).abs() <= half_height + 0.001);
        }
    }

    pub(crate) fn preview_camera_for_bounds(
        bounds: super::super::RenderBounds,
        width: u32,
        height: u32,
        quarter_turns: u8,
    ) -> RenderCameraState {
        let aspect = width as f32 / height as f32;
        let basis = quarter_view_basis(quarter_turns);
        let target = [
            (bounds.min[0] + bounds.max[0]) * 0.5,
            bounds.min[1] + (bounds.max[1] - bounds.min[1]) * 0.4,
            (bounds.min[2] + bounds.max[2]) * 0.5,
        ];
        let mut right_extent = 0.0_f32;
        let mut up_extent = 0.0_f32;

        for corner in bounds_corners(bounds) {
            let delta = [
                corner[0] - target[0],
                corner[1] - target[1],
                corner[2] - target[2],
            ];
            right_extent = right_extent.max(dot3(delta, basis.right).abs());
            up_extent = up_extent.max(dot3(delta, basis.up).abs());
        }

        let half_height = (up_extent.max(right_extent / aspect) * 1.15)
            .max(QUARTER_VIEW_VERTICAL_WORLD_SIZE * 0.5);

        RenderCameraState {
            eye: quarter_view_eye(target, quarter_turns),
            target,
            up: basis.up,
            aspect_override: Some(aspect),
            projection_mode: RenderProjectionMode::Orthographic {
                vertical_world_size: half_height * 2.0,
            },
            basis_override: Some(RenderViewBasis {
                right: basis.right,
                up: basis.up,
                forward: basis.forward,
            }),
        }
    }

    fn bounds_corners(bounds: super::super::RenderBounds) -> [[f32; 3]; 8] {
        [
            [bounds.min[0], bounds.min[1], bounds.min[2]],
            [bounds.min[0], bounds.min[1], bounds.max[2]],
            [bounds.min[0], bounds.max[1], bounds.min[2]],
            [bounds.min[0], bounds.max[1], bounds.max[2]],
            [bounds.max[0], bounds.min[1], bounds.min[2]],
            [bounds.max[0], bounds.min[1], bounds.max[2]],
            [bounds.max[0], bounds.max[1], bounds.min[2]],
            [bounds.max[0], bounds.max[1], bounds.max[2]],
        ]
    }

    fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
        left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
    }
}
