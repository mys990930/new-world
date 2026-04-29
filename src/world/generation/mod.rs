mod corridors;
mod hydrology;
mod inputs;
mod meso_apply;
mod probe;
mod profile;
mod prototype;
mod realization_field;
mod sampler;
mod smoothing;
mod voxelize;

use super::chunk::ChunkData;
use super::coord::ChunkCoord;
use super::meta::WorldMeta;
use super::registry::BlockRegistry;
use super::surface::resolve_chunk_surface_plan;
use crate::world::atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCell, AtlasCoord, AtlasFieldMap, RegionClassCell,
    RegionClassMap, sample_region_class_influences,
};
use crate::world::coord::CHUNK_EDGE_I32;

pub const SEA_LEVEL_Y: i32 = 0;
pub const WORLD_FLOOR_Y: i32 = -256;
pub const GENERATOR_LABEL: &str = "generation_pipeline";
const ATLAS_CELL_BLOCK_SPAN: f32 = (ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32) as f32;

#[allow(dead_code)]
pub const FLAT_WORLD_SURFACE_Y: i32 = SEA_LEVEL_Y;

pub use corridors::{
    ChunkCorridorWindow, RiverCorridorConstraint, build_chunk_corridor_window,
    empty_chunk_corridor_window,
};
pub use hydrology::{
    HydrologyColumn, HydrologyMode, HydrologySolve, build_chunk_hydrology_solve,
    empty_hydrology_solve,
};
pub use inputs::{
    ChunkGenerationInputCache, ChunkGenerationInputs, ChunkGenerationScaffold,
    GenerationScaffoldStage, build_chunk_generation_scaffold, chunk_generation_input_area,
    prepare_chunk_generation_inputs,
};
pub use meso_apply::{
    MesoAppliedColumn, MesoAppliedPrototype, build_chunk_meso_applied_prototype,
    build_chunk_meso_applied_prototype_for_feature, empty_meso_applied_prototype,
};
pub use probe::{
    ChunkGenerationProbe, ChunkSurfaceLodGrid, ChunkSurfaceLodSample, ColumnAtlasSample,
    ColumnGenerationProbe, TerrainProfileCounts, probe_chunk, probe_column,
    sample_chunk_surface_lod,
};
pub use profile::TerrainProfile;
pub use prototype::{
    BaseHeightfieldPrototype, PrototypeColumn, build_chunk_base_heightfield_prototype,
    empty_base_heightfield_prototype,
};
pub use realization_field::{
    ChunkRealizationFieldPatch, REALIZATION_NODE_BLOCK_SPAN, REALIZATION_NODE_CHUNK_SPAN,
    RealizationFieldNode, RealizationMaterialSupport, RealizationSample,
    build_chunk_realization_field_patch, empty_chunk_realization_field_patch,
    sample_chunk_realization_field,
};
pub use smoothing::{
    SmoothedColumn, SmoothedPrototype, build_chunk_smoothed_prototype, empty_smoothed_prototype,
};
pub use voxelize::{
    VoxelizationColumnPlan, VoxelizationPlan, build_chunk_voxelization_plan,
    default_voxelization_plan, voxelize_chunk, voxelize_chunk_at_coord,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct RegionSampleWeight {
    pub cell: RegionClassCell,
    pub weight: f32,
}

pub(super) fn sample_atlas_fields_fractional(
    fields: &AtlasFieldMap,
    world_x: f32,
    world_z: f32,
) -> AtlasCell {
    let (atlas_x, atlas_z) = atlas_sample_position(world_x, world_z);
    let (base_x, base_z, east_x, south_z, frac_x, frac_z) =
        fractional_sample_window(fields.area(), atlas_x, atlas_z);
    let c00 = fields
        .get(AtlasCoord::new(base_x, base_z))
        .expect("fractional atlas sample must exist");
    let c10 = fields
        .get(AtlasCoord::new(east_x, base_z))
        .expect("fractional atlas east sample must exist");
    let c01 = fields
        .get(AtlasCoord::new(base_x, south_z))
        .expect("fractional atlas south sample must exist");
    let c11 = fields
        .get(AtlasCoord::new(east_x, south_z))
        .expect("fractional atlas southeast sample must exist");
    let weights = bilerp_weights(frac_x, frac_z);
    let dominant = dominant_corner_index(weights, c00, c10, c01, c11);

    AtlasCell {
        landness: bilerp(
            c00.landness,
            c10.landness,
            c01.landness,
            c11.landness,
            frac_x,
            frac_z,
        ),
        ocean_distance: bilerp(
            c00.ocean_distance,
            c10.ocean_distance,
            c01.ocean_distance,
            c11.ocean_distance,
            frac_x,
            frac_z,
        ),
        coast_distance: bilerp(
            c00.coast_distance,
            c10.coast_distance,
            c01.coast_distance,
            c11.coast_distance,
            frac_x,
            frac_z,
        ),
        continent_id: dominant.continent_id,
        continent_core_factor: bilerp(
            c00.continent_core_factor,
            c10.continent_core_factor,
            c01.continent_core_factor,
            c11.continent_core_factor,
            frac_x,
            frac_z,
        ),
        macro_elevation: bilerp(
            c00.macro_elevation,
            c10.macro_elevation,
            c01.macro_elevation,
            c11.macro_elevation,
            frac_x,
            frac_z,
        ),
        slope: bilerp(c00.slope, c10.slope, c01.slope, c11.slope, frac_x, frac_z),
        ruggedness: bilerp(
            c00.ruggedness,
            c10.ruggedness,
            c01.ruggedness,
            c11.ruggedness,
            frac_x,
            frac_z,
        ),
        ridge_factor: bilerp(
            c00.ridge_factor,
            c10.ridge_factor,
            c01.ridge_factor,
            c11.ridge_factor,
            frac_x,
            frac_z,
        ),
        mountain_mass: bilerp(
            c00.mountain_mass,
            c10.mountain_mass,
            c01.mountain_mass,
            c11.mountain_mass,
            frac_x,
            frac_z,
        ),
        basinness: bilerp(
            c00.basinness,
            c10.basinness,
            c01.basinness,
            c11.basinness,
            frac_x,
            frac_z,
        ),
        pass_potential: bilerp(
            c00.pass_potential,
            c10.pass_potential,
            c01.pass_potential,
            c11.pass_potential,
            frac_x,
            frac_z,
        ),
        river_source_potential: bilerp(
            c00.river_source_potential,
            c10.river_source_potential,
            c01.river_source_potential,
            c11.river_source_potential,
            frac_x,
            frac_z,
        ),
        river_flow_potential: bilerp(
            c00.river_flow_potential,
            c10.river_flow_potential,
            c01.river_flow_potential,
            c11.river_flow_potential,
            frac_x,
            frac_z,
        ),
        river_distance_estimate: bilerp(
            c00.river_distance_estimate,
            c10.river_distance_estimate,
            c01.river_distance_estimate,
            c11.river_distance_estimate,
            frac_x,
            frac_z,
        ),
        lake_potential: bilerp(
            c00.lake_potential,
            c10.lake_potential,
            c01.lake_potential,
            c11.lake_potential,
            frac_x,
            frac_z,
        ),
        temperature: bilerp(
            c00.temperature,
            c10.temperature,
            c01.temperature,
            c11.temperature,
            frac_x,
            frac_z,
        ),
        humidity: bilerp(
            c00.humidity,
            c10.humidity,
            c01.humidity,
            c11.humidity,
            frac_x,
            frac_z,
        ),
        inlandness: bilerp(
            c00.inlandness,
            c10.inlandness,
            c01.inlandness,
            c11.inlandness,
            frac_x,
            frac_z,
        ),
        aridity: bilerp(
            c00.aridity,
            c10.aridity,
            c01.aridity,
            c11.aridity,
            frac_x,
            frac_z,
        ),
        wetness: bilerp(
            c00.wetness,
            c10.wetness,
            c01.wetness,
            c11.wetness,
            frac_x,
            frac_z,
        ),
        polar_factor: bilerp(
            c00.polar_factor,
            c10.polar_factor,
            c01.polar_factor,
            c11.polar_factor,
            frac_x,
            frac_z,
        ),
        alpine_factor: bilerp(
            c00.alpine_factor,
            c10.alpine_factor,
            c01.alpine_factor,
            c11.alpine_factor,
            frac_x,
            frac_z,
        ),
        coast_factor: bilerp(
            c00.coast_factor,
            c10.coast_factor,
            c01.coast_factor,
            c11.coast_factor,
            frac_x,
            frac_z,
        ),
        wetland_factor: bilerp(
            c00.wetland_factor,
            c10.wetland_factor,
            c01.wetland_factor,
            c11.wetland_factor,
            frac_x,
            frac_z,
        ),
        riverine_factor: bilerp(
            c00.riverine_factor,
            c10.riverine_factor,
            c01.riverine_factor,
            c11.riverine_factor,
            frac_x,
            frac_z,
        ),
        ecotone_strength: bilerp(
            c00.ecotone_strength,
            c10.ecotone_strength,
            c01.ecotone_strength,
            c11.ecotone_strength,
            frac_x,
            frac_z,
        ),
        thermal: dominant.thermal,
        moisture: dominant.moisture,
        form: dominant.form,
        overlay: dominant.overlay,
        cover: dominant.cover,
    }
}

pub(super) fn sample_region_weights(
    classes: &RegionClassMap,
    world_x: f32,
    world_z: f32,
) -> Vec<RegionSampleWeight> {
    let influences = sample_region_class_influences(classes, world_x, world_z);
    let mut samples = Vec::with_capacity(1 + influences.neighbors.len());
    let mut total_weight = 0.0_f32;

    if influences.dominant.weight > f32::EPSILON {
        samples.push(RegionSampleWeight {
            cell: influences.dominant.class,
            weight: influences.dominant.weight,
        });
        total_weight += influences.dominant.weight;
    }

    for neighbor in influences.neighbors {
        if neighbor.weight <= f32::EPSILON {
            continue;
        }

        samples.push(RegionSampleWeight {
            cell: neighbor.class,
            weight: neighbor.weight,
        });
        total_weight += neighbor.weight;
    }

    if total_weight <= f32::EPSILON {
        return vec![RegionSampleWeight {
            cell: RegionClassCell::default(),
            weight: 1.0,
        }];
    }

    let inv_total = total_weight.recip();
    for sample in &mut samples {
        sample.weight *= inv_total;
    }

    samples
}

fn atlas_sample_position(world_x: f32, world_z: f32) -> (f32, f32) {
    (
        world_x / ATLAS_CELL_BLOCK_SPAN,
        world_z / ATLAS_CELL_BLOCK_SPAN,
    )
}

fn fractional_sample_window(
    area: AtlasArea,
    sample_x: f32,
    sample_z: f32,
) -> (i32, i32, i32, i32, f32, f32) {
    let min_x = area.origin().x;
    let min_z = area.origin().z;
    let max_x = area.origin().x + area.width() as i32 - 1;
    let max_z = area.origin().z + area.height() as i32 - 1;
    let base_x = sample_x.floor() as i32;
    let base_z = sample_z.floor() as i32;
    let clamped_base_x = base_x.clamp(min_x, max_x);
    let clamped_base_z = base_z.clamp(min_z, max_z);
    let east_x = (clamped_base_x + 1).min(max_x);
    let south_z = (clamped_base_z + 1).min(max_z);
    let frac_x = (sample_x - clamped_base_x as f32).clamp(0.0, 1.0);
    let frac_z = (sample_z - clamped_base_z as f32).clamp(0.0, 1.0);

    (
        clamped_base_x,
        clamped_base_z,
        east_x,
        south_z,
        frac_x,
        frac_z,
    )
}

fn bilerp(a00: f32, a10: f32, a01: f32, a11: f32, tx: f32, tz: f32) -> f32 {
    let smooth_x = smootherstep(tx);
    let smooth_z = smootherstep(tz);
    let top = a00 + (a10 - a00) * smooth_x;
    let bottom = a01 + (a11 - a01) * smooth_x;
    top + (bottom - top) * smooth_z
}

fn bilerp_weights(tx: f32, tz: f32) -> (f32, f32, f32, f32) {
    let smooth_x = smootherstep(tx);
    let smooth_z = smootherstep(tz);
    let inv_x = 1.0 - smooth_x;
    let inv_z = 1.0 - smooth_z;
    (
        inv_x * inv_z,
        smooth_x * inv_z,
        inv_x * smooth_z,
        smooth_x * smooth_z,
    )
}

fn smootherstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn dominant_corner_index<'a>(
    weights: (f32, f32, f32, f32),
    c00: &'a AtlasCell,
    c10: &'a AtlasCell,
    c01: &'a AtlasCell,
    c11: &'a AtlasCell,
) -> &'a AtlasCell {
    let (w00, w10, w01, w11) = weights;
    if w10 > w00 && w10 >= w01 && w10 >= w11 {
        c10
    } else if w01 > w00 && w01 >= w10 && w01 >= w11 {
        c01
    } else if w11 > w00 && w11 >= w10 && w11 >= w01 {
        c11
    } else {
        c00
    }
}

