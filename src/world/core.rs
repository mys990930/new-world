use std::collections::HashMap;
use std::sync::Arc;

use super::chunk::{BlockId, ChunkData, ChunkSnapshot};
use super::coord::{ChunkCoord, WorldBlockCoord, world_to_chunk_local};
use super::edit::{EditError, EditResult, WorldEdit, remesh_targets_for_block};
use super::meta::WorldMeta;
use super::query::{NeighborChunks, Ray3, RaycastHit};
use super::registry::BlockRegistry;

pub struct WorldCore {
    meta: WorldMeta,
    block_registry: Arc<BlockRegistry>,
    loaded_chunks: HashMap<ChunkCoord, ChunkData>,
}

impl WorldCore {
    pub fn new(meta: WorldMeta, block_registry: Arc<BlockRegistry>) -> Self {
        Self {
            meta,
            block_registry,
            loaded_chunks: HashMap::new(),
        }
    }

    pub fn meta(&self) -> &WorldMeta {
        &self.meta
    }

    pub fn has_chunk(&self, coord: ChunkCoord) -> bool {
        self.loaded_chunks.contains_key(&coord)
    }

    pub fn block_registry(&self) -> &BlockRegistry {
        self.block_registry.as_ref()
    }

    pub fn block_registry_handle(&self) -> Arc<BlockRegistry> {
        Arc::clone(&self.block_registry)
    }

    pub fn insert_chunk(&mut self, coord: ChunkCoord, mut chunk: ChunkData) {
        chunk.set_coord(coord);
        self.loaded_chunks.insert(coord, chunk);
    }

    pub fn remove_chunk(&mut self, coord: ChunkCoord) -> Option<ChunkData> {
        self.loaded_chunks.remove(&coord)
    }

    pub fn get_block(&self, pos: WorldBlockCoord) -> Option<BlockId> {
        let (chunk_coord, local) = world_to_chunk_local(pos);
        self.loaded_chunks.get(&chunk_coord)?.get_block(local)
    }

    pub fn apply_edit(&mut self, edit: WorldEdit) -> EditResult {
        match edit {
            WorldEdit::SetBlock { pos, block } => {
                let (chunk_coord, local) = world_to_chunk_local(pos);
                let Some(chunk) = self.loaded_chunks.get_mut(&chunk_coord) else {
                    return EditResult::failure(EditError::MissingChunk { coord: chunk_coord });
                };

                let previous_block = chunk
                    .set_block(local, block)
                    .expect("world_to_chunk_local must produce in-bounds local coords");
                EditResult::success(
                    Some(previous_block),
                    vec![chunk_coord],
                    remesh_targets_for_block(pos),
                )
            }
        }
    }

    pub fn get_chunk(&self, coord: ChunkCoord) -> Option<&ChunkData> {
        self.loaded_chunks.get(&coord)
    }

    pub fn get_chunk_mut(&mut self, coord: ChunkCoord) -> Option<&mut ChunkData> {
        self.loaded_chunks.get_mut(&coord)
    }

    pub fn snapshot_chunk(&self, coord: ChunkCoord) -> Option<ChunkSnapshot> {
        self.loaded_chunks.get(&coord).map(ChunkData::snapshot)
    }

    pub fn snapshot_region(&self, min: ChunkCoord, max: ChunkCoord) -> Vec<ChunkSnapshot> {
        let mut snapshots = Vec::new();
        for y in min.1..=max.1 {
            for z in min.2..=max.2 {
                for x in min.0..=max.0 {
                    let coord = ChunkCoord(x, y, z);
                    if let Some(snapshot) = self.snapshot_chunk(coord) {
                        snapshots.push(snapshot);
                    }
                }
            }
        }
        snapshots
    }

    pub fn query_neighbors(&self, coord: ChunkCoord) -> NeighborChunks {
        NeighborChunks {
            neg_x: self.snapshot_chunk(coord.offset(-1, 0, 0)),
            pos_x: self.snapshot_chunk(coord.offset(1, 0, 0)),
            neg_y: self.snapshot_chunk(coord.offset(0, -1, 0)),
            pos_y: self.snapshot_chunk(coord.offset(0, 1, 0)),
            neg_z: self.snapshot_chunk(coord.offset(0, 0, -1)),
            pos_z: self.snapshot_chunk(coord.offset(0, 0, 1)),
        }
    }

    pub fn query_block_state(&self, pos: WorldBlockCoord) -> Option<BlockId> {
        self.get_block(pos)
    }

