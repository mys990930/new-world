use super::{MAX_HYDROLOGY_Y, MIN_HYDROLOGY_Y, lerp_f32};

#[derive(Debug, Clone, Copy)]
pub(super) struct WaterProfileInput {
    pub(super) smoothed_height: f32,
    pub(super) remaining_relief_budget: f32,
    pub(super) start_anchor: f32,
    pub(super) end_anchor: f32,
    pub(super) along_t: f32,
    pub(super) incision_bias: f32,
    pub(super) local_channel_depth: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WaterProfileAnchor {
    pub(super) water_surface: f32,
    pub(super) core_floor_target: f32,
    pub(super) core_total_cut: f32,
}

pub(super) fn resolve_water_profile(input: WaterProfileInput) -> WaterProfileAnchor {
    let start_anchor = input.start_anchor.clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
    let end_anchor = input
        .end_anchor
        .min(start_anchor)
        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
    let water_surface =
        lerp_f32(start_anchor, end_anchor, input.along_t).clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);

    let max_cut_depth = (2.8 + input.remaining_relief_budget * (0.80 + input.incision_bias * 0.12))
        .clamp(2.8, 18.0);
    let core_floor_target = (water_surface - input.local_channel_depth)
        .max(input.smoothed_height - max_cut_depth)
        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
    let core_total_cut = (input.smoothed_height - core_floor_target).clamp(0.0, max_cut_depth);

    WaterProfileAnchor {
        water_surface,
        core_floor_target,
        core_total_cut,
    }
}
