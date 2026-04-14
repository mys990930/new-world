use std::collections::HashMap;

use super::{
    texture::{BlockTextureSet, GpuBlockTextureResources},
    CameraGpuState, ChunkCoord, ClearColor, GpuChunkMesh, PipelineSet, RenderConfig,
    RenderEnvironment, RenderStats, SurfaceState,
};

pub struct Renderer {
    pub(crate) config: RenderConfig,
    pub(crate) surface: SurfaceState,
    pub(crate) pipelines: PipelineSet,
    pub(crate) world: RenderWorld,
    pub(crate) block_textures: BlockTextureSet,
    pub(crate) environment: RenderEnvironmentState,
    pub(crate) camera: CameraGpuState,
    pub(crate) backend: Option<RendererBackend>,
    pub(crate) last_stats: RenderStats,
    pub(crate) frame_index: u64,
}

impl Renderer {
    pub fn config(&self) -> &RenderConfig {
        &self.config
    }

    pub fn surface_state(&self) -> &SurfaceState {
        &self.surface
    }

    pub fn pipelines(&self) -> &PipelineSet {
        &self.pipelines
    }

    pub fn world(&self) -> &RenderWorld {
        &self.world
    }

    pub fn environment(&self) -> &RenderEnvironmentState {
        &self.environment
    }

    pub fn set_environment(&mut self, environment: RenderEnvironment) {
        self.environment.set(environment);
    }

    pub fn camera_gpu_state(&self) -> &CameraGpuState {
        &self.camera
    }

    pub fn last_stats(&self) -> &RenderStats {
        &self.last_stats
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    pub fn has_live_backend(&self) -> bool {
        self.backend.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderEnvironmentState {
    current: RenderEnvironment,
    generation: u64,
}

impl RenderEnvironmentState {
    pub(crate) fn from_config(config: &RenderConfig) -> Self {
        Self {
            current: config.environment,
            generation: 0,
        }
    }

    pub fn current(&self) -> &RenderEnvironment {
        &self.current
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn resolved_clear_color(&self, fallback: ClearColor) -> [f32; 4] {
        self.current.resolved_clear_color(fallback)
    }

    pub(crate) fn set(&mut self, next: RenderEnvironment) {
        if self.current != next {
            self.current = next;
            self.generation = self.generation.saturating_add(1);
        }
    }
}

#[derive(Default)]
pub struct RenderWorld {
    pub(crate) chunk_meshes: HashMap<ChunkCoord, GpuChunkMesh>,
    pub(crate) uploaded_this_frame: u32,
    pub(crate) removed_this_frame: u32,
    pub(crate) generation: u64,
}

impl RenderWorld {
    pub fn mesh_count(&self) -> usize {
        self.chunk_meshes.len()
    }

    pub fn chunk_mesh(&self, coord: ChunkCoord) -> Option<&GpuChunkMesh> {
        self.chunk_meshes.get(&coord)
    }

    pub(crate) fn finish_frame(&mut self) -> (u32, u32) {
        let uploaded = self.uploaded_this_frame;
        let removed = self.removed_this_frame;
        self.uploaded_this_frame = 0;
        self.removed_this_frame = 0;
        (uploaded, removed)
    }
}

pub(crate) struct RendererBackend {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) surface_config: wgpu::SurfaceConfiguration,
    pub(crate) depth_format: wgpu::TextureFormat,
    pub(crate) depth_texture: wgpu::Texture,
    pub(crate) depth_view: wgpu::TextureView,
    pub(crate) camera_buffer: wgpu::Buffer,
    pub(crate) camera_bind_group: wgpu::BindGroup,
    pub(crate) environment_buffer: wgpu::Buffer,
    pub(crate) environment_bind_group: wgpu::BindGroup,
    pub(crate) shadow_uniform_buffer: wgpu::Buffer,
    pub(crate) shadow_pass_bind_group: wgpu::BindGroup,
    pub(crate) shadow_sampling_bind_group: wgpu::BindGroup,
    pub(crate) _shadow_map_size: u32,
    pub(crate) _shadow_map_texture: wgpu::Texture,
    pub(crate) shadow_map_view: wgpu::TextureView,
    pub(crate) _shadow_map_sampler: wgpu::Sampler,
    pub(crate) block_texture_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) block_textures: GpuBlockTextureResources,
    pub(crate) sun_overlay_pipeline: wgpu::RenderPipeline,
    pub(crate) terrain_pipeline: wgpu::RenderPipeline,
    pub(crate) water_pipeline: wgpu::RenderPipeline,
    pub(crate) dynamic_cube_pipeline: wgpu::RenderPipeline,
    pub(crate) shadow_depth_pipeline: wgpu::RenderPipeline,
    pub(crate) ui_rect_pipeline: wgpu::RenderPipeline,
    pub(crate) debug_edge_pipeline: wgpu::RenderPipeline,
}
