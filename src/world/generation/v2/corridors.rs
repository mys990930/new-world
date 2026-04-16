use crate::world::coord::ChunkCoord;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverCorridorConstraint {
    pub center_x: f32,
    pub center_z: f32,
    pub half_width_blocks: f32,
    pub downstream_grade_per_block: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkCorridorWindow {
    pub chunk: ChunkCoord,
    pub corridors: Vec<RiverCorridorConstraint>,
}

pub fn empty_chunk_corridor_window(chunk: ChunkCoord) -> ChunkCorridorWindow {
    ChunkCorridorWindow {
        chunk,
        corridors: Vec::new(),
    }
}
