use super::{
    MAX_HYDROLOGY_Y, MIN_HYDROLOGY_Y, MIN_VISIBLE_WATER_DEPTH, band_influence, lerp_f32,
    sanitize_unit_interval, smootherstep01,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct GravelBarInput {
    pub(super) terrain_height_base: f32,
    pub(super) smoothed_height: f32,
    pub(super) depositional_water_surface: f32,
    pub(super) core_floor_target: f32,
    pub(super) water_radius: f32,
    pub(super) bank_radius: f32,
    pub(super) base_channel_radius: f32,
    pub(super) channel_radius: f32,
    pub(super) distance_blocks: f32,
    pub(super) signed_distance_blocks: f32,
    pub(super) lower_reach_signal: f32,
    pub(super) transition_softness: f32,
    pub(super) water_sheet_influence: f32,
    pub(super) core_influence: f32,
    pub(super) bank_influence: f32,
    pub(super) floodplain_influence: f32,
    pub(super) inside_bend_alignment: f32,
    pub(super) curvature_signal: f32,
    pub(super) confluence_signal: f32,
    pub(super) slope_signal: f32,
    pub(super) downstream_grade_per_block: f32,
    pub(super) gravel_bar_noise: f32,
    pub(super) regime_bar_strength: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GravelBarResult {
    pub(super) terrain_height: f32,
    pub(super) strength: f32,
}

pub(super) fn apply_gravel_bar(input: GravelBarInput) -> GravelBarResult {
    let widening_signal = sanitize_unit_interval(
        (input.channel_radius / input.base_channel_radius.max(0.5) - 1.0) / 0.48,
        0.0,
    );
    let flattening_signal = sanitize_unit_interval(
        input.lower_reach_signal * 0.42
            + (1.0 - (input.downstream_grade_per_block / 0.0048).clamp(0.0, 1.0)) * 0.28
            + (1.0 - input.slope_signal) * 0.18
            + input.transition_softness * 0.10,
        0.0,
    );
    let water_edge_distance = input.distance_blocks - input.water_radius;
    let bank_span = (input.bank_radius - input.water_radius).max(1.0);
    let gravel_bar_freeboard = (0.14
        + input.gravel_bar_noise.abs() * 0.08
        + widening_signal * 0.06
        + input.confluence_signal * 0.06
        + input.lower_reach_signal * 0.06)
        .clamp(0.12, 0.54);
    let gravel_bar_target_height = (input.depositional_water_surface + gravel_bar_freeboard)
        .min(input.smoothed_height - 0.04)
        .max(input.core_floor_target + MIN_VISIBLE_WATER_DEPTH * 0.30)
        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
    let target_delta = input.terrain_height_base - gravel_bar_target_height;
    let same_height_band_signal = 1.0 - smootherstep01((target_delta.abs() - 0.55) / 2.15);
    let bank_rise_support = 1.0 - smootherstep01((target_delta.max(0.0) - 2.6) / 3.0);
    let reach_continuity_signal = (0.24
        + same_height_band_signal * 0.34
        + bank_rise_support * 0.12
        + flattening_signal * 0.18
        + input.lower_reach_signal * 0.12
        + input.transition_softness * 0.08)
        .clamp(0.22, 0.78);
    let bar_inner_ramp = (0.55 + input.water_radius * 0.06).clamp(0.55, 1.80);
    let bar_flat_width = (bank_span
        * (0.70
            + same_height_band_signal * 0.12
            + input.transition_softness * 0.18
            + input.lower_reach_signal * 0.24)
        + widening_signal * 4.4
        + input.confluence_signal * 5.2
        + flattening_signal * 3.2)
        .clamp(3.5, 18.0 + input.lower_reach_signal * 18.0);
    let bar_outer_fade = (2.4
        + input.transition_softness * 5.0
        + input.lower_reach_signal * 6.0
        + input.confluence_signal * 2.0)
        .clamp(2.0, 16.0);
    let outside_water_mask =
        smootherstep01((water_edge_distance + bar_inner_ramp * 0.32) / bar_inner_ramp);
    let platform_core_mask =
        1.0 - smootherstep01((water_edge_distance - bar_flat_width).max(0.0) / bar_outer_fade);
    let near_water_bank_mask = outside_water_mask
        * platform_core_mask
        * (1.0 - input.water_sheet_influence * 0.70)
        * (1.0 - input.core_influence * 0.96)
        * input.bank_influence.max(input.floodplain_influence * 0.74)
        * (0.58 + same_height_band_signal * 0.28 + bank_rise_support * 0.14);
    let column_side = if input.signed_distance_blocks >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let curvature_side = if input.curvature_signal >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let noise_side = if input.gravel_bar_noise >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let curvature_turn_signal = smootherstep01((input.curvature_signal.abs() - 0.012) / 0.055);
    let preferred_side = column_side * lerp_f32(noise_side, curvature_side, curvature_turn_signal);
    let side_gate = smootherstep01((preferred_side + 0.18) / 1.18);
    let both_bank_allowance =
        (input.confluence_signal * 0.38 + widening_signal * 0.28 + flattening_signal * 0.18)
            .clamp(0.0, 0.46);
    let continuity_allowance = reach_continuity_signal * (1.0 - curvature_turn_signal * 0.58);
    let lateral_select = side_gate.max(both_bank_allowance).max(continuity_allowance);
    let gravel_bar_shell = near_water_bank_mask
        * (0.82
            + band_influence(
                input.distance_blocks,
                input.water_radius + bar_flat_width * 0.52,
                bar_flat_width.max(1.0),
            ) * 0.18);
    let gravel_bar_lateral_bias = (0.10 + lateral_select * 0.90).clamp(0.10, 1.0);
    let gravel_bar_energy = (0.24
        + reach_continuity_signal * 0.30
        + input.inside_bend_alignment * (0.52 + input.curvature_signal.abs() * 0.24)
        + widening_signal * 0.40
        + input.confluence_signal * 0.56
        + flattening_signal * 0.34
        + input.gravel_bar_noise.abs() * 0.06
        + input.lower_reach_signal * 0.16)
        .clamp(0.0, 1.65);
    let strength = sanitize_unit_interval(
        gravel_bar_shell
            * gravel_bar_lateral_bias
            * gravel_bar_energy
            * 1.34
            * input.regime_bar_strength,
        0.0,
    );
    let bench_presence = gravel_bar_shell * gravel_bar_lateral_bias;
    let gravel_bar_surface_weight = if bench_presence >= 0.026 {
        ((0.18
            + strength * 1.04
            + reach_continuity_signal * 0.18
            + bench_presence * gravel_bar_energy * 0.26
            + flattening_signal * 0.08)
            * input.regime_bar_strength.clamp(0.12, 1.35))
        .clamp(0.0, 0.97)
    } else {
        0.0
    };
    let terrain_height = lerp_f32(
        input.terrain_height_base,
        gravel_bar_target_height,
        gravel_bar_surface_weight,
    )
    .min(input.smoothed_height)
    .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);

    GravelBarResult {
        terrain_height,
        strength,
    }
}
