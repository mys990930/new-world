use std::collections::{HashMap, HashSet};

use super::GameApp;
use crate::jobs::JobRequest;
use crate::world::{
    CHUNK_EDGE_I32, ChunkCoord, ChunkSnapshot, TopdownChunkColumnCoord, TopdownChunkColumnPatch,
    TopdownColumnScan, TopdownSurfaceRange, WorldBlockCoord, WorldCore,
    sample_single_topdown_column, topdown_surface_range,
};

pub const MINIMAP_BLOCK_SPAN: u32 = CHUNK_EDGE_I32 as u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMinimapViewport {
    pub min_world_x: i32,
    pub min_world_z: i32,
    pub columns: Vec<TopdownColumnScan>,
    pub surface_range: Option<TopdownSurfaceRange>,
}

#[derive(Debug, Default)]
pub struct AppMinimapCache {
    columns: HashMap<TopdownChunkColumnCoord, TopdownChunkColumnPatch>,
    pending_columns: HashSet<TopdownChunkColumnCoord>,
    dirty_columns: HashSet<TopdownChunkColumnCoord>,
}

impl AppMinimapCache {
    pub fn note_column_changed(&mut self, coord: TopdownChunkColumnCoord) -> bool {
        if self.pending_columns.contains(&coord) {
            self.dirty_columns.insert(coord);
            false
        } else {
            self.pending_columns.insert(coord);
            true
        }
    }

    pub fn cancel_pending_column(&mut self, coord: TopdownChunkColumnCoord) {
        self.pending_columns.remove(&coord);
        self.dirty_columns.remove(&coord);
    }

    pub fn remove_column(&mut self, coord: TopdownChunkColumnCoord) {
        self.columns.remove(&coord);
        self.cancel_pending_column(coord);
    }

    pub fn apply_built_patch(
        &mut self,
        coord: TopdownChunkColumnCoord,
        patch: TopdownChunkColumnPatch,
    ) -> bool {
        self.pending_columns.remove(&coord);
        self.columns.insert(coord, patch);
        self.dirty_columns.remove(&coord)
    }

    pub fn compose_viewport(
        &self,
        player_world_x: f32,
        player_world_z: f32,
    ) -> AppMinimapViewport {
        let player_block_x = player_world_x.floor() as i32;
        let player_block_z = player_world_z.floor() as i32;
        let half_span = MINIMAP_BLOCK_SPAN as i32 / 2;
        let min_world_x = player_block_x - half_span;
        let min_world_z = player_block_z - half_span;
        let mut columns =
            Vec::with_capacity(MINIMAP_BLOCK_SPAN as usize * MINIMAP_BLOCK_SPAN as usize);

        for z_offset in 0..MINIMAP_BLOCK_SPAN {
            let world_z = min_world_z + z_offset as i32;
            for x_offset in 0..MINIMAP_BLOCK_SPAN {
                let world_x = min_world_x + x_offset as i32;
                columns.push(self.lookup_world_column(world_x, world_z));
            }
        }

        let surface_range = topdown_surface_range(&columns);
        AppMinimapViewport {
            min_world_x,
            min_world_z,
            columns,
            surface_range,
        }
    }

    pub fn patch_world_column_from_live_world(
        &mut self,
        world: &WorldCore,
        block_pos: WorldBlockCoord,
    ) -> bool {
        let Some((min_chunk, max_chunk)) = world.loaded_chunk_bounds() else {
            return false;
        };
        let column_coord = world_block_to_minimap_chunk_column(block_pos.0, block_pos.2);
        let Some(patch) = self.columns.get_mut(&column_coord) else {
            return false;
        };

        let local_x = block_pos.0.rem_euclid(CHUNK_EDGE_I32) as u32;
        let local_z = block_pos.2.rem_euclid(CHUNK_EDGE_I32) as u32;
        let min_world_y = min_chunk.1 * CHUNK_EDGE_I32;
        let max_world_y = (max_chunk.1 + 1) * CHUNK_EDGE_I32 - 1;
        let scan = sample_single_topdown_column(
            world,
            world.block_registry(),
            block_pos.0,
            block_pos.2,
            min_world_y,
            max_world_y,
        );
        patch.set(local_x, local_z, scan)
    }

    fn lookup_world_column(&self, world_x: i32, world_z: i32) -> TopdownColumnScan {
        let chunk_coord = world_block_to_minimap_chunk_column(world_x, world_z);
        let local_x = world_x.rem_euclid(CHUNK_EDGE_I32) as u32;
        let local_z = world_z.rem_euclid(CHUNK_EDGE_I32) as u32;
        self.columns
            .get(&chunk_coord)
            .and_then(|patch| patch.get(local_x, local_z))
            .unwrap_or(TopdownColumnScan::AIR)
    }
}

impl GameApp {
    pub(crate) fn queue_loaded_world_minimap_rebuilds(&mut self) {
        self.queue_minimap_chunk_column_rebuilds(loaded_world_minimap_columns(&self.world));
    }

