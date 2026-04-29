use super::HydrologyMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct BranchKey {
    pub(super) river_id: u32,
    pub(super) kind_rank: u8,
    pub(super) order: u8,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CorridorHydrologyResponse {
    pub(super) branch_key: BranchKey,
    pub(super) terrain_height: f32,
    pub(super) water_surface_height: Option<f32>,
    pub(super) channel_floor_height: Option<f32>,
    pub(super) saturation: f32,
    pub(super) gravel_bar_strength: f32,
    pub(super) mode: HydrologyMode,
    pub(super) influence: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RegionHydrologySignals {
    pub(super) floodplain_bias: f32,
    pub(super) lake_bias: f32,
    pub(super) wetland_bias: f32,
    pub(super) dryland_bias: f32,
    pub(super) outlet_bias: f32,
    pub(super) incision_bias: f32,
    pub(super) transition_softness: f32,
    pub(super) lateral_variability: f32,
    pub(super) width_variability: f32,
    pub(super) depth_variability: f32,
    pub(super) meander_bias: f32,
    pub(super) outer_spread_bias: f32,
    pub(super) confinement_bias: f32,
}
