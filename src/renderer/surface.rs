use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use pollster::block_on;
use winit::window::Window;

use super::{
    state::RendererBackend,
    texture::{
        BlockTextureSet, create_block_texture_bind_group_layout,
        create_gpu_block_texture_resources,
    },
    CameraGpuState, PipelineSet, RenderConfig, RenderStats, RenderWorld, Renderer,
};

pub trait RenderSurfaceTarget {
    fn drawable_size(&self) -> (u32, u32);

    fn debug_name(&self) -> Option<&str> {
        None
    }

    fn owned_window(&self) -> Option<Arc<Window>> {
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StubSurfaceTarget {
    pub width: u32,
    pub height: u32,
    pub debug_name: Option<String>,
}

impl StubSurfaceTarget {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            debug_name: None,
        }
    }

    pub fn with_name(width: u32, height: u32, debug_name: impl Into<String>) -> Self {
        Self {
            width,
            height,
            debug_name: Some(debug_name.into()),
        }
    }
}

impl RenderSurfaceTarget for StubSurfaceTarget {
    fn drawable_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn debug_name(&self) -> Option<&str> {
        self.debug_name.as_deref()
    }
}

impl RenderSurfaceTarget for Arc<Window> {
    fn drawable_size(&self) -> (u32, u32) {
        let size = self.inner_size();
        (size.width, size.height)
    }

    fn debug_name(&self) -> Option<&str> {
        Some("winit_window_surface")
    }