    pub fn loaded_chunk_bounds(&self) -> Option<(ChunkCoord, ChunkCoord)> {
        let mut coords = self.loaded_chunks.keys().copied();
        let first = coords.next()?;
        let mut min = first;
        let mut max = first;

        for coord in coords {
            min.0 = min.0.min(coord.0);
            min.1 = min.1.min(coord.1);
            min.2 = min.2.min(coord.2);
            max.0 = max.0.max(coord.0);
            max.1 = max.1.max(coord.1);
            max.2 = max.2.max(coord.2);
        }

        Some((min, max))
    }

    pub fn raycast_blocks(&self, ray: Ray3, max_distance: f32) -> Option<RaycastHit> {
        if max_distance <= 0.0 {
            return None;
        }

        let direction = normalize3(ray.direction)?;
        let mut block = WorldBlockCoord(
            ray.origin[0].floor() as i32,
            ray.origin[1].floor() as i32,
            ray.origin[2].floor() as i32,
        );
        let mut traveled = 0.0_f32;
        let mut entry_face = None;

        let step_x = step_sign(direction[0]);
        let step_y = step_sign(direction[1]);
        let step_z = step_sign(direction[2]);

        let mut t_max_x = first_boundary_distance(ray.origin[0], direction[0], block.0, step_x);
        let mut t_max_y = first_boundary_distance(ray.origin[1], direction[1], block.1, step_y);
        let mut t_max_z = first_boundary_distance(ray.origin[2], direction[2], block.2, step_z);

        let t_delta_x = axis_delta(direction[0]);
        let t_delta_y = axis_delta(direction[1]);
        let t_delta_z = axis_delta(direction[2]);

        let max_steps = (max_distance.ceil() as usize).saturating_mul(6).max(1);

        for _ in 0..max_steps {
            if self.get_block(block).is_some_and(|block_id| self.block_registry.is_solid(block_id)) {
                let hit_face = entry_face.unwrap_or_else(|| opposite_face_for_direction(direction));
                let point = add_scaled3(ray.origin, direction, traveled);
                return Some(RaycastHit {
                    block,
                    face: hit_face,
                    point,
                    distance: traveled,
                });
            }

            if t_max_x <= t_max_y && t_max_x <= t_max_z {
                traveled = t_max_x;
                if traveled > max_distance {
                    return None;
                }
                block.0 += step_x;
                t_max_x += t_delta_x;
                entry_face = Some(if step_x >= 0 {
                    super::chunk::BlockFace::NegX
                } else {
                    super::chunk::BlockFace::PosX
                });
            } else if t_max_y <= t_max_z {
                traveled = t_max_y;
                if traveled > max_distance {
                    return None;
                }
                block.1 += step_y;
                t_max_y += t_delta_y;
                entry_face = Some(if step_y >= 0 {
                    super::chunk::BlockFace::NegY
                } else {
                    super::chunk::BlockFace::PosY
                });
            } else {
                traveled = t_max_z;
                if traveled > max_distance {
                    return None;
                }
                block.2 += step_z;
                t_max_z += t_delta_z;
                entry_face = Some(if step_z >= 0 {
                    super::chunk::BlockFace::NegZ
                } else {
                    super::chunk::BlockFace::PosZ
                });
            }
        }

        None
    }
}

fn normalize3(vector: [f32; 3]) -> Option<[f32; 3]> {
    let length_sq = vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2];
    if length_sq <= f32::EPSILON {
        None
    } else {
        let inv_length = length_sq.sqrt().recip();
        Some([
            vector[0] * inv_length,
            vector[1] * inv_length,
            vector[2] * inv_length,
        ])
    }
}

fn step_sign(axis: f32) -> i32 {
    if axis > 0.0 {
        1
    } else if axis < 0.0 {
        -1
    } else {
        0
    }
}

fn axis_delta(axis: f32) -> f32 {
    if axis.abs() <= f32::EPSILON {
        f32::INFINITY
    } else {
        axis.abs().recip()
    }
}

fn first_boundary_distance(origin: f32, direction: f32, block: i32, step: i32) -> f32 {
    if step == 0 || direction.abs() <= f32::EPSILON {
        return f32::INFINITY;
    }

    let boundary = if step > 0 {
        block as f32 + 1.0
    } else {
        block as f32
    };
    ((boundary - origin) / direction).max(0.0)
}

fn add_scaled3(origin: [f32; 3], direction: [f32; 3], distance: f32) -> [f32; 3] {
    [
        origin[0] + direction[0] * distance,
        origin[1] + direction[1] * distance,
        origin[2] + direction[2] * distance,
    ]
}