pub fn generate_chunk(coord: ChunkCoord, meta: &WorldMeta, registry: &BlockRegistry) -> ChunkData {
    let inputs = prepare_chunk_generation_inputs(coord, meta);
    generate_chunk_from_generation_inputs(&inputs, registry)
}

pub fn generate_chunk_from_generation_inputs(
    inputs: &ChunkGenerationInputs,
    registry: &BlockRegistry,
) -> ChunkData {
    let voxelization = build_chunk_generation_voxelization_plan(inputs);

    generate_chunk_from_voxelization_plan(inputs.chunk, &voxelization, registry)
}

pub fn build_chunk_generation_voxelization_plan(
    inputs: &ChunkGenerationInputs,
) -> VoxelizationPlan {
    let coord = inputs.chunk;
    let realization = build_chunk_realization_field_patch(coord, inputs);
    let corridors = build_chunk_corridor_window(coord, inputs);
    let prototype = build_chunk_base_heightfield_prototype(coord, inputs, &realization, &corridors);
    let meso = build_chunk_meso_applied_prototype(coord, inputs, &corridors, &prototype);
    let smoothed = build_chunk_smoothed_prototype(coord, &corridors, &meso);
    let hydrology = build_chunk_hydrology_solve(coord, inputs, &corridors, &smoothed);
    let surface = resolve_chunk_surface_plan(coord, inputs, &smoothed, &hydrology);

    build_chunk_voxelization_plan(coord, &surface)
}

