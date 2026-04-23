mod probe;
mod profile;
mod sampler;
pub mod v2;

use super::chunk::ChunkData;
use super::coord::ChunkCoord;
use super::meta::WorldMeta;
use super::registry::BlockRegistry;
use super::surface::resolve_chunk_surface_plan;

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
    ChunkGenerationV2Inputs, ChunkGenerationV2Scaffold, ChunkRealizationFieldPatch,
    HydrologyColumn, HydrologyMode, HydrologySolve, MesoAppliedColumn, MesoAppliedPrototype,
    PrototypeColumn, VoxelizationColumnPlan,
    REALIZATION_NODE_BLOCK_SPAN, REALIZATION_NODE_CHUNK_SPAN, RealizationFieldNode,
    RealizationSample, RiverCorridorConstraint, SmoothedColumn, SmoothedPrototype,
    V2ScaffoldStage, VoxelizationPlan, build_chunk_base_heightfield_prototype,
    build_chunk_corridor_window, build_chunk_hydrology_solve, build_chunk_meso_applied_prototype,
    build_chunk_meso_applied_prototype_for_feature, build_chunk_realization_field_patch,
    build_chunk_smoothed_prototype, build_chunk_v2_scaffold, build_chunk_voxelization_plan,
    default_voxelization_plan, empty_base_heightfield_prototype, empty_chunk_corridor_window,
    empty_chunk_realization_field_patch, empty_hydrology_solve, empty_meso_applied_prototype,
    empty_smoothed_prototype, prepare_chunk_v2_inputs, sample_chunk_realization_field,
    voxelize_chunk,
};

pub fn generate_chunk(
    coord: ChunkCoord,
    meta: &WorldMeta,
    registry: &BlockRegistry,
) -> ChunkData {
    let inputs = prepare_chunk_v2_inputs(coord, meta);
    let realization = build_chunk_realization_field_patch(coord, &inputs);
    let corridors = build_chunk_corridor_window(coord, &inputs);
    let prototype =
        build_chunk_base_heightfield_prototype(coord, &inputs, &realization, &corridors);
    let meso = build_chunk_meso_applied_prototype(coord, &inputs, &corridors, &prototype);
    let smoothed = build_chunk_smoothed_prototype(coord, &corridors, &meso);
    let hydrology = build_chunk_hydrology_solve(coord, &inputs, &corridors, &smoothed);
    let surface = resolve_chunk_surface_plan(coord, &inputs, &smoothed, &hydrology);
    let voxelization = build_chunk_voxelization_plan(coord, &surface);

    voxelize_chunk(&voxelization, registry)
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
        assert_eq!(a.stage, V2ScaffoldStage::CorridorWindowReady);
        assert_eq!(a.chunk, a.realization_field_patch.chunk);
        assert_eq!(a.chunk, a.corridor_window.chunk);
        assert_eq!(a.inputs.chunk, a.chunk);
        assert_eq!(a.center_region, b.center_region);
    }

    #[test]
    fn generate_chunk_is_deterministic() {
        let meta = WorldMeta::new(42);
        let registry = BlockRegistry::load_default().expect("default registry should load");
        let coord = ChunkCoord(15, 0, 15);

        let a = generate_chunk(coord, &meta, &registry);
        let b = generate_chunk(coord, &meta, &registry);

        assert_eq!(a, b);
        assert_eq!(a.coord(), coord);
    }
}
