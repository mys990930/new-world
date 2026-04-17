mod probe;
mod profile;
mod sampler;
pub mod v2;

use super::chunk::ChunkData;
use super::coord::ChunkCoord;
use super::meta::WorldMeta;
use super::registry::BlockRegistry;

pub const SEA_LEVEL_Y: i32 = 0;
pub const WORLD_FLOOR_Y: i32 = -256;

#[allow(dead_code)]
pub const FLAT_WORLD_SURFACE_Y: i32 = SEA_LEVEL_Y;

pub use probe::{
    ChunkGenerationProbe, ChunkSurfaceLodGrid, ChunkSurfaceLodSample, ColumnAtlasSample,
    ColumnGenerationProbe, TerrainProfileCounts, probe_chunk, probe_column,
    sample_chunk_surface_lod,
};
pub use profile::TerrainProfile;
pub use v2::{
    GENERATOR_LABEL as V2_GENERATOR_LABEL, BaseHeightfieldPrototype, ChunkCorridorWindow,
    ChunkGenerationV2Inputs, ChunkGenerationV2Scaffold, HydrologySolve,
    MesoAppliedPrototype, PrototypeColumn, RiverCorridorConstraint, SmoothedPrototype,
    V2ScaffoldStage, VoxelizationPlan, build_chunk_corridor_window, build_chunk_v2_scaffold,
    default_voxelization_plan, empty_base_heightfield_prototype,
    empty_chunk_corridor_window, empty_hydrology_solve, empty_meso_applied_prototype,
    empty_smoothed_prototype, prepare_chunk_v2_inputs,
};

pub fn generate_chunk(
    _coord: ChunkCoord,
    _meta: &WorldMeta,
    _registry: &BlockRegistry,
) -> ChunkData {
    todo!("V2 chunk realization is not implemented yet; see src/world/generation/status.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_scaffold_is_deterministic() {
        let meta = WorldMeta::new(42);

        let a = build_chunk_v2_scaffold(ChunkCoord(4, 0, -3), &meta);
        let b = build_chunk_v2_scaffold(ChunkCoord(4, 0, -3), &meta);

        assert_eq!(a, b);
        assert_eq!(a.stage, V2ScaffoldStage::RegionClassificationReady);
    }
}
