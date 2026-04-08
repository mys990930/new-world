use std::collections::BTreeSet;

use bevy_ecs::prelude::Resource;

use crate::world::ChunkCoord;

#[derive(Resource, Debug, Clone, Default)]
pub struct ChunkStates {
    pub interest: BTreeSet<ChunkCoord>,
    pub loaded: BTreeSet<ChunkCoord>,
    pub generation_requested: BTreeSet<ChunkCoord>,
    pub mesh_requested: BTreeSet<ChunkCoord>,
    pub render_ready: BTreeSet<ChunkCoord>,
}

impl ChunkStates {
    pub fn focus_single(&mut self, coord: ChunkCoord) {
        self.interest.clear();
        self.interest.insert(coord);
    }

    pub fn visible_chunks(&self) -> Vec<ChunkCoord> {
        self.render_ready.iter().copied().collect()
    }
}
