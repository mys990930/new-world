use std::sync::Arc;

use super::coord::{CHUNK_EDGE, CHUNK_VOLUME, ChunkCoord, LocalBlockCoord, is_local_in_bounds};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct BlockId(u16);

impl BlockId {
    pub const AIR: Self = Self(0);
    pub const GRASS: Self = Self(1);
    pub const DIRT: Self = Self(2);
    pub const STONE: Self = Self(3);

    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u16 {
        self.0
    }

    pub const fn to_raw(self) -> u16 {
        self.raw()
    }

    pub const fn is_air(self) -> bool {
        self.raw() == Self::AIR.raw()
    }

    pub const fn is_solid(self) -> bool {
        !self.is_air()
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
            blocks: vec![BlockId::AIR; CHUNK_VOLUME],
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

        chunk.set_block(local, BlockId::GRASS).unwrap();
        let snapshot = chunk.snapshot();
        chunk.set_block(local, BlockId::STONE).unwrap();

        assert_eq!(snapshot.coord(), coord);
        assert_eq!(snapshot.get_block(local), Some(BlockId::GRASS));
        assert_eq!(chunk.get_block(local), Some(BlockId::STONE));
    }
}
