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
    use crate::world::registry::BlockRegistry;

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
}
