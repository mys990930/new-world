use super::{MAX_HYDROLOGY_Y, MIN_HYDROLOGY_Y, MIN_VISIBLE_WATER_DEPTH};

#[derive(Debug, Clone, Copy)]
pub(super) struct VisibleWaterInput {
    pub(super) water_presence: f32,
    pub(super) channel_has_visible_water: bool,
    pub(super) lake_bias: f32,
    pub(super) anchored_water_surface: f32,
    pub(super) terrain_height: f32,
    pub(super) channel_floor_y: f32,
}

pub(super) fn resolve_visible_water_surface(input: VisibleWaterInput) -> Option<f32> {
    if input.water_presence < 0.18 || (!input.channel_has_visible_water && input.lake_bias < 0.52) {
        return None;
    }

    let water_surface = input.anchored_water_surface;
    if !water_surface.is_finite() {
        return None;
    }

    let available_depth = water_surface - input.channel_floor_y;
    let visible_headroom = water_surface - input.terrain_height;

    if available_depth >= MIN_VISIBLE_WATER_DEPTH && visible_headroom >= MIN_VISIBLE_WATER_DEPTH {
        Some(water_surface)
    } else {
        None
    }
}

pub(super) fn carve_supported_water_surface(
    profile_water_surface: f32,
    core_floor_target: f32,
    desired_water_depth: f32,
) -> f32 {
    if !profile_water_surface.is_finite()
        || !core_floor_target.is_finite()
        || !desired_water_depth.is_finite()
    {
        return profile_water_surface;
    }

    let depth_limited_surface =
        core_floor_target + desired_water_depth.max(MIN_VISIBLE_WATER_DEPTH);
    profile_water_surface
        .min(depth_limited_surface)
        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y)
}

pub(super) fn desired_water_depth(
    local_channel_depth: f32,
    river_flow_potential: f32,
    downstream_grade_per_block: f32,
    depth_noise: f32,
    water_sheet_influence: f32,
) -> f32 {
    (MIN_VISIBLE_WATER_DEPTH
        + local_channel_depth * (0.22 + river_flow_potential * 0.04)
        + downstream_grade_per_block * 560.0
        + river_flow_potential * 0.55
        + depth_noise.max(0.0) * 0.20
        + water_sheet_influence * 0.18)
        .clamp(MIN_VISIBLE_WATER_DEPTH, 4.2)
}
