use crate::world::coord::ChunkCoord;

#[derive(Debug, Clone, PartialEq)]
pub struct SmoothedPrototype {
    pub chunk: ChunkCoord,
    pub preserved_corridors: usize,
}

pub fn empty_smoothed_prototype(chunk: ChunkCoord) -> SmoothedPrototype {
    SmoothedPrototype {
        chunk,
        preserved_corridors: 0,
    }
}
