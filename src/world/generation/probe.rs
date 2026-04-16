use super::profile::TerrainProfile;
use super::super::atlas::AtlasCoord;
use super::super::coord::ChunkCoord;
use super::super::meta::WorldMeta;

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerrainProfileCounts {
    pub deep_ocean: u32,
    pub shelf: u32,
    pub coast: u32,
    pub plain: u32,
    pub upland: u32,
    pub ridge: u32,
}

impl TerrainProfileCounts {
    pub fn total(self) -> u32 {
        self.deep_ocean + self.shelf + self.coast + self.plain + self.upland + self.ridge
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnGenerationProbe {
    pub chunk: ChunkCoord,
    pub local_x: u8,
    pub local_z: u8,
    pub world_x: i32,
    pub world_z: i32,
    pub atlas_coord: AtlasCoord,
    pub atlas_frac_x: f32,
    pub atlas_frac_z: f32,
    pub atlas_sample: ColumnAtlasSample,
    pub profile: TerrainProfile,
    pub surface_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChunkGenerationProbe {
    pub chunk: ChunkCoord,
    pub surface_min_y: i32,
    pub surface_max_y: i32,
    pub surface_mean_y: f32,
    pub dominant_profile: TerrainProfile,
    pub profile_counts: TerrainProfileCounts,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChunkSurfaceLodSample {
    pub chunk: ChunkCoord,
    pub local_min_x: u8,
    pub local_min_z: u8,
    pub world_min_x: i32,
    pub world_min_z: i32,
    pub span_blocks: u8,
    pub profile: TerrainProfile,
    pub surface_y: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkSurfaceLodGrid {
    pub chunk: ChunkCoord,
    pub step_blocks: u8,
    pub samples_per_axis: u8,
    pub samples: Vec<ChunkSurfaceLodSample>,
}

pub fn probe_column(
    _coord: ChunkCoord,
    _local_x: u8,
    _local_z: u8,
    _meta: &WorldMeta,
) -> ColumnGenerationProbe {
    todo!("V2 column probe is not implemented yet; see src/world/generation/status.md")
}

pub fn probe_chunk(_coord: ChunkCoord, _meta: &WorldMeta) -> ChunkGenerationProbe {
    todo!("V2 chunk probe is not implemented yet; see src/world/generation/status.md")
}

pub fn sample_chunk_surface_lod(
    _coord: ChunkCoord,
    _step_blocks: u8,
    _meta: &WorldMeta,
) -> ChunkSurfaceLodGrid {
    todo!("V2 surface LOD sampling is not implemented yet; see src/world/generation/status.md")
}
