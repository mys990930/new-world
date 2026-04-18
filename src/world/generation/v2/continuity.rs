use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

pub(super) const PROTOTYPE_CONTINUITY_TILE_EDGE_CHUNKS: i32 = 4;
pub(super) const PROTOTYPE_CONTINUITY_HALO_CHUNKS: i32 = 0;
pub(super) const PROTOTYPE_BORDER_ANCHOR_BAND_BLOCKS: i32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct GenerationTileCoord {
    pub x: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GenerationTileBounds {
    pub coord: GenerationTileCoord,
    pub core_min_chunk_x: i32,
    pub core_min_chunk_z: i32,
    pub core_max_chunk_x_exclusive: i32,
    pub core_max_chunk_z_exclusive: i32,
    pub solve_min_chunk_x: i32,
    pub solve_min_chunk_z: i32,
    pub solve_max_chunk_x_exclusive: i32,
    pub solve_max_chunk_z_exclusive: i32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct BorderAnchorPoint {
    pub sample_world_x: f32,
    pub sample_world_z: f32,
    pub strength: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ColumnBorderAnchors {
    pub slots: [Option<BorderAnchorPoint>; 4],
}

impl GenerationTileBounds {
    pub(super) fn for_chunk(chunk: ChunkCoord) -> Self {
        let coord = GenerationTileCoord {
            x: chunk.0.div_euclid(PROTOTYPE_CONTINUITY_TILE_EDGE_CHUNKS),
            z: chunk.2.div_euclid(PROTOTYPE_CONTINUITY_TILE_EDGE_CHUNKS),
        };
        let core_min_chunk_x = coord.x * PROTOTYPE_CONTINUITY_TILE_EDGE_CHUNKS;
        let core_min_chunk_z = coord.z * PROTOTYPE_CONTINUITY_TILE_EDGE_CHUNKS;
        let core_max_chunk_x_exclusive = core_min_chunk_x + PROTOTYPE_CONTINUITY_TILE_EDGE_CHUNKS;
        let core_max_chunk_z_exclusive = core_min_chunk_z + PROTOTYPE_CONTINUITY_TILE_EDGE_CHUNKS;

        Self {
            coord,
            core_min_chunk_x,
            core_min_chunk_z,
            core_max_chunk_x_exclusive,
            core_max_chunk_z_exclusive,
            solve_min_chunk_x: core_min_chunk_x - PROTOTYPE_CONTINUITY_HALO_CHUNKS,
            solve_min_chunk_z: core_min_chunk_z - PROTOTYPE_CONTINUITY_HALO_CHUNKS,
            solve_max_chunk_x_exclusive: core_max_chunk_x_exclusive + PROTOTYPE_CONTINUITY_HALO_CHUNKS,
            solve_max_chunk_z_exclusive: core_max_chunk_z_exclusive + PROTOTYPE_CONTINUITY_HALO_CHUNKS,
        }
    }

    pub(super) fn core_min_world_x(self) -> i32 {
        self.core_min_chunk_x * CHUNK_EDGE_I32
    }

    pub(super) fn core_min_world_z(self) -> i32 {
        self.core_min_chunk_z * CHUNK_EDGE_I32
    }

    pub(super) fn core_max_world_x_exclusive(self) -> i32 {
        self.core_max_chunk_x_exclusive * CHUNK_EDGE_I32
    }

    pub(super) fn core_max_world_z_exclusive(self) -> i32 {
        self.core_max_chunk_z_exclusive * CHUNK_EDGE_I32
    }

    pub(super) fn solve_chunks(self) -> impl Iterator<Item = ChunkCoord> {
        (self.solve_min_chunk_z..self.solve_max_chunk_z_exclusive).flat_map(move |chunk_z| {
            (self.solve_min_chunk_x..self.solve_max_chunk_x_exclusive)
                .map(move |chunk_x| ChunkCoord(chunk_x, 0, chunk_z))
        })
    }

    pub(super) fn anchors_for_world_column(self, world_x: i32, world_z: i32) -> ColumnBorderAnchors {
        let mut anchors = ColumnBorderAnchors::default();
        let band = PROTOTYPE_BORDER_ANCHOR_BAND_BLOCKS;
        let west_distance = world_x - self.core_min_world_x();
        let east_distance = self.core_max_world_x_exclusive() - 1 - world_x;
        let north_distance = world_z - self.core_min_world_z();
        let south_distance = self.core_max_world_z_exclusive() - 1 - world_z;

        if let Some(strength) = border_strength(west_distance, band) {
            anchors.push(BorderAnchorPoint {
                sample_world_x: self.core_min_world_x() as f32,
                sample_world_z: world_z as f32 + 0.5,
                strength,
            });
        }
        if let Some(strength) = border_strength(east_distance, band) {
            anchors.push(BorderAnchorPoint {
                sample_world_x: self.core_max_world_x_exclusive() as f32,
                sample_world_z: world_z as f32 + 0.5,
                strength,
            });
        }
        if let Some(strength) = border_strength(north_distance, band) {
            anchors.push(BorderAnchorPoint {
                sample_world_x: world_x as f32 + 0.5,
                sample_world_z: self.core_min_world_z() as f32,
                strength,
            });
        }
        if let Some(strength) = border_strength(south_distance, band) {
            anchors.push(BorderAnchorPoint {
                sample_world_x: world_x as f32 + 0.5,
                sample_world_z: self.core_max_world_z_exclusive() as f32,
                strength,
            });
        }

        anchors
    }
}

impl ColumnBorderAnchors {
    fn push(&mut self, anchor: BorderAnchorPoint) {
        for slot in &mut self.slots {
            if slot.is_none() {
                *slot = Some(anchor);
                return;
            }
        }

        if let Some((index, _)) = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| slot.map(|existing| (index, existing)))
            .min_by(|(_, left), (_, right)| left.strength.total_cmp(&right.strength))
        {
            if anchor.strength > self.slots[index].map(|existing| existing.strength).unwrap_or(0.0) {
                self.slots[index] = Some(anchor);
            }
        }
    }

    pub(super) fn iter(self) -> impl Iterator<Item = BorderAnchorPoint> {
        self.slots.into_iter().flatten()
    }
}

impl Default for ColumnBorderAnchors {
    fn default() -> Self {
        Self {
            slots: [None, None, None, None],
        }
    }
}

fn border_strength(distance_from_border_blocks: i32, band_blocks: i32) -> Option<f32> {
    if distance_from_border_blocks < 0 || distance_from_border_blocks >= band_blocks {
        return None;
    }

    let normalized = 1.0 - distance_from_border_blocks as f32 / band_blocks as f32;
    let t = normalized.clamp(0.0, 1.0);
    Some(t * t * (3.0 - 2.0 * t))
}
