pub mod corridors;
pub mod hydrology;
pub mod inputs;
pub mod meso_apply;
pub mod prototype;
pub mod smoothing;
pub mod voxelize;

pub const GENERATOR_LABEL: &str = "v2_scaffold";

pub use corridors::{
    ChunkCorridorWindow, RiverCorridorConstraint, build_chunk_corridor_window,
    empty_chunk_corridor_window,
};
pub use hydrology::{HydrologySolve, empty_hydrology_solve};
pub use inputs::{
    ChunkGenerationV2Inputs, ChunkGenerationV2Scaffold, V2ScaffoldStage, build_chunk_v2_scaffold,
    prepare_chunk_v2_inputs,
};
pub use meso_apply::{MesoAppliedPrototype, empty_meso_applied_prototype};
pub use prototype::{BaseHeightfieldPrototype, PrototypeColumn, empty_base_heightfield_prototype};
pub use smoothing::{SmoothedPrototype, empty_smoothed_prototype};
pub use voxelize::{VoxelizationPlan, default_voxelization_plan};
