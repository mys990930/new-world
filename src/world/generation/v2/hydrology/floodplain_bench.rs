use super::{band_influence, lerp_f32};

#[derive(Debug, Clone, Copy)]
pub(super) struct FloodplainBenchInput {
    pub(super) water_radius: f32,
    pub(super) bank_radius: f32,
    pub(super) floodplain_radius: f32,
    pub(super) outer_radius: f32,
    pub(super) distance_blocks: f32,
    pub(super) lower_reach_signal: f32,
    pub(super) transition_softness: f32,
    pub(super) outer_spread_bias: f32,
    pub(super) core_influence: f32,
    pub(super) bank_influence: f32,
    pub(super) floodplain_influence: f32,
    pub(super) outer_influence: f32,
    pub(super) signed_distance_blocks: f32,
    pub(super) curvature_signal: f32,
    pub(super) bank_shelf_noise: f32,
    pub(super) gravel_bar_noise: f32,
    pub(super) outer_delta: f32,
    pub(super) flood_delta: f32,
    pub(super) bank_delta: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FloodplainBenchResult {
    pub(super) bank_shelf_lift: f32,
    pub(super) flood_bench_lift: f32,
    pub(super) inside_bend_alignment: f32,
}

pub(super) fn resolve_floodplain_bench(input: FloodplainBenchInput) -> FloodplainBenchResult {
    let bank_shelf_center = input.water_radius
        + (input.bank_radius - input.water_radius).max(0.8)
            * lerp_f32(0.34, 0.56, input.lower_reach_signal);
    let bank_shelf_width = ((input.bank_radius - input.water_radius).max(1.0)
        * (0.34 + input.transition_softness * 0.16 + input.lower_reach_signal * 0.08))
        .clamp(1.0, input.bank_radius.max(1.0));
    let bank_shelf_shell = band_influence(
        input.distance_blocks,
        bank_shelf_center,
        bank_shelf_width,
    ) * input
        .bank_influence
        .max(input.floodplain_influence * 0.72)
        * (1.0 - input.core_influence * 0.88);
    let inside_bend_alignment =
        (input.signed_distance_blocks.signum() * input.curvature_signal.signum()).max(0.0);
    let bank_shelf_bias = (0.24
        + inside_bend_alignment * 0.36
        + input.bank_shelf_noise.max(0.0) * 0.22
        + input.transition_softness * 0.12
        + input.lower_reach_signal * 0.08)
        .clamp(0.0, 1.0);

    let flood_bench_center = input.bank_radius
        + (input.floodplain_radius - input.bank_radius).max(1.2)
            * lerp_f32(0.26, 0.58, input.lower_reach_signal);
    let flood_bench_width = ((input.outer_radius - input.bank_radius).max(1.2)
        * (0.24 + input.outer_spread_bias * 0.18 + input.lower_reach_signal * 0.08))
        .clamp(1.0, input.outer_radius.max(1.0));
    let flood_bench_shell = band_influence(
        input.distance_blocks,
        flood_bench_center,
        flood_bench_width,
    ) * input
        .floodplain_influence
        .max(input.outer_influence * 0.70)
        * (1.0 - input.bank_influence * 0.78);

    FloodplainBenchResult {
        bank_shelf_lift: bank_shelf_shell
            * bank_shelf_bias
            * (input.bank_delta * 0.48 + input.flood_delta * 0.18),
        flood_bench_lift: flood_bench_shell
            * (0.10
                + input.gravel_bar_noise.abs() * 0.10
                + input.transition_softness * 0.12
                + input.outer_spread_bias * 0.08)
            * (input.flood_delta * 0.42 + input.outer_delta * 0.34),
        inside_bend_alignment,
    }
}
