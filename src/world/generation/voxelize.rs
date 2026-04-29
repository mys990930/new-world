use crate::world::chunk::{BlockId, ChunkData};
use crate::world::coord::{CHUNK_EDGE, CHUNK_EDGE_I32, CHUNK_VOLUME, ChunkCoord};
use crate::world::registry::BlockRegistry;
use crate::world::surface::{ChunkSurfacePlan, SurfaceColumnPlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelizationColumnPlan {
    pub owner_archetype: crate::world::RegionArchetype,
    pub material_policy: crate::world::MaterialPolicyId,
    pub seasonal_state: Option<crate::world::SeasonalBiomeStateId>,
    pub cover_phase: crate::world::CoverPhase,
    pub cover_override_key: Option<&'static str>,
    pub terrain_top_y: i32,
    pub water_top_y: Option<i32>,
    pub top_block_key: &'static str,
    pub filler_block_key: &'static str,
    pub core_block_key: &'static str,
    pub water_block_key: Option<&'static str>,
    pub filler_depth: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelizationPlan {
    pub chunk: ChunkCoord,
    pub columns: Vec<VoxelizationColumnPlan>,
}

impl From<SurfaceColumnPlan> for VoxelizationColumnPlan {
    fn from(plan: SurfaceColumnPlan) -> Self {
        Self {
            owner_archetype: plan.owner_archetype,
            material_policy: plan.material_policy,
            seasonal_state: plan.seasonal_state,
            cover_phase: plan.cover_phase,
            cover_override_key: plan.cover_override_key,
            terrain_top_y: plan.terrain_top_y,
            water_top_y: plan.water_top_y,
            top_block_key: plan.top_block_key,
            filler_block_key: plan.filler_block_key,
            core_block_key: plan.core_block_key,
            water_block_key: plan.water_block_key,
            filler_depth: plan.filler_depth,
        }
    }
}

pub fn default_voxelization_plan(chunk: ChunkCoord) -> VoxelizationPlan {
    VoxelizationPlan {
        chunk,
        columns: Vec::new(),
    }
}

pub fn build_chunk_voxelization_plan(
    chunk: ChunkCoord,
    surface: &ChunkSurfacePlan,
) -> VoxelizationPlan {
    debug_assert_eq!(surface.chunk, chunk);

    VoxelizationPlan {
        chunk,
        columns: surface.columns.iter().copied().map(Into::into).collect(),
    }
}

pub fn voxelize_chunk(plan: &VoxelizationPlan, registry: &BlockRegistry) -> ChunkData {
    voxelize_chunk_at_coord(plan.chunk, plan, registry)
}

pub fn voxelize_chunk_at_coord(
    coord: ChunkCoord,
    plan: &VoxelizationPlan,
    registry: &BlockRegistry,
) -> ChunkData {
    debug_assert_eq!(plan.chunk.0, coord.0);
    debug_assert_eq!(plan.chunk.2, coord.2);

    if is_chunk_range_above_plan(coord, plan) {
        return ChunkData::new_empty(coord);
    }

    let chunk_min_y = coord.1 * CHUNK_EDGE_I32;
    let mut blocks = vec![BlockId::AIR; CHUNK_VOLUME];

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let column = plan.columns[column_index(local_x, local_z)];
            let top_id = block_id_or_panic(registry, column.top_block_key);
            let filler_id = block_id_or_panic(registry, column.filler_block_key);
            let core_id = block_id_or_panic(registry, column.core_block_key);
            let water_id = column
                .water_block_key
                .map(|key| block_id_or_panic(registry, key));

            for local_y in 0..CHUNK_EDGE_I32 {
                let world_y = chunk_min_y + local_y;
                let block = if world_y <= column.terrain_top_y {
                    if world_y == column.terrain_top_y {
                        top_id
                    } else if world_y >= column.terrain_top_y - i32::from(column.filler_depth) {
                        filler_id
                    } else {
                        core_id
                    }
                } else if column
                    .water_top_y
                    .is_some_and(|water_top_y| world_y <= water_top_y)
                {
                    water_id.unwrap_or(BlockId::AIR)
                } else {
                    BlockId::AIR
                };

                blocks[linear_index(local_x as usize, local_y as usize, local_z as usize)] = block;
            }
        }
    }

    ChunkData::from_blocks(coord, blocks)
}