fn opposite_face_for_direction(direction: [f32; 3]) -> super::chunk::BlockFace {
    let abs_x = direction[0].abs();
    let abs_y = direction[1].abs();
    let abs_z = direction[2].abs();

    if abs_x >= abs_y && abs_x >= abs_z {
        if direction[0] >= 0.0 {
            super::chunk::BlockFace::NegX
        } else {
            super::chunk::BlockFace::PosX
        }
    } else if abs_y >= abs_z {
        if direction[1] >= 0.0 {
            super::chunk::BlockFace::NegY
        } else {
            super::chunk::BlockFace::PosY
        }
    } else if direction[2] >= 0.0 {
        super::chunk::BlockFace::NegZ
    } else {
        super::chunk::BlockFace::PosZ
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::world::{BlockFace, CHUNK_EDGE, LocalBlockCoord, generate_chunk};

    fn test_registry() -> Arc<BlockRegistry> {
        Arc::new(BlockRegistry::load_default().expect("default registry should load"))
    }

    #[test]
    fn world_core_reads_generated_floor_block() {
        let meta = WorldMeta::new(42);
        let coord = ChunkCoord(0, -8, 0);
        let registry = test_registry();
        let terrain = registry
            .block_id("terrain_debug")
            .expect("terrain_debug block should exist");
        let chunk = generate_chunk(coord, &meta, registry.as_ref());
        let mut world = WorldCore::new(meta, registry);
        world.insert_chunk(coord, chunk);

        assert_eq!(world.get_block(WorldBlockCoord(2, -256, 3)), Some(terrain));
        assert_eq!(world.get_block(WorldBlockCoord(2, -225, 3)), Some(terrain));
    }

    #[test]
    fn boundary_edit_marks_neighbor_chunk_for_remesh() {
        let mut world = WorldCore::new(WorldMeta::default(), test_registry());
        let coord = ChunkCoord(0, 0, 0);
        world.insert_chunk(coord, ChunkData::new_empty(coord));
        let edge = CHUNK_EDGE as i32 - 1;
        let local_edge = LocalBlockCoord::new(CHUNK_EDGE as u8 - 1, 0, 0).unwrap();

        let result = world.apply_edit(WorldEdit::SetBlock {
            pos: WorldBlockCoord(edge, 0, 0),
            block: BlockId::STONE,
        });

        assert!(result.applied);
        assert_eq!(result.changed_chunks, vec![coord]);
        assert!(result.remesh_chunks.contains(&coord));
        assert!(result.remesh_chunks.contains(&ChunkCoord(1, 0, 0)));
        assert!(result.remesh_chunks.contains(&ChunkCoord(0, -1, 0)));
        assert!(result.remesh_chunks.contains(&ChunkCoord(0, 0, -1)));
        assert_eq!(
            world
                .get_chunk(coord)
                .and_then(|chunk| chunk.get_block(local_edge)),
            Some(BlockId::STONE)
        );
    }

    #[test]
    fn query_neighbors_returns_loaded_neighbor_snapshots() {
        let mut world = WorldCore::new(WorldMeta::default(), test_registry());
        world.insert_chunk(ChunkCoord(0, 0, 0), ChunkData::new_empty(ChunkCoord(0, 0, 0)));
        let mut east = ChunkData::new_empty(ChunkCoord(1, 0, 0));
        east.set_block(LocalBlockCoord::new(0, 0, 0).unwrap(), BlockId::GRASS)
            .unwrap();
        world.insert_chunk(ChunkCoord(1, 0, 0), east);

        let neighbors = world.query_neighbors(ChunkCoord(0, 0, 0));

        assert_eq!(
            neighbors
                .pos_x
                .as_ref()
                .and_then(|chunk| chunk.get_block(LocalBlockCoord::new(0, 0, 0).unwrap())),
            Some(BlockId::GRASS)
        );
        assert!(neighbors.neg_x.is_none());
    }

    #[test]
    fn raycast_hits_top_face_of_plane_block() {
        let mut world = WorldCore::new(WorldMeta::default(), test_registry());
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        chunk.set_block(LocalBlockCoord::new(3, 0, 3).unwrap(), BlockId::GRASS)
            .unwrap();
        world.insert_chunk(ChunkCoord(0, 0, 0), chunk);

        let hit = world
            .raycast_blocks(
                Ray3 {
                    origin: [3.5, 4.0, 3.5],
                    direction: [0.0, -1.0, 0.0],
                },
                16.0,
            )
            .expect("ray should hit the plane block");

        assert_eq!(hit.block, WorldBlockCoord(3, 0, 3));
        assert_eq!(hit.face, BlockFace::PosY);
        assert!((hit.point[0] - 3.5).abs() < 1e-5);
        assert!((hit.point[1] - 1.0).abs() < 1e-5);
        assert!((hit.point[2] - 3.5).abs() < 1e-5);
    }
}
