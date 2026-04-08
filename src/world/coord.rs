pub const CHUNK_EDGE: usize = 16;
pub const CHUNK_EDGE_I32: i32 = CHUNK_EDGE as i32;
pub const CHUNK_VOLUME: usize = CHUNK_EDGE * CHUNK_EDGE * CHUNK_EDGE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChunkCoord(pub i32, pub i32, pub i32);

impl ChunkCoord {
    pub const fn offset(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self(self.0 + dx, self.1 + dy, self.2 + dz)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WorldBlockCoord(pub i32, pub i32, pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LocalBlockCoord {
    pub x: u8,
    pub y: u8,
    pub z: u8,
}

impl LocalBlockCoord {
    pub fn new(x: u8, y: u8, z: u8) -> Option<Self> {
        let local = Self { x, y, z };
        is_local_in_bounds(local).then_some(local)
    }

    pub(crate) const fn new_unchecked(x: u8, y: u8, z: u8) -> Self {
        Self { x, y, z }
    }
}

pub fn world_to_chunk_local(pos: WorldBlockCoord) -> (ChunkCoord, LocalBlockCoord) {
    let chunk = ChunkCoord(
        pos.0.div_euclid(CHUNK_EDGE_I32),
        pos.1.div_euclid(CHUNK_EDGE_I32),
        pos.2.div_euclid(CHUNK_EDGE_I32),
    );
    let local = LocalBlockCoord::new_unchecked(
        pos.0.rem_euclid(CHUNK_EDGE_I32) as u8,
        pos.1.rem_euclid(CHUNK_EDGE_I32) as u8,
        pos.2.rem_euclid(CHUNK_EDGE_I32) as u8,
    );
    (chunk, local)
}

pub fn chunk_local_to_world(chunk: ChunkCoord, local: LocalBlockCoord) -> WorldBlockCoord {
    WorldBlockCoord(
        chunk.0 * CHUNK_EDGE_I32 + i32::from(local.x),
        chunk.1 * CHUNK_EDGE_I32 + i32::from(local.y),
        chunk.2 * CHUNK_EDGE_I32 + i32::from(local.z),
    )
}

pub fn is_local_in_bounds(local: LocalBlockCoord) -> bool {
    usize::from(local.x) < CHUNK_EDGE
        && usize::from(local.y) < CHUNK_EDGE
        && usize::from(local.z) < CHUNK_EDGE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_to_chunk_local_round_trips_negative_coordinates() {
        let samples = [
            WorldBlockCoord(-17, -1, -16),
            WorldBlockCoord(-16, 0, 15),
            WorldBlockCoord(-1, 31, 32),
            WorldBlockCoord(0, 0, 0),
            WorldBlockCoord(15, 15, 15),
            WorldBlockCoord(16, 16, 16),
        ];

        for sample in samples {
            let (chunk, local) = world_to_chunk_local(sample);
            assert_eq!(chunk_local_to_world(chunk, local), sample);
            assert!(is_local_in_bounds(local));
        }
    }
}
