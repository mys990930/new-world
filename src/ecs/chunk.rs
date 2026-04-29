use std::collections::BTreeSet;

use bevy_ecs::prelude::Resource;

use crate::jobs::JobRequest;
use crate::world::ChunkCoord;

pub const HORIZONTAL_INTEREST_CHUNK_RADIUS: i32 = 3;
pub const HORIZONTAL_RETAIN_CHUNK_RADIUS: i32 = 4;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChunkLifecyclePlan {
    pub interest: BTreeSet<ChunkCoord>,
    pub retain: BTreeSet<ChunkCoord>,
    pub job_requests: Vec<JobRequest>,
    pub unload_coords: Vec<ChunkCoord>,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct ChunkStates {
    pub interest: BTreeSet<ChunkCoord>,
    pub retain: BTreeSet<ChunkCoord>,
    pub loaded: BTreeSet<ChunkCoord>,
    pub load_requested: BTreeSet<ChunkCoord>,
    pub generation_requested: BTreeSet<ChunkCoord>,
    pub mesh_requested: BTreeSet<ChunkCoord>,
    pub remesh_needed: BTreeSet<ChunkCoord>,
    pub render_ready: BTreeSet<ChunkCoord>,
}

impl ChunkStates {
    pub fn set_interest(&mut self, coords: impl IntoIterator<Item = ChunkCoord>) {
        self.interest = coords.into_iter().collect();
    }

    pub fn set_retain(&mut self, coords: impl IntoIterator<Item = ChunkCoord>) {
        self.retain = coords.into_iter().collect();
    }

    pub fn visible_chunks(&self) -> Vec<ChunkCoord> {
        self.render_ready.iter().copied().collect()
    }
}