fn block_id_or_panic(registry: &BlockRegistry, key: &str) -> BlockId {
    registry
        .block_id(key)
        .unwrap_or_else(|| panic!("voxelize requires block key `{key}` to exist in the registry"))
}

fn column_index(local_x: i32, local_z: i32) -> usize {
    local_z as usize * CHUNK_EDGE + local_x as usize
}

fn linear_index(x: usize, y: usize, z: usize) -> usize {
    x + z * CHUNK_EDGE + y * CHUNK_EDGE * CHUNK_EDGE
}

fn is_chunk_range_above_plan(coord: ChunkCoord, plan: &VoxelizationPlan) -> bool {
    let chunk_min_y = coord.1 * CHUNK_EDGE_I32;

    plan.columns
        .iter()
        .all(|column| column_top_non_air_y(*column) < chunk_min_y)
}

fn column_top_non_air_y(column: VoxelizationColumnPlan) -> i32 {
    column
        .water_top_y
        .unwrap_or(i32::MIN)
        .max(column.terrain_top_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::{
        build_chunk_base_heightfield_prototype, build_chunk_corridor_window,
        build_chunk_hydrology_solve, build_chunk_meso_applied_prototype,
        build_chunk_realization_field_patch, build_chunk_smoothed_prototype,
        prepare_chunk_generation_inputs,
    };
    use crate::world::meta::WorldMeta;
    use crate::world::registry::BlockRegistry;
    use crate::world::surface::resolve_chunk_surface_plan;
    use crate::world::{HydrologyMode, LocalBlockCoord};

    fn build_voxelization(
        chunk: ChunkCoord,
        meta: &WorldMeta,
        registry: &BlockRegistry,
    ) -> (
        crate::world::HydrologySolve,
        ChunkSurfacePlan,
        VoxelizationPlan,
        ChunkData,
    ) {
        let inputs = prepare_chunk_generation_inputs(chunk, meta);
        let realization = build_chunk_realization_field_patch(chunk, &inputs);
        let corridors = build_chunk_corridor_window(chunk, &inputs);
        let prototype =
            build_chunk_base_heightfield_prototype(chunk, &inputs, &realization, &corridors);
        let meso = build_chunk_meso_applied_prototype(chunk, &inputs, &corridors, &prototype);
        let smoothed = build_chunk_smoothed_prototype(chunk, &corridors, &meso);
        let hydrology = build_chunk_hydrology_solve(chunk, &inputs, &corridors, &smoothed);
        let surface = resolve_chunk_surface_plan(chunk, &inputs, &smoothed, &hydrology);
        let plan = build_chunk_voxelization_plan(chunk, &surface);
        let chunk_data = voxelize_chunk(&plan, registry);

        (hydrology, surface, plan, chunk_data)
    }

    fn find_chunk_with_visible_water(
        meta: &WorldMeta,
        registry: &BlockRegistry,
    ) -> (
        crate::world::HydrologySolve,
        ChunkSurfacePlan,
        VoxelizationPlan,
        ChunkData,
    ) {
        let seed_chunks = [
            ChunkCoord(40, 0, -29),
            ChunkCoord(39, 0, -29),
            ChunkCoord(40, 0, -30),
            ChunkCoord(4, 0, -3),
            ChunkCoord(0, 0, 0),
            ChunkCoord(15, 0, 15),
        ];

        for seed in seed_chunks {
            for offset_z in -2..=2 {
                for offset_x in -2..=2 {
                    let candidate = ChunkCoord(seed.0 + offset_x, 0, seed.2 + offset_z);
                    let built = build_voxelization(candidate, meta, registry);
                    let water_columns = built
                        .0
                        .columns
                        .iter()
                        .filter(|column| column.water_surface_height.is_some())
                        .count();
                    if water_columns >= 8 {
                        return built;
                    }
                }
            }
        }

        panic!("expected at least one sampled chunk to contain visible water");
    }

    fn synthetic_column(terrain_top_y: i32, water_top_y: Option<i32>) -> VoxelizationColumnPlan {
        VoxelizationColumnPlan {
            owner_archetype: crate::world::RegionArchetype::TemperatePlain,
            material_policy: crate::world::MaterialPolicyId::TemperateGrassland,
            seasonal_state: None,
            cover_phase: crate::world::CoverPhase::Growing,
            cover_override_key: None,
            terrain_top_y,
            water_top_y,
            top_block_key: "grass",
            filler_block_key: "dirt",
            core_block_key: "stone",
            water_block_key: water_top_y.map(|_| "water"),
            filler_depth: 4,
        }
    }

    fn synthetic_plan(chunk: ChunkCoord, column: VoxelizationColumnPlan) -> VoxelizationPlan {
        VoxelizationPlan {
            chunk,
            columns: vec![column; CHUNK_EDGE * CHUNK_EDGE],
        }
    }

    #[test]
    fn voxelize_chunk_at_coord_uses_requested_vertical_chunk() {
        let registry = BlockRegistry::load_default().expect("default registry should load");
        let plan = synthetic_plan(ChunkCoord(2, 0, -3), synthetic_column(10, None));

        let chunk = voxelize_chunk_at_coord(ChunkCoord(2, 1, -3), &plan, &registry);

        assert_eq!(chunk.coord(), ChunkCoord(2, 1, -3));
        assert_eq!(chunk.snapshot().uniform_block(), Some(BlockId::AIR));
    }

    #[test]
    #[ignore = "slow current generation voxelization pipeline smoke test"]
    fn voxelization_plan_is_deterministic() {
        let meta = WorldMeta::new(42);
        let registry = BlockRegistry::load_default().expect("default registry should load");
        let chunk = ChunkCoord(15, 0, 15);

        let (_, _, a_plan, a_chunk) = build_voxelization(chunk, &meta, &registry);
        let (_, _, b_plan, b_chunk) = build_voxelization(chunk, &meta, &registry);

        assert_eq!(a_plan, b_plan);
        assert_eq!(a_chunk, b_chunk);
        assert_eq!(
            a_plan.columns.len(),
            (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize
        );
    }

    #[test]
    #[ignore = "slow current generation voxelization pipeline smoke test"]
    fn voxelization_emits_solid_ground_for_dry_columns() {
        let meta = WorldMeta::new(42);
        let registry = BlockRegistry::load_default().expect("default registry should load");
        let chunk = ChunkCoord(15, 0, 15);
        let (_, surface, _, chunk_data) = build_voxelization(chunk, &meta, &registry);

        let dry_index = surface
            .columns
            .iter()
            .position(|column| column.water_top_y.is_none())
            .expect("expected at least one dry surface column");
        let local_x = (dry_index % CHUNK_EDGE) as u8;
        let local_z = (dry_index / CHUNK_EDGE) as u8;
        let world_y = surface.columns[dry_index].terrain_top_y - chunk.1 * CHUNK_EDGE_I32;
        let local_y =
            u8::try_from(world_y).expect("dry surface should land inside the sampled chunk");
        let local =
            LocalBlockCoord::new(local_x, local_y, local_z).expect("surface coord should be valid");

        let block = chunk_data
            .get_block(local)
            .expect("surface block should be addressable");

        assert_ne!(block, BlockId::AIR);
    }

    #[test]
    #[ignore = "slow current generation voxelization water search smoke test"]
    fn voxelization_places_water_or_ice_above_hydrology_channels() {
        let meta = WorldMeta::new(42);
        let registry = BlockRegistry::load_default().expect("default registry should load");
        let (hydrology, surface, _, chunk_data) = find_chunk_with_visible_water(&meta, &registry);

        let water_index = hydrology
            .columns
            .iter()
            .enumerate()
            .find(|(_, column)| {
                column.water_surface_height.is_some()
                    && matches!(
                        column.mode,
                        HydrologyMode::Channel
                            | HydrologyMode::Floodplain
                            | HydrologyMode::Lake
                            | HydrologyMode::Wetland
                    )
            })
            .map(|(index, _)| index)
            .expect("expected a visible hydrology water column");
        let column = &surface.columns[water_index];
        let local_x = (water_index % CHUNK_EDGE) as u8;
        let local_z = (water_index / CHUNK_EDGE) as u8;
        let local_y = u8::try_from(
            column.water_top_y.expect("surface plan should carry water")
                - chunk_data.coord().1 * CHUNK_EDGE_I32,
        )
        .expect("water top should lie inside the chunk");
        let local =
            LocalBlockCoord::new(local_x, local_y, local_z).expect("water coord should be valid");
        let block = chunk_data
            .get_block(local)
            .expect("water block should be addressable");
        let block_key = registry.block_or_missing(block).key.as_str();

        assert!(matches!(block_key, "water" | "ice"));
    }
}