    pub(crate) fn queue_minimap_chunk_column_rebuilds(
        &mut self,
        coords: impl IntoIterator<Item = TopdownChunkColumnCoord>,
    ) {
        for coord in coords {
            self.queue_minimap_chunk_column_rebuild(coord);
        }
    }

    pub(crate) fn queue_minimap_chunk_column_rebuild(&mut self, coord: TopdownChunkColumnCoord) {
        if !self.minimap.note_column_changed(coord) {
            return;
        }

        let Some(request) = self.build_minimap_chunk_column_request(coord) else {
            self.minimap.remove_column(coord);
            return;
        };

        if let Err(error) = self.jobs.submit(request) {
            self.minimap.cancel_pending_column(coord);
            eprintln!(
                "[app] failed to queue minimap rebuild for chunk column ({}, {}): {:?}",
                coord.chunk_x, coord.chunk_z, error
            );
        }
    }

    pub(crate) fn handle_minimap_chunk_column_built(
        &mut self,
        coord: TopdownChunkColumnCoord,
        patch: TopdownChunkColumnPatch,
    ) {
        let needs_resubmit = self.minimap.apply_built_patch(coord, patch);
        if needs_resubmit {
            self.queue_minimap_chunk_column_rebuild(coord);
        }
    }

    fn build_minimap_chunk_column_request(
        &self,
        coord: TopdownChunkColumnCoord,
    ) -> Option<JobRequest> {
        let chunks = loaded_world_column_snapshots(&self.world, coord);
        if chunks.is_empty() {
            return None;
        }

        Some(JobRequest::BuildMinimapChunkColumn {
            coord,
            chunks,
            registry: self.world.block_registry_handle(),
        })
    }
}

pub fn world_block_to_minimap_chunk_column(world_x: i32, world_z: i32) -> TopdownChunkColumnCoord {
    TopdownChunkColumnCoord {
        chunk_x: world_x.div_euclid(CHUNK_EDGE_I32),
        chunk_z: world_z.div_euclid(CHUNK_EDGE_I32),
    }
}

fn loaded_world_minimap_columns(world: &WorldCore) -> Vec<TopdownChunkColumnCoord> {
    let Some((min, max)) = world.loaded_chunk_bounds() else {
        return Vec::new();
    };

    let mut coords = Vec::new();
    for z in min.2..=max.2 {
        for x in min.0..=max.0 {
            let has_loaded_column = (min.1..=max.1).any(|y| world.has_chunk(ChunkCoord(x, y, z)));
            if has_loaded_column {
                coords.push(TopdownChunkColumnCoord {
                    chunk_x: x,
                    chunk_z: z,
                });
            }
        }
    }
    coords
}

fn loaded_world_column_snapshots(
    world: &WorldCore,
    coord: TopdownChunkColumnCoord,
) -> Vec<ChunkSnapshot> {
    let Some((min, max)) = world.loaded_chunk_bounds() else {
        return Vec::new();
    };

    let mut snapshots = Vec::new();
    for y in min.1..=max.1 {
        let chunk_coord = ChunkCoord(coord.chunk_x, y, coord.chunk_z);
        if let Some(snapshot) = world.snapshot_chunk(chunk_coord) {
            snapshots.push(snapshot);
        }
    }
    snapshots
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockId, TopdownCell, CHUNK_EDGE};

    #[test]
    fn cache_marks_second_change_as_dirty_while_pending() {
        let coord = TopdownChunkColumnCoord {
            chunk_x: 3,
            chunk_z: -2,
        };
        let mut cache = AppMinimapCache::default();

        assert!(cache.note_column_changed(coord));
        assert!(!cache.note_column_changed(coord));

        let patch = TopdownChunkColumnPatch::new(
            coord,
            vec![TopdownColumnScan::AIR; CHUNK_EDGE * CHUNK_EDGE],
        );
        assert!(cache.apply_built_patch(coord, patch));
    }

    #[test]
    fn viewport_reads_cached_chunk_column_cells() {
        let coord = TopdownChunkColumnCoord {
            chunk_x: 0,
            chunk_z: 0,
        };
        let mut patch =
            TopdownChunkColumnPatch::new(coord, vec![TopdownColumnScan::AIR; CHUNK_EDGE * CHUNK_EDGE]);
        patch.set(
            0,
            0,
            TopdownColumnScan {
                visible: TopdownCell {
                    top_y: Some(4),
                    block: BlockId::STONE,
                },
                ..TopdownColumnScan::AIR
            },
        );

        let mut cache = AppMinimapCache::default();
        cache.apply_built_patch(coord, patch);

        let viewport = cache.compose_viewport(16.0, 16.0);
        assert_eq!(viewport.columns.len(), CHUNK_EDGE * CHUNK_EDGE);
        assert!(viewport.columns.iter().any(|scan| scan.visible.block == BlockId::STONE));
    }
}
