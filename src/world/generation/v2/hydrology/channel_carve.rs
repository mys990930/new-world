use super::{MAX_HYDROLOGY_Y, MIN_HYDROLOGY_Y};

#[derive(Debug, Clone, Copy)]
pub(super) struct ChannelCarveInput {
    pub(super) core_total_cut: f32,
    pub(super) floodplain_lowering_base: f32,
    pub(super) transition_softness: f32,
    pub(super) floodplain_bias: f32,
    pub(super) outer_spread_bias: f32,
    pub(super) lower_reach_signal: f32,
    pub(super) edge_noise: f32,
    pub(super) carve_breakup_noise: f32,
    pub(super) depth_variability: f32,
    pub(super) bank_shelf_noise: f32,
    pub(super) lateral_variability: f32,
    pub(super) curvature_signal: f32,
    pub(super) outer_noise: f32,
    pub(super) gravel_bar_noise: f32,
    pub(super) transition_noise: f32,
    pub(super) depth_noise: f32,
    pub(super) confinement: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ChannelCarveLayers {
    pub(super) outer_delta: f32,
    pub(super) flood_delta: f32,
    pub(super) bank_delta: f32,
    pub(super) core_delta: f32,
}

pub(super) fn resolve_channel_carve_layers(input: ChannelCarveInput) -> ChannelCarveLayers {
    let outer_total_cut = (input.floodplain_lowering_base
        * (0.32 + input.transition_softness * 0.26 + input.outer_spread_bias * 0.20)
        + input.core_total_cut * (0.04 + input.lower_reach_signal * 0.10)
        + input.edge_noise.max(0.0) * 0.18)
        .clamp(0.0, input.core_total_cut * 0.46);
    let flood_total_cut_min = outer_total_cut + 0.10;
    let flood_total_cut_max = (input.core_total_cut * 0.64).max(flood_total_cut_min);
    let flood_total_cut = (input.floodplain_lowering_base
        * (0.84 + input.transition_softness * 0.24 + input.outer_spread_bias * 0.10)
        + input.core_total_cut
            * (0.06 + input.floodplain_bias * 0.10 + input.lower_reach_signal * 0.08))
        .clamp(flood_total_cut_min, flood_total_cut_max);
    let bank_total_cut_min = flood_total_cut + 0.14;
    let bank_total_cut_max = (input.core_total_cut * 0.92).max(bank_total_cut_min);
    let bank_total_cut = (input.core_total_cut
        * (0.54 + input.confinement * 0.18 + input.depth_variability * 0.06
            - input.transition_softness * 0.05)
        + input.floodplain_lowering_base * 0.24
        + input.depth_noise.max(0.0) * 0.18)
        .clamp(bank_total_cut_min, bank_total_cut_max);
    let carve_breakup = (input.carve_breakup_noise * (0.20 + input.depth_variability * 0.16)
        + input.bank_shelf_noise * (0.12 + input.lateral_variability * 0.10)
        + input.curvature_signal.abs() * 0.12
        + input.lower_reach_signal * 0.05)
        .clamp(-0.42, 0.48);

    ChannelCarveLayers {
        outer_delta: (outer_total_cut
            * (1.0
                + carve_breakup * 0.16
                + input.outer_noise * 0.08
                + input.gravel_bar_noise.abs() * 0.04))
            .max(0.0),
        flood_delta: ((flood_total_cut - outer_total_cut).max(0.0)
            * (1.0
                + carve_breakup * 0.18
                + input.transition_noise * 0.10
                + input.outer_noise * 0.06))
            .max(0.0),
        bank_delta: ((bank_total_cut - flood_total_cut).max(0.0)
            * (1.0
                + carve_breakup * 0.22
                + input.bank_shelf_noise.abs() * 0.14
                + input.depth_noise * 0.08))
            .max(0.0),
        core_delta: ((input.core_total_cut - bank_total_cut).max(0.0)
            * (1.0
                + carve_breakup * 0.14
                + input.depth_noise * 0.10
                + input.curvature_signal.abs() * 0.08))
            .max(0.0),
    }
}

pub(super) fn apply_channel_carve_layers(
    smoothed_height: f32,
    layers: ChannelCarveLayers,
    outer_influence: f32,
    floodplain_influence: f32,
    bank_influence: f32,
    core_influence: f32,
    bank_shelf_lift: f32,
    flood_bench_lift: f32,
) -> f32 {
    let terrain_cut = (layers.outer_delta * outer_influence
        + layers.flood_delta * floodplain_influence
        + layers.bank_delta * bank_influence
        + layers.core_delta * core_influence
        - bank_shelf_lift
        - flood_bench_lift)
        .max(0.0);

    (smoothed_height - terrain_cut)
        .min(smoothed_height)
        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y)
}
