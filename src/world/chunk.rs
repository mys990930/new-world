use std::sync::Arc;

use super::coord::{CHUNK_EDGE, CHUNK_VOLUME, ChunkCoord, LocalBlockCoord, is_local_in_bounds};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u16)]
pub enum BlockId {
    #[default]
    Air = 0,
    Grass = 1,
    Dirt = 2,
    Stone = 3,
}

impl BlockId {
    pub const fn from_raw(raw: u16) -> Option<Self> {
        match raw {
            0 => Some(Self::Air),
            1 => Some(Self::Grass),
            2 => Some(Self::Dirt),
            3 => Some(Self::Stone),
            _ => None,
        }
    }

    pub const fn to_raw(self) -> u16 {
        self as u16
    }

    pub const fn is_air(self) -> bool {
        matches!(self, Self::Air)
    }

    pub const fn is_solid(self) -> bool {
        !self.is_air()
    }

    pub const fn face_color(self, face: BlockFace) -> [f32; 4] {
        match self {
            Self::Air => [0.0, 0.0, 0.0, 0.0],
            Self::Grass => match face {
                BlockFace::PosY => [0.34, 0.72, 0.30, 1.0],
                BlockFace::NegY => [0.24, 0.18, 0.10, 1.0],
                BlockFace::NegX | BlockFace::PosX | BlockFace::NegZ | BlockFace::PosZ => {
                    [0.46, 0.33, 0.18, 1.0]
                }
            },
            Self::Dirt => [0.44, 0.30, 0.17, 1.0],
            Self::Stone => [0.52, 0.54, 0.58, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockFace {
    NegX,
    PosX,
    NegY,
    PosY,
    NegZ,
    PosZ,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkData {
    coord: ChunkCoord,
    blocks: Vec<BlockId>,
}

impl ChunkData {
    pub fn new_empty(coord: ChunkCoord) -> Self {
        Self {
            coord,
            blocks: vec![BlockId::Air; CHUNK_VOLUME],
        }
    }

    pub(crate) fn from_blocks(coord: ChunkCoord, blocks: Vec<BlockId>) -> Self {
        debug_assert_eq!(blocks.len(), CHUNK_VOLUME);
        Self { coord, blocks }
    }

    pub fn coord(&self) -> ChunkCoord {
        self.coord
    }

    pub(crate) fn set_coord(&mut self, coord: ChunkCoord) {
        self.coord = coord;
    }

    pub fn get_block(&self, local: LocalBlockCoord) -> Option<BlockId> {
        Some(self.blocks[linear_index(local)?])
    }

    pub fn set_block(
        &mut self,
        local: LocalBlockCoord,
        block: BlockId,
    ) -> Result<BlockId, ChunkWriteError> {
        let index = linear_index(local).ok_or(ChunkWriteError::OutOfBounds(local))?;
        let previous = std::mem::replace(&mut self.blocks[index], block);
        Ok(previous)
    }

    pub fn snapshot(&self) -> ChunkSnapshot {
        ChunkSnapshot {
            coord: self.coord,
            blocks: Arc::from(self.blocks.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSnapshot {
    coord: ChunkCoord,
    blocks: Arc<[BlockId]>,
}

impl ChunkSnapshot {
    pub fn coord(&self) -> ChunkCoord {
        self.coord
    }

    pub fn blocks(&self) -> &[BlockId] {
        &self.blocks
    }

    pub fn get_block(&self, local: LocalBlockCoord) -> Option<BlockId> {
        Some(self.blocks[linear_index(local)?])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkWriteError {
    OutOfBounds(LocalBlockCoord),
}

fn linear_index(local: LocalBlockCoord) -> Option<usize> {
    if !is_local_in_bounds(local) {
        return None;
    }

    let x = usize::from(local.x);
    let y = usize::from(local.y);
    let z = usize::from(local.z);
    Some(x + z * CHUNK_EDGE + y * CHUNK_EDGE * CHUNK_EDGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_snapshot_freezes_chunk_state() {
        let coord = ChunkCoord(1, 2, 3);
        let mut chunk = ChunkData::new_empty(coord);
        let local = LocalBlockCoord::new(0, 0, 0).unwrap();

        chunk.set_block(local, BlockId::Grass).unwrap();
        let snapshot = chunk.snapshot();
        chunk.set_block(local, BlockId::Stone).unwrap();

        assert_eq!(snapshot.coord(), coord);
        assert_eq!(snapshot.get_block(local), Some(BlockId::Grass));
        assert_eq!(chunk.get_block(local), Some(BlockId::Stone));
    }
}
