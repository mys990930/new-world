use super::chunk::BlockId;
use super::coord::{ChunkCoord, LocalBlockCoord, WorldBlockCoord, world_to_chunk_local};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldEdit {
    SetBlock { pos: WorldBlockCoord, block: BlockId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    MissingChunk { coord: ChunkCoord },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditResult {
    pub applied: bool,
    pub previous_block: Option<BlockId>,
    pub changed_chunks: Vec<ChunkCoord>,
    pub remesh_chunks: Vec<ChunkCoord>,
    pub error: Option<EditError>,
}

impl EditResult {
    pub fn success(
        previous_block: Option<BlockId>,
        changed_chunks: Vec<ChunkCoord>,
        remesh_chunks: Vec<ChunkCoord>,
    ) -> Self {
        Self {
            applied: true,
            previous_block,
            changed_chunks,
            remesh_chunks,
            error: None,
        }
    }

    pub fn failure(error: EditError) -> Self {
        Self {
            applied: false,
            previous_block: None,
            changed_chunks: Vec::new(),
            remesh_chunks: Vec::new(),
            error: Some(error),
        }
    }
}

pub(crate) fn remesh_targets_for_block(pos: WorldBlockCoord) -> Vec<ChunkCoord> {
    let (chunk, local) = world_to_chunk_local(pos);
    let mut coords = vec![chunk];

    push_neighbor_if_boundary(&mut coords, chunk, local, 0);
    push_neighbor_if_boundary(&mut coords, chunk, local, 1);
    push_neighbor_if_boundary(&mut coords, chunk, local, 2);

    coords
}

fn push_neighbor_if_boundary(
    coords: &mut Vec<ChunkCoord>,
    chunk: ChunkCoord,
    local: LocalBlockCoord,
    axis: u8,
) {
    let max = super::coord::CHUNK_EDGE as u8 - 1;
    match axis {
        0 => {
            if local.x == 0 {
                push_unique(coords, chunk.offset(-1, 0, 0));
            }
            if local.x == max {
                push_unique(coords, chunk.offset(1, 0, 0));
            }
        }
        1 => {
            if local.y == 0 {
                push_unique(coords, chunk.offset(0, -1, 0));
            }
            if local.y == max {
                push_unique(coords, chunk.offset(0, 1, 0));
            }
        }
        2 => {
            if local.z == 0 {
                push_unique(coords, chunk.offset(0, 0, -1));
            }
            if local.z == max {
                push_unique(coords, chunk.offset(0, 0, 1));
            }
        }
        _ => {}
    }
}

pub(crate) fn push_unique(coords: &mut Vec<ChunkCoord>, coord: ChunkCoord) {
    if !coords.contains(&coord) {
        coords.push(coord);
    }
}