pub fn generate_chunk_from_voxelization_plan(
    coord: ChunkCoord,
    plan: &VoxelizationPlan,
    registry: &BlockRegistry,
) -> ChunkData {
    voxelize_chunk_at_coord(coord, plan, registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "slow generation terrain scaffold smoke test"]
    fn generation_scaffold_is_deterministic() {
        let meta = WorldMeta::new(42);

        let a = build_chunk_generation_scaffold(ChunkCoord(4, 0, -3), &meta);
        let b = build_chunk_generation_scaffold(ChunkCoord(4, 0, -3), &meta);

        assert_eq!(a, b);
        assert_eq!(a.stage, GenerationScaffoldStage::CorridorWindowReady);
        assert_eq!(a.chunk, a.realization_field_patch.chunk);
        assert_eq!(a.chunk, a.corridor_window.chunk);
        assert_eq!(a.inputs.chunk, a.chunk);
        assert_eq!(a.center_region, b.center_region);
    }

    #[test]
    #[ignore = "slow end-to-end generation chunk smoke test"]
    fn generate_chunk_is_deterministic() {
        let meta = WorldMeta::new(42);
        let registry = BlockRegistry::load_default().expect("default registry should load");
        let coord = ChunkCoord(15, 0, 15);

        let a = generate_chunk(coord, &meta, &registry);
        let b = generate_chunk(coord, &meta, &registry);

        assert_eq!(a, b);
        assert_eq!(a.coord(), coord);
    }

    #[test]
    #[ignore = "slow end-to-end generation stack smoke test"]
    fn voxelization_plan_can_be_reused_across_vertical_chunks() {
        let meta = WorldMeta::new(42);
        let registry = BlockRegistry::load_default().expect("default registry should load");
        let plan_coord = ChunkCoord(15, 0, 15);
        let inputs = prepare_chunk_generation_inputs(plan_coord, &meta);
        let plan = build_chunk_generation_voxelization_plan(&inputs);

        for chunk_y in [-2, 0, 3] {
            let coord = ChunkCoord(plan_coord.0, chunk_y, plan_coord.2);
            let direct = generate_chunk(coord, &meta, &registry);
            let from_plan = generate_chunk_from_voxelization_plan(coord, &plan, &registry);

            assert_eq!(from_plan, direct);
        }
    }
}
