use super::{RenderConfig, SurfaceState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineKind {
    TerrainOpaque,
    DynamicOpaque,
    DebugOverlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineState {
    pub label: &'static str,
    pub kind: PipelineKind,
    pub sample_count: u32,
    pub depth_enabled: bool,
    pub rebuild_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineSet {
    pub terrain_opaque: PipelineState,
    pub dynamic_opaque: PipelineState,
    pub debug_overlay: Option<PipelineState>,
}

impl PipelineSet {
    pub(crate) fn new(config: &RenderConfig, surface: &SurfaceState) -> Self {
        let base_generation = surface.resize_generation;
        let terrain_opaque = PipelineState {
            label: "terrain_opaque",
            kind: PipelineKind::TerrainOpaque,
            sample_count: config.sample_count,
            depth_enabled: true,
            rebuild_generation: base_generation,
        };
        let dynamic_opaque = PipelineState {
            label: "dynamic_opaque",
            kind: PipelineKind::DynamicOpaque,
            sample_count: config.sample_count,
            depth_enabled: true,
            rebuild_generation: base_generation,
        };
        let debug_overlay = config.debug.debug_overlay.then_some(PipelineState {
            label: "debug_overlay",
            kind: PipelineKind::DebugOverlay,
            sample_count: config.sample_count,
            depth_enabled: false,
            rebuild_generation: base_generation,
        });

        Self {
            terrain_opaque,
            dynamic_opaque,
            debug_overlay,
        }
    }

    pub(crate) fn handle_surface_reconfigured(&mut self, surface: &SurfaceState) {
        self.terrain_opaque.rebuild_generation = surface.resize_generation;
        self.dynamic_opaque.rebuild_generation = surface.resize_generation;
        if let Some(debug_overlay) = self.debug_overlay.as_mut() {
            debug_overlay.rebuild_generation = surface.resize_generation;
        }
    }
}
