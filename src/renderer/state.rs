use std::collections::HashMap;

use super::{
    CameraGpuState, ChunkCoord, GpuChunkMesh, PipelineSet, RenderConfig, RenderStats, SurfaceState,
};

#[derive(Debug, Clone)]
pub struct Renderer {
    pub(crate) config: RenderConfig,
    pub(crate) surface: SurfaceState,
    pub(crate) pipelines: PipelineSet,
    pub(crate) world: RenderWorld,
    pub(crate) camera: CameraGpuState,
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

    pub fn camera_gpu_state(&self) -> &CameraGpuState {
        &self.camera
    }

    pub fn last_stats(&self) -> &RenderStats {
        &self.last_stats
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }
}

#[derive(Debug, Clone, Default)]
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
