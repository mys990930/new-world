use super::super::chunk::BlockId;
use super::super::registry::BlockRegistry;

#[derive(Debug, Clone, Copy)]
pub(super) struct GenerationPalette {
    pub grass: BlockId,
    pub dirt: BlockId,
    pub stone: BlockId,
    pub sand: BlockId,
    pub gravel: BlockId,
    pub mud: BlockId,
    pub snow: BlockId,
    pub water: BlockId,
}

impl GenerationPalette {
    pub(super) fn from_registry(registry: &BlockRegistry) -> Self {
        let grass = registry.block_id("grass").unwrap_or(BlockId::GRASS);
        let dirt = registry.block_id("dirt").unwrap_or(BlockId::DIRT);
        let stone = registry.block_id("stone").unwrap_or(BlockId::STONE);

        Self {
            grass,
            dirt,
            stone,
            sand: registry.block_id("sand").unwrap_or(dirt),
            gravel: registry.block_id("gravel").unwrap_or(stone),
            mud: registry.block_id("mud").unwrap_or(dirt),
            snow: registry.block_id("snow").unwrap_or(stone),
            water: registry.block_id("water").unwrap_or(BlockId::AIR),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ColumnAtlasSample {
    pub landness: f32,
    pub ocean_distance: f32,
    pub coast_factor: f32,
    pub continent_core_factor: f32,
    pub macro_elevation: f32,
    pub ridge_factor: f32,
    pub mountain_mass: f32,
    pub ruggedness: f32,
    pub river_source_potential: f32,
    pub river_flow_potential: f32,
    pub riverine_factor: f32,
    pub lake_potential: f32,
    pub temperature: f32,
    pub humidity: f32,
    pub aridity: f32,
    pub wetness: f32,
    pub polar_factor: f32,
    pub alpine_factor: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RiverStage {
    Headwaters,
    Middle,
    Lower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ColumnFillProfile {
    DeepOcean,
    ShallowOcean { river_connected: bool },
    River(RiverStage),
    Coast,
    Desert,
    Frozen,
    SoilWithGrassTop,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ColumnRealization {
    pub surface_y: i32,
    pub stone_ceiling_y: i32,
    pub water_top_y: Option<i32>,
    pub fill_profile: ColumnFillProfile,
}
