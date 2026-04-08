use std::collections::HashMap;

use super::chunk::{BlockId, ChunkData, ChunkSnapshot};
use super::coord::{ChunkCoord, WorldBlockCoord, world_to_chunk_local};
use super::edit::{EditError, EditResult, WorldEdit, remesh_targets_for_block};
use super::meta::WorldMeta;
use super::query::NeighborChunks;

pub struct WorldCore {
    meta: WorldMeta,
    loaded_chunks: HashMap<ChunkCoord, ChunkData>,
}

impl WorldCore {
    pub fn new(meta: WorldMeta) -> Self {
        Self {
            meta,
            loaded_chunks: HashMap::new(),
        }
    }

    pub fn meta(&self) -> &WorldMeta {
        &self.meta
    }

    pub fn has_chunk(&self, coord: ChunkCoord) -> bool {
        self.loaded_chunks.contains_key(&coord)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{LocalBlockCoord, generate_chunk};

    #[test]
    fn world_core_reads_generated_plane_block() {
        let meta = WorldMeta::new(42);
        let coord = ChunkCoord(0, 0, 0);
        let chunk = generate_chunk(coord, &meta);
        let mut world = WorldCore::new(meta);
        world.insert_chunk(coord, chunk);

        assert_eq!(world.get_block(WorldBlockCoord(2, 0, 3)), Some(BlockId::Grass));
        assert_eq!(world.get_block(WorldBlockCoord(2, 1, 3)), Some(BlockId::Air));
    }

    #[test]
    fn boundary_edit_marks_neighbor_chunk_for_remesh() {
        let mut world = WorldCore::new(WorldMeta::default());
        let coord = ChunkCoord(0, 0, 0);
        world.insert_chunk(coord, ChunkData::new_empty(coord));

        let result = world.apply_edit(WorldEdit::SetBlock {
            pos: WorldBlockCoord(15, 0, 0),
            block: BlockId::Stone,
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
                .and_then(|chunk| chunk.get_block(LocalBlockCoord::new(15, 0, 0).unwrap())),
            Some(BlockId::Stone)
        );
    }

    #[test]
    fn query_neighbors_returns_loaded_neighbor_snapshots() {
        let mut world = WorldCore::new(WorldMeta::default());
        world.insert_chunk(ChunkCoord(0, 0, 0), ChunkData::new_empty(ChunkCoord(0, 0, 0)));
        let mut east = ChunkData::new_empty(ChunkCoord(1, 0, 0));
        east.set_block(LocalBlockCoord::new(0, 0, 0).unwrap(), BlockId::Grass)
            .unwrap();
        world.insert_chunk(ChunkCoord(1, 0, 0), east);

        let neighbors = world.query_neighbors(ChunkCoord(0, 0, 0));

        assert_eq!(
            neighbors
                .pos_x
                .as_ref()
                .and_then(|chunk| chunk.get_block(LocalBlockCoord::new(0, 0, 0).unwrap())),
            Some(BlockId::Grass)
        );
        assert!(neighbors.neg_x.is_none());
    }
}
