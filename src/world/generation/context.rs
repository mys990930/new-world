use super::profile::TerrainProfile;
use super::super::chunk::BlockId;
use super::super::registry::BlockRegistry;

#[derive(Debug, Clone, Copy)]
pub(super) struct GenerationPalette {
    pub stone: BlockId,
}

impl GenerationPalette {
    pub(super) fn from_registry(registry: &BlockRegistry) -> Self {
        Self {
            stone: registry.block_id("stone").unwrap_or(BlockId::STONE),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ColumnAtlasSample {
    pub landness: f32,
    pub ocean_distance: f32,
    pub coast_factor: f32,
    pub continent_core_factor: f32,
    pub macro_elevation: f32,
    pub ridge_factor: f32,
    pub mountain_mass: f32,
    pub ruggedness: f32,
    pub riverine_factor: f32,
    pub alpine_factor: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ColumnRealization {
    pub surface_y: i32,
    #[allow(dead_code)]
    pub profile: TerrainProfile,
}
