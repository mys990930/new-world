use crate::world::coord::ChunkCoord;

#[derive(Debug, Clone, PartialEq)]
pub struct HydrologySolve {
    pub chunk: ChunkCoord,
    pub connected_waterlines: usize,
}

pub fn empty_hydrology_solve(chunk: ChunkCoord) -> HydrologySolve {
    HydrologySolve {
        chunk,
        connected_waterlines: 0,
    }
}
