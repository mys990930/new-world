use crate::world::coord::ChunkCoord;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrototypeColumn {
    pub base_height: f32,
    pub relief_budget: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BaseHeightfieldPrototype {
    pub chunk: ChunkCoord,
    pub columns: Vec<PrototypeColumn>,
}

pub fn empty_base_heightfield_prototype(chunk: ChunkCoord) -> BaseHeightfieldPrototype {
    BaseHeightfieldPrototype {
        chunk,
        columns: Vec::new(),
    }
}
