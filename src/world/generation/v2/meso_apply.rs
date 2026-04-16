use crate::world::coord::ChunkCoord;

#[derive(Debug, Clone, PartialEq)]
pub struct MesoAppliedPrototype {
    pub chunk: ChunkCoord,
    pub applied_feature_keys: Vec<&'static str>,
}

pub fn empty_meso_applied_prototype(chunk: ChunkCoord) -> MesoAppliedPrototype {
    MesoAppliedPrototype {
        chunk,
        applied_feature_keys: Vec::new(),
    }
}