    fn owned_window(&self) -> Option<Arc<Window>> {
        Some(self.clone())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SurfaceSnapshot {
    pub width: u32,
    pub height: u32,
    pub debug_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceState {
    pub snapshot: SurfaceSnapshot,
    pub configured: bool,
    pub minimized: bool,
    pub resize_generation: u64,
    pub present_generation: u64,
}

impl SurfaceState {
    pub fn width(&self) -> u32 {
        self.snapshot.width
    }

    pub fn height(&self) -> u32 {
        self.snapshot.height
    }

    pub fn is_configured(&self) -> bool {
        self.configured
    }

    pub(crate) fn from_target(target: &impl RenderSurfaceTarget) -> Self {
        let snapshot = SurfaceSnapshot {
            width: target.drawable_size().0,
            height: target.drawable_size().1,
            debug_name: target.debug_name().map(ToOwned::to_owned),
        };
        let configured = snapshot.width > 0 && snapshot.height > 0;

        Self {
            minimized: !configured,
            configured,
            snapshot,
            resize_generation: 0,
            present_generation: 0,
        }
    }

    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.snapshot.width = width;
        self.snapshot.height = height;
        self.configured = width > 0 && height > 0;
        self.minimized = !self.configured;
        self.resize_generation = self.resize_generation.saturating_add(1);
    }

    pub(crate) fn mark_presented(&mut self) {
        self.present_generation = self.present_generation.saturating_add(1);
    }
}

#[derive(Debug, Clone)]
pub enum RenderInitError {
    InvalidConfig(&'static str),
    Backend(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSurfaceError {
    InvalidSurfaceSize,
    Lost,
    Outdated,
    Timeout,
    Validation,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct LightUniform {
    pub direction_to_light: [f32; 4],
    pub color: [f32; 4],
    pub ambient: [f32; 4],
}

pub(crate) fn default_directional_light_uniform() -> LightUniform {
    let direction = normalize3([0.35, 1.0, -0.25]);
    LightUniform {
        direction_to_light: [direction[0], direction[1], direction[2], 0.0],
        color: [0.82, 0.82, 0.82, 1.0],
        ambient: [0.22, 0.22, 0.22, 1.0],
    }
}

impl Renderer {
    pub fn new(
        target: &impl RenderSurfaceTarget,
        config: RenderConfig,
    ) -> Result<Self, RenderInitError> {
        config.validate().map_err(RenderInitError::InvalidConfig)?;

        let surface = SurfaceState::from_target(target);
        let pipelines = PipelineSet::new(&config, &surface);
        let camera = CameraGpuState::new(
            &config.camera_projection,
            surface.width(),
            surface.height(),
        )
        .map_err(|_| RenderInitError::InvalidConfig("camera projection config is invalid"))?;
        let block_textures = BlockTextureSet::default();

        let backend = match target.owned_window() {
            Some(window) => Some(block_on(create_backend(
                window,
                &config,
                &surface,
                &block_textures,
            ))?),
            None => None,
        };

        Ok(Self {
            config,
            surface,
            pipelines,
            world: RenderWorld::default(),
            block_textures,
            camera,
            backend,
            last_stats: RenderStats::default(),
            frame_index: 0,
        })
    }

    pub fn attach_window_surface(&mut self, window: Arc<Window>) -> Result<(), RenderInitError> {
        if self.backend.is_some() {
            return Ok(());
        }

        let size = window.inner_size();
        self.surface.resize(size.width, size.height);
        self.pipelines.handle_surface_reconfigured(&self.surface);
        self.camera.handle_resize(size.width, size.height);
        self.backend = Some(block_on(create_backend(
            window,
            &self.config,
            &self.surface,
            &self.block_textures,
        ))?);
        self.rebuild_chunk_mesh_buffers()
            .map_err(|error| RenderInitError::Backend(format!("chunk mesh rebuild failed: {error:?}")))?;
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), RenderSurfaceError> {
        if width == u32::MAX || height == u32::MAX {
            return Err(RenderSurfaceError::InvalidSurfaceSize);
        }

        self.surface.resize(width, height);
        self.pipelines.handle_surface_reconfigured(&self.surface);
        self.camera.handle_resize(width, height);

        if let Some(backend) = self.backend.as_mut() {
            reconfigure_surface_backend(backend, width, height);
        }

        Ok(())
    }
}

async fn create_backend(
    window: Arc<Window>,
    config: &RenderConfig,
    surface: &SurfaceState,
    block_textures: &BlockTextureSet,
) -> Result<RendererBackend, RenderInitError> {
    let instance = wgpu::Instance::default();
    let surface_handle = instance
        .create_surface(window)
        .map_err(|error| RenderInitError::Backend(format!("surface creation failed: {error}")))?;

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface_handle),
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|error| RenderInitError::Backend(format!("adapter request failed: {error}")))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("new_world_renderer_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
        })
        .await
        .map_err(|error| RenderInitError::Backend(format!("device request failed: {error}")))?;

    let mut surface_config = surface_handle
        .get_default_config(&adapter, surface.width().max(1), surface.height().max(1))
        .ok_or_else(|| {
            RenderInitError::Backend("surface is not supported by the selected adapter".to_string())
        })?;

    let capabilities = surface_handle.get_capabilities(&adapter);
    surface_config.format = choose_surface_format(config, &capabilities.formats);
    surface_config.present_mode = choose_present_mode(config, &capabilities.present_modes);
    surface_config.alpha_mode = capabilities
        .alpha_modes
        .first()
        .copied()
        .unwrap_or(wgpu::CompositeAlphaMode::Auto);
    println!(
        "[renderer] selected surface format: {:?}",
        surface_config.format
    );

    if surface.width() > 0 && surface.height() > 0 {
        surface_handle.configure(&device, &surface_config);
    }

    let depth_format = depth_texture_format(config.depth_format);
    let (depth_texture, depth_view) = create_depth_texture(
        &device,
        surface.width().max(1),
        surface.height().max(1),
        depth_format,
    );
    let shader = device.create_shader_module(wgpu::include_wgsl!("player_cube.wgsl"));
    let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("renderer_camera_buffer"),
        size: std::mem::size_of::<super::camera::CameraUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let camera_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("renderer_camera_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
    let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("renderer_camera_bind_group"),
        layout: &camera_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: camera_buffer.as_entire_binding(),
        }],
    });
    let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("renderer_light_buffer"),
        size: std::mem::size_of::<LightUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(
        &light_buffer,
        0,
        bytemuck::cast_slice(&[default_directional_light_uniform()]),
    );
    let light_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("renderer_light_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
    let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("renderer_light_bind_group"),
        layout: &light_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: light_buffer.as_entire_binding(),
        }],
    });
    let block_texture_bind_group_layout = create_block_texture_bind_group_layout(&device);
    let block_textures = create_gpu_block_texture_resources(
        &device,
        &queue,
        &block_texture_bind_group_layout,
        block_textures,
    );
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("renderer_cube_pipeline_layout"),
        bind_group_layouts: &[
            Some(&camera_bind_group_layout),
            Some(&light_bind_group_layout),
            Some(&block_texture_bind_group_layout),
        ],
        immediate_size: 0,
    });
    let cube_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("renderer_cube_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[super::MeshVertex::vertex_buffer_layout()],
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
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    let cube_edge_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("renderer_cube_edge_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[super::MeshVertex::vertex_buffer_layout()],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });

    Ok(RendererBackend {
        surface: surface_handle,
        device,
        queue,
        surface_config,
        depth_format,
        depth_texture,
        depth_view,
        camera_buffer,
        camera_bind_group,
        _light_buffer: light_buffer,
        light_bind_group,
        block_texture_bind_group_layout,
        block_textures,
        cube_pipeline,
        cube_edge_pipeline,
    })
}

fn choose_surface_format(
    config: &RenderConfig,
    supported_formats: &[wgpu::TextureFormat],
) -> wgpu::TextureFormat {
    match config.preferred_surface_format {
        super::SurfaceFormatPolicy::PreferredSrgb => preferred_format_or_first(
            supported_formats,
            &[
                wgpu::TextureFormat::Rgba8UnormSrgb,
                wgpu::TextureFormat::Bgra8UnormSrgb,
            ],
            wgpu::TextureFormat::Rgba8UnormSrgb,
            |format| format.is_srgb(),
        ),
        super::SurfaceFormatPolicy::PreferredLinear => preferred_format_or_first(
            supported_formats,
            &[wgpu::TextureFormat::Rgba8Unorm, wgpu::TextureFormat::Bgra8Unorm],
            wgpu::TextureFormat::Rgba8Unorm,
            |format| !format.is_srgb(),
        ),
    }
}

fn preferred_format_or_first(
    supported_formats: &[wgpu::TextureFormat],
    preferred_formats: &[wgpu::TextureFormat],
    fallback: wgpu::TextureFormat,
    predicate: impl Fn(wgpu::TextureFormat) -> bool,
) -> wgpu::TextureFormat {
    preferred_formats
        .iter()
        .copied()
        .find(|preferred| supported_formats.contains(preferred))
        .or_else(|| supported_formats.iter().copied().find(|format| predicate(*format)))
        .or_else(|| supported_formats.first().copied())
        .unwrap_or(fallback)
}

fn choose_present_mode(
    config: &RenderConfig,
    supported_present_modes: &[wgpu::PresentMode],
) -> wgpu::PresentMode {
    let preferred = match config.preferred_present_mode {
        super::PresentMode::Fifo => wgpu::PresentMode::Fifo,
        super::PresentMode::Mailbox => wgpu::PresentMode::Mailbox,
        super::PresentMode::Immediate => wgpu::PresentMode::Immediate,
    };

    supported_present_modes
        .iter()
        .copied()
        .find(|mode| *mode == preferred)
        .or_else(|| supported_present_modes.first().copied())
        .unwrap_or(wgpu::PresentMode::Fifo)
}

pub(crate) fn reconfigure_surface_backend(
    backend: &mut RendererBackend,
    width: u32,
    height: u32,
) {
    if width == 0 || height == 0 {
        return;
    }

    backend.surface_config.width = width;
    backend.surface_config.height = height;
    backend.surface.configure(&backend.device, &backend.surface_config);
    let (depth_texture, depth_view) =
        create_depth_texture(&backend.device, width, height, backend.depth_format);
    backend.depth_texture = depth_texture;
    backend.depth_view = depth_view;
}

fn depth_texture_format(depth_format: super::DepthFormat) -> wgpu::TextureFormat {
    match depth_format {
        super::DepthFormat::Depth24Plus => wgpu::TextureFormat::Depth24Plus,
        super::DepthFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
    }
}

fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("renderer_depth_texture"),
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

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length_sq = vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2];
    if length_sq <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        let inv_length = length_sq.sqrt().recip();
        [
            vector[0] * inv_length,
            vector[1] * inv_length,
            vector[2] * inv_length,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::{RenderConfig, SurfaceFormatPolicy};

    #[test]
    fn prefers_rgba_srgb_when_available() {
        let config = RenderConfig {
            preferred_surface_format: SurfaceFormatPolicy::PreferredSrgb,
            ..RenderConfig::default()
        };

        let selected = choose_surface_format(
            &config,
            &[
                wgpu::TextureFormat::Bgra8UnormSrgb,
                wgpu::TextureFormat::Rgba8UnormSrgb,
            ],
        );

        assert_eq!(selected, wgpu::TextureFormat::Rgba8UnormSrgb);
    }
}
