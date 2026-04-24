use super::MIN_VISIBLE_WATER_DEPTH;

#[derive(Debug, Clone, Copy)]
pub(super) struct VisibleWaterInput {
    pub(super) water_presence: f32,
    pub(super) channel_has_visible_water: bool,
    pub(super) lake_bias: f32,
    pub(super) water_radius: f32,
    pub(super) region_lake_bias: f32,
    pub(super) transition_softness: f32,
    pub(super) local_channel_depth: f32,
    pub(super) river_flow_potential: f32,
    pub(super) downstream_grade_per_block: f32,
    pub(super) depth_noise: f32,
    pub(super) water_sheet_influence: f32,
    pub(super) anchored_water_surface: f32,
    pub(super) terrain_height: f32,
    pub(super) channel_floor_y: f32,
}

pub(super) fn resolve_visible_water_surface(input: VisibleWaterInput) -> Option<f32> {
    if input.water_presence < 0.18 || (!input.channel_has_visible_water && input.lake_bias < 0.52) {
        return None;
    }

    let max_water_depth = (0.85
        + input.water_radius.sqrt() * 0.44
        + input.region_lake_bias * 1.10
        + input.transition_softness * 0.24)
        .clamp(0.85, 5.0);
    let desired_water_depth = desired_water_depth(
        input.local_channel_depth,
        input.river_flow_potential,
        input.downstream_grade_per_block,
        input.depth_noise,
        input.water_sheet_influence,
    );
    let max_supported_surface =
        input.anchored_water_surface.min(input.terrain_height + max_water_depth);
    let available_depth = max_supported_surface - input.channel_floor_y;
    let visible_headroom = max_supported_surface - input.terrain_height;

    if available_depth >= MIN_VISIBLE_WATER_DEPTH && visible_headroom >= MIN_VISIBLE_WATER_DEPTH {
        Some(
            (input.channel_floor_y + desired_water_depth.min(available_depth))
                .min(max_supported_surface)
                .max(input.terrain_height + MIN_VISIBLE_WATER_DEPTH),
        )
    } else {
        None
    }
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
