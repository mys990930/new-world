use super::chunk::{BlockFace, BlockId, ChunkSnapshot};
use super::coord::{CHUNK_EDGE as CHUNK_EDGE_USIZE, LocalBlockCoord};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NeighborChunks {
    pub neg_x: Option<ChunkSnapshot>,
    pub pos_x: Option<ChunkSnapshot>,
    pub neg_y: Option<ChunkSnapshot>,
    pub pos_y: Option<ChunkSnapshot>,
    pub neg_z: Option<ChunkSnapshot>,
    pub pos_z: Option<ChunkSnapshot>,
}

impl NeighborChunks {
    pub fn block_across_face(&self, face: BlockFace, local: LocalBlockCoord) -> Option<BlockId> {
        let max = CHUNK_EDGE_USIZE as u8 - 1;
        let neighbor_local = match face {
            BlockFace::NegX => LocalBlockCoord::new(max, local.y, local.z),
            BlockFace::PosX => LocalBlockCoord::new(0, local.y, local.z),
            BlockFace::NegY => LocalBlockCoord::new(local.x, max, local.z),
            BlockFace::PosY => LocalBlockCoord::new(local.x, 0, local.z),
            BlockFace::NegZ => LocalBlockCoord::new(local.x, local.y, max),
            BlockFace::PosZ => LocalBlockCoord::new(local.x, local.y, 0),
        }?;

        match face {
            BlockFace::NegX => self.neg_x.as_ref(),
            BlockFace::PosX => self.pos_x.as_ref(),
            BlockFace::NegY => self.neg_y.as_ref(),
            BlockFace::PosY => self.pos_y.as_ref(),
            BlockFace::NegZ => self.neg_z.as_ref(),
            BlockFace::PosZ => self.pos_z.as_ref(),
        }
        .and_then(|snapshot| snapshot.get_block(neighbor_local))
    }
}
