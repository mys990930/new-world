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
    blocks: ChunkBlockStorage,
}

impl ChunkData {
    pub fn new_empty(coord: ChunkCoord) -> Self {
        Self::new_filled(coord, BlockId::AIR)
    }

    pub fn new_filled(coord: ChunkCoord, block: BlockId) -> Self {
        Self {
            coord,
            blocks: ChunkBlockStorage::Uniform(block),
        }
    }

    pub(crate) fn from_blocks(coord: ChunkCoord, blocks: Vec<BlockId>) -> Self {
        debug_assert_eq!(blocks.len(), CHUNK_VOLUME);
        Self {
            coord,
            blocks: ChunkBlockStorage::from_dense(blocks),
        }
    }

    pub fn coord(&self) -> ChunkCoord {
        self.coord
    }

    pub(crate) fn set_coord(&mut self, coord: ChunkCoord) {
        self.coord = coord;
    }

    pub fn get_block(&self, local: LocalBlockCoord) -> Option<BlockId> {
        Some(self.blocks.get(linear_index(local)?))
    }

    pub fn set_block(
        &mut self,
        local: LocalBlockCoord,
        block: BlockId,
    ) -> Result<BlockId, ChunkWriteError> {
        let index = linear_index(local).ok_or(ChunkWriteError::OutOfBounds(local))?;
        let previous = self.blocks.set(index, block);
        Ok(previous)
    }

    pub fn snapshot(&self) -> ChunkSnapshot {
        ChunkSnapshot {
            coord: self.coord,
            blocks: self.blocks.snapshot(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkSnapshot {
    coord: ChunkCoord,
    blocks: ChunkSnapshotStorage,
}

impl ChunkSnapshot {
    pub fn coord(&self) -> ChunkCoord {
        self.coord
    }

    pub fn block_count(&self) -> usize {
        CHUNK_VOLUME
    }

    pub fn get_block(&self, local: LocalBlockCoord) -> Option<BlockId> {
        Some(self.blocks.get(linear_index(local)?))
    }

    pub(crate) fn storage_encoding(&self) -> ChunkStorageEncoding {
        self.blocks.encoding()
    }

    pub(crate) fn uniform_block(&self) -> Option<BlockId> {
        self.blocks.uniform_block()
    }

    pub(crate) fn iter_blocks(&self) -> ChunkBlockIter<'_> {
        self.blocks.iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkWriteError {
    OutOfBounds(LocalBlockCoord),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChunkStorageEncoding {
    Uniform = 0,
    Dense = 1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChunkBlockStorage {
    Uniform(BlockId),
    Dense(Vec<BlockId>),
}

impl ChunkBlockStorage {
    fn from_dense(blocks: Vec<BlockId>) -> Self {
        uniform_block_in_slice(&blocks)
            .map(Self::Uniform)
            .unwrap_or(Self::Dense(blocks))
    }

    fn get(&self, index: usize) -> BlockId {
        match self {
            Self::Uniform(block) => *block,
            Self::Dense(blocks) => blocks[index],
        }
    }

    fn set(&mut self, index: usize, block: BlockId) -> BlockId {
        match self {
            Self::Uniform(current) => {
                let previous = *current;
                if previous == block {
                    return previous;
                }

                let mut blocks = vec![previous; CHUNK_VOLUME];
                blocks[index] = block;
                *self = Self::Dense(blocks);
                previous
            }
            Self::Dense(blocks) => std::mem::replace(&mut blocks[index], block),
        }
    }

    fn snapshot(&self) -> ChunkSnapshotStorage {
        match self {
            Self::Uniform(block) => ChunkSnapshotStorage::Uniform(*block),
            Self::Dense(blocks) => ChunkSnapshotStorage::from_dense(blocks.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChunkSnapshotStorage {
    Uniform(BlockId),
    Dense(Arc<[BlockId]>),
}

impl ChunkSnapshotStorage {
    fn from_dense(blocks: Vec<BlockId>) -> Self {
        uniform_block_in_slice(&blocks)
            .map(Self::Uniform)
            .unwrap_or_else(|| Self::Dense(Arc::from(blocks)))
    }

    fn get(&self, index: usize) -> BlockId {
        match self {
            Self::Uniform(block) => *block,
            Self::Dense(blocks) => blocks[index],
        }
    }

    fn encoding(&self) -> ChunkStorageEncoding {
        match self {
            Self::Uniform(_) => ChunkStorageEncoding::Uniform,
            Self::Dense(_) => ChunkStorageEncoding::Dense,
        }
    }

    fn uniform_block(&self) -> Option<BlockId> {
        match self {
            Self::Uniform(block) => Some(*block),
            Self::Dense(_) => None,
        }
    }

    fn iter(&self) -> ChunkBlockIter<'_> {
        match self {
            Self::Uniform(block) => ChunkBlockIter::Uniform {
                block: *block,
                remaining: CHUNK_VOLUME,
            },
            Self::Dense(blocks) => ChunkBlockIter::Dense(blocks.iter()),
        }
    }
}

pub(crate) enum ChunkBlockIter<'a> {
    Uniform { block: BlockId, remaining: usize },
    Dense(std::slice::Iter<'a, BlockId>),
}

impl Iterator for ChunkBlockIter<'_> {
    type Item = BlockId;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Uniform { block, remaining } => {
                if *remaining == 0 {
                    None
                } else {
                    *remaining -= 1;
                    Some(*block)
                }
            }
            Self::Dense(iter) => iter.next().copied(),
        }
    }
}

fn uniform_block_in_slice(blocks: &[BlockId]) -> Option<BlockId> {
    let first = blocks.first().copied()?;
    blocks.iter().all(|block| *block == first).then_some(first)
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

    #[test]
    fn empty_chunk_stays_uniform_until_first_non_air_write() {
        let coord = ChunkCoord(0, 0, 0);
        let local = LocalBlockCoord::new(1, 2, 3).unwrap();
        let mut chunk = ChunkData::new_empty(coord);

        assert_eq!(
            chunk.snapshot().storage_encoding(),
            ChunkStorageEncoding::Uniform
        );

        chunk.set_block(local, BlockId::GRASS).unwrap();

        assert_eq!(
            chunk.snapshot().storage_encoding(),
            ChunkStorageEncoding::Dense
        );
        assert_eq!(chunk.get_block(local), Some(BlockId::GRASS));
        assert_eq!(
            chunk.get_block(LocalBlockCoord::new(0, 0, 0).unwrap()),
            Some(BlockId::AIR)
        );
    }
}
