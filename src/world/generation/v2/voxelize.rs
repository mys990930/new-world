use crate::world::coord::ChunkCoord;

#[derive(Debug, Clone, PartialEq)]
pub struct VoxelizationPlan {
    pub chunk: ChunkCoord,
    pub material_policy_key: &'static str,
    pub seasonal_override_key: Option<&'static str>,
}

pub fn default_voxelization_plan(chunk: ChunkCoord) -> VoxelizationPlan {
    VoxelizationPlan {
        chunk,
        material_policy_key: "unassigned",
        seasonal_override_key: None,
    }
}
