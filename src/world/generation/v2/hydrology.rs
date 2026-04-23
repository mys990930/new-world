use std::collections::BTreeSet;
use std::f32::consts::{PI, TAU};

use crate::world::atlas::{
    BiomeFamily, CoastalContext, HydrologyContext, ReliefClass, RiverPathKind,
    TerrainFormFamily,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

use super::super::{SEA_LEVEL_Y, WORLD_FLOOR_Y};
use super::{
    ChunkCorridorWindow, ChunkGenerationV2Inputs, RegionSampleWeight, RiverCorridorConstraint,
    SmoothedPrototype, sample_atlas_fields_fractional, sample_region_weights,
};

const MIN_HYDROLOGY_Y: f32 = WORLD_FLOOR_Y as f32 + 4.0;
const MAX_HYDROLOGY_Y: f32 = SEA_LEVEL_Y as f32 + 180.0;
const MIN_VISIBLE_WATER_DEPTH: f32 = 0.45;
const HYDRO_HASH_K1: u64 = 0x9E37_79B9_7F4A_7C15;
const HYDRO_HASH_K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const HYDRO_HASH_K3: u64 = 0x1656_67B1_9E37_79F9;
const HYDRO_SALT_WARP_X: u64 = 0xA511_3001_0000_0001;
const HYDRO_SALT_WARP_Z: u64 = 0xA511_3001_0000_0002;
const HYDRO_SALT_CENTERLINE: u64 = 0xA511_3001_0000_0003;
const HYDRO_SALT_WIDTH: u64 = 0xA511_3001_0000_0004;
const HYDRO_SALT_DEPTH: u64 = 0xA511_3001_0000_0005;
const HYDRO_SALT_TRANSITION: u64 = 0xA511_3001_0000_0006;
const HYDRO_SALT_OUTER: u64 = 0xA511_3001_0000_0007;
const HYDRO_SALT_ASYMMETRY: u64 = 0xA511_3001_0000_0008;
const HYDRO_SALT_EDGE: u64 = 0xA511_3001_0000_0009;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HydrologyMode {
    Dry,
    Channel,
    Floodplain,
    Lake,
    Wetland,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HydrologyColumn {
    pub terrain_height: f32,
    pub water_surface_height: Option<f32>,
    pub channel_floor_height: Option<f32>,
    pub saturation: f32,
    pub mode: HydrologyMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HydrologySolve {
    pub chunk: ChunkCoord,
    pub columns: Vec<HydrologyColumn>,
    pub connected_waterlines: usize,
}

pub fn empty_hydrology_solve(chunk: ChunkCoord) -> HydrologySolve {
    HydrologySolve {
        chunk,
        columns: Vec::new(),
        connected_waterlines: 0,
    }
}

pub fn build_chunk_hydrology_solve(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    corridor_window: &ChunkCorridorWindow,
    smoothed: &SmoothedPrototype,
) -> HydrologySolve {
    debug_assert_eq!(inputs.chunk, chunk);
    debug_assert_eq!(corridor_window.chunk, chunk);
    debug_assert_eq!(smoothed.chunk, chunk);

    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let mut columns = Vec::with_capacity(smoothed.columns.len());
    let mut connected_waterlines = BTreeSet::new();

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let index = column_index(local_x, local_z);
            let smoothed_column = smoothed.columns[index];
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let sample_world_x = world_x as f32 + 0.5;
            let sample_world_z = world_z as f32 + 0.5;
            let field =
                sample_atlas_fields_fractional(&inputs.atlas_fields, sample_world_x, sample_world_z);
            let region_weights =
                sample_region_weights(&inputs.region_classes, sample_world_x, sample_world_z);
            let region_signals = blended_region_hydrology_signals(&region_weights);

            let best_corridor = corridor_window
                .corridors
                .iter()
                .filter_map(|corridor| {
                    corridor_hydrology_response(
                        chunk,
                        inputs,
                        field,
                        region_signals,
                        local_x as f32 + 0.5,
                        local_z as f32 + 0.5,
                        smoothed_column,
                        *corridor,
                    )
                })
                .max_by(|left, right| left.influence.total_cmp(&right.influence));

            let column = match best_corridor {
                Some(response) => {
                    let column = sanitize_hydrology_column(
                        HydrologyColumn {
                            terrain_height: response.terrain_height,
                            water_surface_height: response.water_surface_height,
                            channel_floor_height: response.channel_floor_height,
                            saturation: response.saturation,
                            mode: response.mode,
                        },
                        smoothed_column,
                    );
                    if column.water_surface_height.is_some() {
                        connected_waterlines.insert(response.branch_key);
                    }

                    column
                }
                None => sanitize_hydrology_column(
                    basin_fallback_column(field, region_signals, smoothed_column),
                    smoothed_column,
                ),
            };

            columns.push(column);
        }
    }

    HydrologySolve {
        chunk,
        columns,
        connected_waterlines: connected_waterlines.len(),
    }
}

fn sanitize_unit_interval(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback.clamp(0.0, 1.0)
    }
}

fn sanitize_optional_height(value: Option<f32>) -> Option<f32> {
    value.and_then(|height| {
        height
            .is_finite()
            .then_some(height.clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y))
    })
}

fn sanitize_hydrology_column(
    column: HydrologyColumn,
    smoothed: super::smoothing::SmoothedColumn,
) -> HydrologyColumn {
    let fallback_terrain = smoothed.height.clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
    let mut terrain_height = if column.terrain_height.is_finite() {
        column.terrain_height.clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y)
    } else {
        fallback_terrain
    };
    let saturation = sanitize_unit_interval(column.saturation, 0.0);
    let mut water_surface_height = sanitize_optional_height(column.water_surface_height);
    let mut channel_floor_height = sanitize_optional_height(column.channel_floor_height);
    let mut mode = column.mode;

    if let Some(water) = water_surface_height {
        let max_supported_terrain = water - MIN_VISIBLE_WATER_DEPTH;
        if !max_supported_terrain.is_finite() || max_supported_terrain <= MIN_HYDROLOGY_Y {
            water_surface_height = None;
            channel_floor_height = None;
            if matches!(mode, HydrologyMode::Channel | HydrologyMode::Lake) {
                mode = HydrologyMode::Floodplain;
            }
        } else {
            terrain_height = terrain_height
                .min(max_supported_terrain)
                .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
            let floor_cap = max_supported_terrain
                .min(terrain_height)
                .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
            let floor_fallback = terrain_height.min(floor_cap);
            channel_floor_height = Some(
                channel_floor_height
                    .unwrap_or(floor_fallback)
                    .min(terrain_height)
                    .min(floor_cap)
                    .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y),
            );
            if matches!(mode, HydrologyMode::Dry | HydrologyMode::Wetland) {
                mode = HydrologyMode::Floodplain;
            }
        }
    } else if let Some(floor) = channel_floor_height {
        channel_floor_height = Some(floor.min(terrain_height).clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y));
    }

    HydrologyColumn {
        terrain_height,
        water_surface_height,
        channel_floor_height,
        saturation,
        mode,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct BranchKey {
    river_id: u32,
    kind_rank: u8,
    order: u8,
}

#[derive(Debug, Clone, Copy)]
struct CorridorHydrologyResponse {
    branch_key: BranchKey,
    terrain_height: f32,
    water_surface_height: Option<f32>,
    channel_floor_height: Option<f32>,
    saturation: f32,
    mode: HydrologyMode,
    influence: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct RegionHydrologySignals {
    floodplain_bias: f32,
    lake_bias: f32,
    wetland_bias: f32,
    dryland_bias: f32,
    outlet_bias: f32,
    incision_bias: f32,
    transition_softness: f32,
    lateral_variability: f32,
    width_variability: f32,
    depth_variability: f32,
    meander_bias: f32,
    outer_spread_bias: f32,
    confinement_bias: f32,
}

#[derive(Debug, Clone, Copy)]
struct ProjectedSegmentPoint {
    signed_distance_blocks: f32,
    longitudinal_error_blocks: f32,
    segment_t: f32,
    segment_length_blocks: f32,
}

fn corridor_hydrology_response(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationV2Inputs,
    field: crate::world::atlas::AtlasCell,
    region_signals: RegionHydrologySignals,
    local_x: f32,
    local_z: f32,
    smoothed: super::smoothing::SmoothedColumn,
    corridor: RiverCorridorConstraint,
) -> Option<CorridorHydrologyResponse> {
    let projected = project_point_onto_segment(
        (local_x, local_z),
        (corridor.start_x, corridor.start_z),
        (corridor.end_x, corridor.end_z),
    );
    let positive_concavity = sanitize_unit_interval(smoothed.concavity.max(0.0) / 3.0, 0.0);
    let slope_signal = sanitize_unit_interval(smoothed.local_slope / 4.5, 0.0);

    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let world_x = chunk_origin_x as f32 + local_x;
    let world_z = chunk_origin_z as f32 + local_z;
    let start_world_x = chunk_origin_x as f32 + corridor.start_x;
    let start_world_z = chunk_origin_z as f32 + corridor.start_z;
    let end_world_x = chunk_origin_x as f32 + corridor.end_x;
    let end_world_z = chunk_origin_z as f32 + corridor.end_z;
    let start_field = sample_atlas_fields_fractional(&inputs.atlas_fields, start_world_x, start_world_z);
    let end_field = sample_atlas_fields_fractional(&inputs.atlas_fields, end_world_x, end_world_z);
    let start_region = blended_region_hydrology_signals(&sample_region_weights(
        &inputs.region_classes,
        start_world_x,
        start_world_z,
    ));
    let end_region = blended_region_hydrology_signals(&sample_region_weights(
        &inputs.region_classes,
        end_world_x,
        end_world_z,
    ));
    let along_t = smootherstep01(projected.segment_t);
    let centerline_world_x = lerp_f32(start_world_x, end_world_x, along_t);
    let centerline_world_z = lerp_f32(start_world_z, end_world_z, along_t);
    let start_anchor = corridor_anchor_world_y(inputs, start_world_x, start_world_z, corridor);
    let end_anchor = corridor_anchor_world_y(inputs, end_world_x, end_world_z, corridor);
    let base_channel_radius_start = final_channel_radius(corridor, start_field, start_region);
    let base_channel_radius_end = final_channel_radius(corridor, end_field, end_region);
    let base_channel_radius = lerp_f32(base_channel_radius_start, base_channel_radius_end, along_t);
    let water_radius_start =
        final_water_radius(base_channel_radius_start, corridor, start_field, start_region);
    let water_radius_end =
        final_water_radius(base_channel_radius_end, corridor, end_field, end_region);
    let base_water_radius = lerp_f32(water_radius_start, water_radius_end, along_t);
    let floodplain_radius_start =
        final_floodplain_radius(base_channel_radius_start, start_field, start_region);
    let floodplain_radius_end =
        final_floodplain_radius(base_channel_radius_end, end_field, end_region);
    let base_floodplain_radius = lerp_f32(floodplain_radius_start, floodplain_radius_end, along_t);
    let segment_floodplain_bias =
        lerp_f32(start_region.floodplain_bias, end_region.floodplain_bias, along_t);
    let segment_dryland_bias = lerp_f32(start_region.dryland_bias, end_region.dryland_bias, along_t);
    let segment_outlet_bias = lerp_f32(start_region.outlet_bias, end_region.outlet_bias, along_t);
    let segment_wetness = lerp_f32(start_field.wetness, end_field.wetness, along_t);
    let segment_riverine = lerp_f32(start_field.riverine_factor, end_field.riverine_factor, along_t);
    let segment_slope = lerp_f32(start_field.slope, end_field.slope, along_t);
    let lower_reach_signal = (segment_floodplain_bias * 0.24
        + segment_wetness * 0.18
        + segment_riverine * 0.20
        + segment_outlet_bias * 0.20
        + field.lake_potential * 0.06
        - segment_slope * 0.16
        - segment_dryland_bias * 0.08
        - corridor.downstream_grade_per_block * 28.0)
        .clamp(0.0, 1.0);
    let branch_seed = branch_noise_seed(corridor);
    let (edge_warp_x, edge_warp_z) = seedless_domain_warp(
        world_x,
        world_z,
        lerp_f32(18.0, 132.0, lower_reach_signal),
        lerp_f32(2.5, 18.0, lower_reach_signal),
        HYDRO_SALT_WARP_X ^ branch_seed,
        HYDRO_SALT_WARP_Z ^ branch_seed,
    );
    let (center_warp_x, center_warp_z) = seedless_domain_warp(
        centerline_world_x,
        centerline_world_z,
        lerp_f32(26.0, 176.0, lower_reach_signal),
        lerp_f32(3.0, 15.0, lower_reach_signal),
        (HYDRO_SALT_WARP_X ^ branch_seed).rotate_left(11),
        (HYDRO_SALT_WARP_Z ^ branch_seed).rotate_left(17),
    );
    let width_noise = seedless_value_fbm(
        center_warp_x,
        center_warp_z,
        lerp_f32(14.0, 84.0, lower_reach_signal),
        4,
        2.05,
        0.55,
        HYDRO_SALT_WIDTH ^ branch_seed,
    );
    let depth_noise = seedless_ridged_fbm(
        center_warp_x + 71.0,
        center_warp_z - 43.0,
        lerp_f32(
            12.0,
            72.0,
            lower_reach_signal * 0.55 + region_signals.depth_variability * 0.45,
        ),
        4,
        2.0,
        0.52,
        HYDRO_SALT_DEPTH ^ branch_seed,
    );
    let transition_noise = seedless_value_fbm(
        edge_warp_x - 103.0,
        edge_warp_z + 61.0,
        lerp_f32(22.0, 220.0, lower_reach_signal),
        4,
        2.0,
        0.50,
        HYDRO_SALT_TRANSITION ^ branch_seed,
    );
    let outer_noise = seedless_value_fbm(
        edge_warp_x + 139.0,
        edge_warp_z - 91.0,
        lerp_f32(28.0, 260.0, lower_reach_signal),
        4,
        2.0,
        0.48,
        HYDRO_SALT_OUTER ^ branch_seed,
    );
    let asymmetry_noise = seedless_value_noise_signed(
        center_warp_x + 23.0,
        center_warp_z - 17.0,
        lerp_f32(18.0, 144.0, lower_reach_signal),
        HYDRO_SALT_ASYMMETRY ^ branch_seed,
    );
    let edge_noise = seedless_value_fbm(
        edge_warp_x,
        edge_warp_z,
        lerp_f32(10.0, 48.0, lower_reach_signal),
        3,
        2.0,
        0.55,
        HYDRO_SALT_EDGE ^ branch_seed,
    );
    let meander_offset = centerline_meander_offset_blocks(
        corridor,
        along_t,
        projected.segment_length_blocks,
        centerline_world_x,
        centerline_world_z,
        base_channel_radius,
        base_floodplain_radius,
        segment_floodplain_bias,
        segment_wetness,
        segment_riverine,
        segment_dryland_bias,
        segment_slope,
        region_signals.meander_bias,
        region_signals.lateral_variability,
        region_signals.confinement_bias,
        lower_reach_signal,
        branch_seed,
    );
    let signed_distance_blocks = projected.signed_distance_blocks - meander_offset;
    let confinement = (0.18
        + region_signals.incision_bias * 0.28
        + region_signals.confinement_bias * 0.34
        + slope_signal * 0.18
        - region_signals.transition_softness * 0.12)
        .clamp(0.08, 1.15);
    let lateral_width_mod = (width_noise * (0.14 + region_signals.width_variability * 0.34)
        + edge_noise * (0.06 + region_signals.lateral_variability * 0.16)
        + lower_reach_signal * 0.06)
        .clamp(-0.48, 0.72);
    let side_asymmetry = (asymmetry_noise * (0.12 + region_signals.lateral_variability * 0.20))
        .clamp(-0.36, 0.36);
    let side_scale = if signed_distance_blocks >= 0.0 {
        1.0 + side_asymmetry
    } else {
        1.0 - side_asymmetry
    }
    .clamp(0.72, 1.30);
    let channel_radius = (base_channel_radius * (1.0 + lateral_width_mod) * side_scale).clamp(
        match corridor.kind {
            RiverPathKind::Trunk => 4.0,
            RiverPathKind::Tributary => 2.6,
        },
        corridor.half_width_blocks * 0.38
            + match corridor.kind {
                RiverPathKind::Trunk => 11.0,
                RiverPathKind::Tributary => 7.0,
            },
    );
    let water_width_mod = (width_noise * (0.18 + region_signals.width_variability * 0.18)
        + edge_noise * 0.12
        - confinement * 0.05)
        .clamp(-0.36, 0.34);
    let water_radius = (base_water_radius * (0.82 + water_width_mod) * side_scale.clamp(0.78, 1.12))
        .clamp(1.4, channel_radius * 0.72);
    let bank_span = ((2.2
        + channel_radius * (0.20 + region_signals.transition_softness * 0.12 + lower_reach_signal * 0.10)
        + region_signals.outer_spread_bias * 1.4
        - confinement * 0.85)
        * (1.0 + transition_noise * (0.08 + region_signals.lateral_variability * 0.10)))
        .clamp(1.6, channel_radius * 1.3 + 8.0);
    let bank_radius_min = channel_radius + 1.2;
    let bank_radius_max =
        (corridor.half_width_blocks * 0.56 + lerp_f32(8.0, 18.0, lower_reach_signal))
            .max(bank_radius_min);
    let bank_radius = (channel_radius + bank_span).clamp(bank_radius_min, bank_radius_max);
    let floodplain_span = ((3.2
        + region_signals.floodplain_bias * 4.8
        + region_signals.outer_spread_bias * 4.6
        + lower_reach_signal * 10.0
        + segment_wetness * 2.2
        - region_signals.dryland_bias * 1.8)
        * (1.0
            + transition_noise * (0.12 + region_signals.width_variability * 0.16)
            + outer_noise.max(0.0) * lerp_f32(0.04, 0.20, lower_reach_signal)))
        .clamp(
            2.4,
            corridor.half_width_blocks * (0.18 + lower_reach_signal * 0.40) + 28.0,
        );
    let floodplain_radius_min = bank_radius + 2.4;
    let floodplain_radius_max = (corridor.half_width_blocks * (0.78 + lower_reach_signal * 0.72)
        + lerp_f32(8.0, 34.0, lower_reach_signal))
    .max(floodplain_radius_min);
    let floodplain_radius =
        (bank_radius + floodplain_span).clamp(floodplain_radius_min, floodplain_radius_max);
    let outer_span = ((4.0
        + region_signals.transition_softness * 7.2
        + region_signals.outer_spread_bias * 11.0
        + lower_reach_signal * 22.0
        - region_signals.confinement_bias * 3.5)
        + outer_noise.abs() * lerp_f32(4.0, 44.0, lower_reach_signal))
        .clamp(
            3.0,
            corridor.half_width_blocks * (0.16 + lower_reach_signal * 0.72)
                + lerp_f32(10.0, 72.0, lower_reach_signal),
        );
    let outer_radius_min = floodplain_radius + 3.0;
    let outer_radius_max = (corridor.half_width_blocks * (0.95 + lower_reach_signal * 1.10)
        + lerp_f32(12.0, 72.0, lower_reach_signal))
    .max(outer_radius_min);
    let outer_radius = (floodplain_radius + outer_span).clamp(outer_radius_min, outer_radius_max);
    let wet_margin_radius = outer_radius
        + lerp_f32(
            2.0,
            14.0,
            (region_signals.wetland_bias * 0.55 + lower_reach_signal * 0.45).clamp(0.0, 1.0),
        ) * (1.0 + edge_noise.abs() * 0.12);
    let distance_blocks =
        (signed_distance_blocks * signed_distance_blocks + projected.longitudinal_error_blocks.powi(2))
            .sqrt();
    let core_influence = radial_influence(distance_blocks, channel_radius, 0.88 + confinement * 0.60);
    let water_sheet_influence =
        radial_influence(distance_blocks, water_radius, 0.74 + confinement * 0.18);
    let bank_influence = radial_influence(
        distance_blocks,
        bank_radius,
        0.96 + region_signals.confinement_bias * 0.44 - region_signals.transition_softness * 0.12,
    );
    let floodplain_influence = radial_influence(
        distance_blocks,
        floodplain_radius,
        0.70 + region_signals.transition_softness * 0.24,
    );
    let outer_influence = radial_influence(
        distance_blocks,
        outer_radius,
        0.58 + region_signals.transition_softness * 0.18,
    );
    let wet_margin_influence = radial_influence(distance_blocks, wet_margin_radius, 0.48);
    let influence = (core_influence * 1.14
        + bank_influence * 0.34
        + floodplain_influence * 0.20
        + outer_influence * 0.12
        + wet_margin_influence * 0.06)
        .clamp(0.0, 1.0);

    if !influence.is_finite() || influence <= 0.018 {
        return None;
    }

    let flood_allowance = region_signals.lake_bias * 0.32
        + region_signals.floodplain_bias * 0.18
        + region_signals.transition_softness * 0.12
        + positive_concavity * 0.24;
    let bank_clearance =
        (0.78 - flood_allowance * 0.20 + slope_signal * 0.24 + region_signals.incision_bias * 0.08)
            .clamp(0.42, 1.30);
    let anchored_water_surface = lerp_f32(start_anchor, end_anchor, along_t)
        .min(smoothed.height - bank_clearance);

    let start_channel_depth =
        final_channel_depth(corridor, start_field, start_region, positive_concavity);
    let end_channel_depth = final_channel_depth(corridor, end_field, end_region, positive_concavity);
    let start_floodplain_lowering =
        final_floodplain_lowering(start_field, start_region, positive_concavity, slope_signal);
    let end_floodplain_lowering =
        final_floodplain_lowering(end_field, end_region, positive_concavity, slope_signal);
    let base_channel_depth = lerp_f32(start_channel_depth, end_channel_depth, along_t);
    let local_channel_depth = (base_channel_depth
        * (1.0 + depth_noise * (0.12 + region_signals.depth_variability * 0.28))
        * (1.0 + region_signals.incision_bias * 0.10 + region_signals.confinement_bias * 0.08)
        + region_signals.incision_bias * 0.90
        - region_signals.transition_softness * 0.24)
        .clamp(
            match corridor.kind {
                RiverPathKind::Trunk => 3.0,
                RiverPathKind::Tributary => 2.2,
            },
            14.5,
        );
    let floodplain_lowering_base = lerp_f32(start_floodplain_lowering, end_floodplain_lowering, along_t)
        * (1.0 + transition_noise * (0.16 + region_signals.transition_softness * 0.18))
        + outer_noise.max(0.0) * 0.28;
    let max_cut_depth = (2.8
        + smoothed.remaining_relief_budget * (0.80 + region_signals.incision_bias * 0.12))
        .clamp(2.8, 18.0);
    let core_floor_target = (anchored_water_surface - local_channel_depth)
        .max(smoothed.height - max_cut_depth)
        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
    let core_total_cut = (smoothed.height - core_floor_target).clamp(0.0, max_cut_depth);
    let outer_total_cut = (floodplain_lowering_base
        * (0.32 + region_signals.transition_softness * 0.26 + region_signals.outer_spread_bias * 0.20)
        + core_total_cut * (0.04 + lower_reach_signal * 0.10)
        + edge_noise.max(0.0) * 0.18)
        .clamp(0.0, core_total_cut * 0.46);
    let flood_total_cut_min = outer_total_cut + 0.10;
    let flood_total_cut_max = (core_total_cut * 0.64).max(flood_total_cut_min);
    let flood_total_cut = (floodplain_lowering_base
        * (0.84 + region_signals.transition_softness * 0.24 + region_signals.outer_spread_bias * 0.10)
        + core_total_cut * (0.06 + region_signals.floodplain_bias * 0.10 + lower_reach_signal * 0.08))
        .clamp(flood_total_cut_min, flood_total_cut_max);
    let bank_total_cut_min = flood_total_cut + 0.14;
    let bank_total_cut_max = (core_total_cut * 0.92).max(bank_total_cut_min);
    let bank_total_cut = (core_total_cut
        * (0.54 + confinement * 0.18 + region_signals.depth_variability * 0.06
            - region_signals.transition_softness * 0.05)
        + floodplain_lowering_base * 0.24
        + depth_noise.max(0.0) * 0.18)
        .clamp(bank_total_cut_min, bank_total_cut_max);
    let outer_delta = outer_total_cut;
    let flood_delta = (flood_total_cut - outer_total_cut).max(0.0);
    let bank_delta = (bank_total_cut - flood_total_cut).max(0.0);
    let core_delta = (core_total_cut - bank_total_cut).max(0.0);
    let terrain_cut = outer_delta * outer_influence
        + flood_delta * floodplain_influence
        + bank_delta * bank_influence
        + core_delta * core_influence;
    let terrain_height = (smoothed.height - terrain_cut)
        .min(smoothed.height)
        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
    let floor_hold_influence = radial_influence(
        distance_blocks,
        bank_radius.max(water_radius + 0.6),
        0.86 + confinement * 0.18,
    );
    let channel_floor_y = lerp_f32(terrain_height, core_floor_target, floor_hold_influence)
        .min(terrain_height)
        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);

    let basin_signal = sanitize_unit_interval(
        field.lake_potential * 0.42
        + field.basinness * 0.26
        + region_signals.lake_bias * 0.22
        + positive_concavity * 0.24
        - field.aridity * 0.14
        - slope_signal * 0.10,
        0.0,
    );
    let saturation = sanitize_unit_interval(
        wet_margin_influence * 0.22
        + outer_influence * 0.18
        + floodplain_influence * 0.22
        + bank_influence * 0.06
        + field.wetness * 0.24
        + field.wetland_factor * 0.12
        + region_signals.wetland_bias * 0.24
        + positive_concavity * 0.15
        + basin_signal * 0.08
        - field.aridity * 0.20,
        0.0,
    );

    let lake_bias = basin_signal * floodplain_influence.max(outer_influence * 0.65);
    let water_presence = sanitize_unit_interval(
        water_sheet_influence * 1.20
        + core_influence * 0.14
        + bank_influence * 0.08
        + lake_bias * 0.36
        + region_signals.wetland_bias * 0.05
        + positive_concavity * 0.04,
        0.0,
    );

    let channel_has_visible_water =
        water_sheet_influence >= 0.14 || (distance_blocks <= water_radius * 1.05 && bank_influence >= 0.25);
    let standing_water = if water_presence >= 0.18 && (channel_has_visible_water || lake_bias >= 0.52) {
        let max_water_depth = (0.85
            + water_radius.sqrt() * 0.44
            + region_signals.lake_bias * 1.10
            + region_signals.transition_softness * 0.24)
            .clamp(0.85, 5.0);
        let desired_water_depth = (MIN_VISIBLE_WATER_DEPTH
            + local_channel_depth * (0.22 + field.river_flow_potential * 0.04)
            + corridor.downstream_grade_per_block * 560.0
            + field.river_flow_potential * 0.55
            + depth_noise.max(0.0) * 0.20
            + water_sheet_influence * 0.18)
            .clamp(MIN_VISIBLE_WATER_DEPTH, 4.2);
        let max_supported_surface = anchored_water_surface.min(terrain_height + max_water_depth);
        let available_depth = max_supported_surface - channel_floor_y;
        let visible_headroom = max_supported_surface - terrain_height;
        if available_depth >= MIN_VISIBLE_WATER_DEPTH && visible_headroom >= MIN_VISIBLE_WATER_DEPTH {
            Some(
                (channel_floor_y + desired_water_depth.min(available_depth))
                    .min(max_supported_surface)
                    .max(terrain_height + MIN_VISIBLE_WATER_DEPTH),
            )
        } else {
            None
        }
    } else {
        None
    };

    let mode = if standing_water.is_some() {
        if lake_bias >= core_influence.max(0.24) && basin_signal >= 0.50 {
            HydrologyMode::Lake
        } else if channel_has_visible_water {
            HydrologyMode::Channel
        } else {
            HydrologyMode::Floodplain
        }
    } else if saturation >= 0.56 && floodplain_influence >= 0.34 {
        HydrologyMode::Wetland
    } else if floodplain_influence >= 0.40 || outer_influence >= 0.52 {
        HydrologyMode::Floodplain
    } else {
        HydrologyMode::Dry
    };

    let channel_floor_height = (core_influence >= 0.18 || standing_water.is_some())
        .then_some(channel_floor_y.clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y));
    let water_surface_height =
        standing_water.map(|height| height.clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y));

    Some(CorridorHydrologyResponse {
        branch_key: BranchKey {
            river_id: corridor.river_id,
            kind_rank: kind_rank(corridor.kind),
            order: corridor.order,
        },
        terrain_height,
        water_surface_height,
        channel_floor_height,
        saturation,
        mode,
        influence,
    })
}

fn basin_fallback_column(
    field: crate::world::atlas::AtlasCell,
    region_signals: RegionHydrologySignals,
    smoothed: super::smoothing::SmoothedColumn,
) -> HydrologyColumn {
    let positive_concavity = sanitize_unit_interval(smoothed.concavity.max(0.0) / 3.0, 0.0);
    let slope_signal = sanitize_unit_interval(smoothed.local_slope / 4.5, 0.0);
    let pond_signal = sanitize_unit_interval(
        field.lake_potential * 0.36
        + field.basinness * 0.24
        + field.wetness * 0.18
        + region_signals.lake_bias * 0.28
        + region_signals.wetland_bias * 0.12
        + positive_concavity * 0.32
        - field.aridity * 0.22
        - slope_signal * 0.26,
        0.0,
    );
    let saturation = sanitize_unit_interval(
        field.wetness * 0.32
        + field.wetland_factor * 0.18
        + region_signals.wetland_bias * 0.24
        + positive_concavity * 0.20
        + pond_signal * 0.18
        - field.aridity * 0.22,
        0.0,
    );

    if pond_signal >= 0.74 {
        let water_surface_y = (macro_elevation_to_world_y(field.macro_elevation)
            - 15.0
            - field.wetness * 2.2
            - region_signals.lake_bias * 1.6)
            .min(smoothed.height - 0.10 + pond_signal * 0.48)
            .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
        let max_cut_depth = (1.8 + smoothed.remaining_relief_budget * 0.46).clamp(1.8, 6.0);
        let channel_floor_height = (water_surface_y - (0.75 + pond_signal * 1.25))
            .max(smoothed.height - max_cut_depth)
            .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);

        return HydrologyColumn {
            terrain_height: channel_floor_height,
            water_surface_height: Some((channel_floor_height + 0.65 + pond_signal * 0.90).clamp(
                MIN_HYDROLOGY_Y,
                MAX_HYDROLOGY_Y,
            )),
            channel_floor_height: Some(channel_floor_height),
            saturation,
            mode: HydrologyMode::Lake,
        };
    }

    HydrologyColumn {
        terrain_height: smoothed.height.clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y),
        water_surface_height: None,
        channel_floor_height: None,
        saturation,
        mode: if saturation >= 0.58 {
            HydrologyMode::Wetland
        } else {
            HydrologyMode::Dry
        },
    }
}

fn blended_region_hydrology_signals(
    region_samples: &[RegionSampleWeight; 4],
) -> RegionHydrologySignals {
    let mut signals = RegionHydrologySignals::default();
    let mut total_weight = 0.0_f32;

    for sample in region_samples {
        if sample.weight <= f32::EPSILON {
            continue;
        }

        total_weight += sample.weight;
        signals.floodplain_bias += match sample.cell.hydrology_context {
            HydrologyContext::RiverCorridor => 0.88,
            HydrologyContext::WetLowland => 1.00,
            HydrologyContext::LakeBasin => 0.42,
            HydrologyContext::WellDrained => 0.16,
            HydrologyContext::Dryland => 0.04,
        } * sample.weight;
        signals.lake_bias += match sample.cell.hydrology_context {
            HydrologyContext::LakeBasin => 1.00,
            HydrologyContext::WetLowland => 0.40,
            HydrologyContext::RiverCorridor => 0.18,
            HydrologyContext::WellDrained => 0.05,
            HydrologyContext::Dryland => 0.0,
        } * sample.weight;
        signals.wetland_bias += match sample.cell.hydrology_context {
            HydrologyContext::WetLowland => 1.00,
            HydrologyContext::LakeBasin => 0.60,
            HydrologyContext::RiverCorridor => 0.38,
            HydrologyContext::WellDrained => 0.10,
            HydrologyContext::Dryland => 0.0,
        } * sample.weight;
        signals.dryland_bias += match sample.cell.hydrology_context {
            HydrologyContext::Dryland => 1.00,
            HydrologyContext::WellDrained => 0.32,
            HydrologyContext::RiverCorridor => 0.08,
            HydrologyContext::WetLowland => 0.02,
            HydrologyContext::LakeBasin => 0.0,
        } * sample.weight;
        signals.outlet_bias += match sample.cell.coastal_context {
            CoastalContext::Marine => 1.00,
            CoastalContext::Coastal => 0.80,
            CoastalContext::NearCoast => 0.42,
            CoastalContext::Inland => 0.0,
        } * sample.weight;
        signals.incision_bias += match sample.cell.hydrology_context {
            HydrologyContext::Dryland => 0.62,
            HydrologyContext::WellDrained => 0.44,
            HydrologyContext::RiverCorridor => 0.34,
            HydrologyContext::WetLowland => 0.12,
            HydrologyContext::LakeBasin => 0.18,
        } * sample.weight;
        signals.transition_softness += match sample.cell.hydrology_context {
            HydrologyContext::WetLowland => 0.82,
            HydrologyContext::LakeBasin => 0.70,
            HydrologyContext::RiverCorridor => 0.38,
            HydrologyContext::WellDrained => 0.30,
            HydrologyContext::Dryland => 0.20,
        } * sample.weight;
        signals.lateral_variability += match sample.cell.hydrology_context {
            HydrologyContext::WetLowland => 0.58,
            HydrologyContext::RiverCorridor => 0.54,
            HydrologyContext::Dryland => 0.52,
            HydrologyContext::WellDrained => 0.46,
            HydrologyContext::LakeBasin => 0.42,
        } * sample.weight;
        signals.width_variability += match sample.cell.hydrology_context {
            HydrologyContext::WetLowland => 0.72,
            HydrologyContext::Dryland => 0.54,
            HydrologyContext::RiverCorridor => 0.52,
            HydrologyContext::WellDrained => 0.48,
            HydrologyContext::LakeBasin => 0.46,
        } * sample.weight;
        signals.depth_variability += match sample.cell.hydrology_context {
            HydrologyContext::Dryland => 0.62,
            HydrologyContext::WellDrained => 0.56,
            HydrologyContext::RiverCorridor => 0.54,
            HydrologyContext::LakeBasin => 0.40,
            HydrologyContext::WetLowland => 0.32,
        } * sample.weight;
        signals.meander_bias += match sample.cell.hydrology_context {
            HydrologyContext::RiverCorridor => 0.56,
            HydrologyContext::WetLowland => 0.48,
            HydrologyContext::WellDrained => 0.48,
            HydrologyContext::Dryland => 0.42,
            HydrologyContext::LakeBasin => 0.18,
        } * sample.weight;
        signals.outer_spread_bias += match sample.cell.hydrology_context {
            HydrologyContext::WetLowland => 0.78,
            HydrologyContext::LakeBasin => 0.52,
            HydrologyContext::RiverCorridor => 0.34,
            HydrologyContext::WellDrained => 0.24,
            HydrologyContext::Dryland => 0.16,
        } * sample.weight;
        signals.confinement_bias += match sample.cell.hydrology_context {
            HydrologyContext::Dryland => 0.56,
            HydrologyContext::WellDrained => 0.38,
            HydrologyContext::RiverCorridor => 0.36,
            HydrologyContext::LakeBasin => 0.18,
            HydrologyContext::WetLowland => 0.16,
        } * sample.weight;

        match sample.cell.terrain_form_family {
            TerrainFormFamily::Delta
            | TerrainFormFamily::Floodplain
            | TerrainFormFamily::WetLowland
            | TerrainFormFamily::AlluvialLowland => {
                signals.floodplain_bias += sample.weight * 0.32;
                signals.wetland_bias += sample.weight * 0.18;
                signals.transition_softness += sample.weight * 0.28;
                signals.lateral_variability += sample.weight * 0.12;
                signals.width_variability += sample.weight * 0.18;
                signals.meander_bias += sample.weight * 0.18;
                signals.outer_spread_bias += sample.weight * 0.34;
            }
            TerrainFormFamily::Basin => {
                signals.lake_bias += sample.weight * 0.28;
                signals.wetland_bias += sample.weight * 0.12;
                signals.transition_softness += sample.weight * 0.18;
                signals.outer_spread_bias += sample.weight * 0.20;
            }
            TerrainFormFamily::BroadValley => {
                signals.floodplain_bias += sample.weight * 0.20;
                signals.transition_softness += sample.weight * 0.16;
                signals.meander_bias += sample.weight * 0.18;
                signals.width_variability += sample.weight * 0.10;
                signals.outer_spread_bias += sample.weight * 0.16;
            }
            TerrainFormFamily::NarrowValley => {
                signals.floodplain_bias += sample.weight * 0.14;
                signals.incision_bias += sample.weight * 0.22;
                signals.depth_variability += sample.weight * 0.12;
                signals.meander_bias += sample.weight * 0.12;
                signals.confinement_bias += sample.weight * 0.20;
                signals.transition_softness -= sample.weight * 0.08;
            }
            TerrainFormFamily::Canyon
            | TerrainFormFamily::RavineCountry
            | TerrainFormFamily::GlacialValley => {
                signals.incision_bias += sample.weight * 0.36;
                signals.depth_variability += sample.weight * 0.18;
                signals.confinement_bias += sample.weight * 0.30;
                signals.transition_softness -= sample.weight * 0.18;
                signals.outer_spread_bias -= sample.weight * 0.10;
                signals.meander_bias += sample.weight * 0.10;
            }
            TerrainFormFamily::MesaCountry | TerrainFormFamily::Badlands => {
                signals.dryland_bias += sample.weight * 0.20;
                signals.incision_bias += sample.weight * 0.22;
                signals.depth_variability += sample.weight * 0.18;
                signals.transition_softness -= sample.weight * 0.12;
                signals.outer_spread_bias -= sample.weight * 0.08;
            }
            TerrainFormFamily::Escarpment => {
                signals.incision_bias += sample.weight * 0.18;
                signals.depth_variability += sample.weight * 0.10;
                signals.transition_softness -= sample.weight * 0.10;
                signals.confinement_bias += sample.weight * 0.10;
            }
            TerrainFormFamily::Pediment | TerrainFormFamily::AlluvialFan => {
                signals.width_variability += sample.weight * 0.12;
                signals.transition_softness += sample.weight * 0.08;
                signals.outer_spread_bias += sample.weight * 0.06;
            }
            TerrainFormFamily::MountainFront
            | TerrainFormFamily::Mountain
            | TerrainFormFamily::HillCluster
            | TerrainFormFamily::RidgeCountry => {
                signals.incision_bias += sample.weight * 0.18;
                signals.depth_variability += sample.weight * 0.12;
                signals.confinement_bias += sample.weight * 0.18;
                signals.transition_softness -= sample.weight * 0.08;
            }
            _ => {}
        }

        match sample.cell.biome_family {
            BiomeFamily::Marsh
            | BiomeFamily::Swamp
            | BiomeFamily::FloodedForest
            | BiomeFamily::Mangrove
            | BiomeFamily::LagoonCoast
            | BiomeFamily::EstuarineCoast => {
                signals.transition_softness += sample.weight * 0.24;
                signals.outer_spread_bias += sample.weight * 0.18;
                signals.width_variability += sample.weight * 0.10;
                signals.meander_bias += sample.weight * 0.08;
            }
            BiomeFamily::Desert
            | BiomeFamily::SemiDesert
            | BiomeFamily::Steppe
            | BiomeFamily::DryShrubland
            | BiomeFamily::MediterraneanShrubland => {
                signals.incision_bias += sample.weight * 0.16;
                signals.depth_variability += sample.weight * 0.16;
                signals.width_variability += sample.weight * 0.14;
                signals.transition_softness -= sample.weight * 0.08;
                signals.outer_spread_bias -= sample.weight * 0.06;
            }
            BiomeFamily::TemperateBroadleafForest
            | BiomeFamily::TemperateMixedForest
            | BiomeFamily::TemperateRainforest
            | BiomeFamily::BorealForest
            | BiomeFamily::TropicalDryForest
            | BiomeFamily::TropicalRainforest
            | BiomeFamily::MonsoonForest => {
                signals.transition_softness += sample.weight * 0.10;
                signals.meander_bias += sample.weight * 0.06;
                signals.lateral_variability += sample.weight * 0.08;
            }
            BiomeFamily::SubalpineWoodland
            | BiomeFamily::AlpineMeadow
            | BiomeFamily::Tundra
            | BiomeFamily::PolarBarrens
            | BiomeFamily::PolarIce => {
                signals.incision_bias += sample.weight * 0.12;
                signals.confinement_bias += sample.weight * 0.12;
                signals.depth_variability += sample.weight * 0.10;
                signals.transition_softness -= sample.weight * 0.06;
                signals.meander_bias += sample.weight * 0.06;
            }
            _ => {}
        }

        match sample.cell.relief_class {
            ReliefClass::Plain => {
                signals.transition_softness += sample.weight * 0.16;
                signals.width_variability += sample.weight * 0.14;
                signals.meander_bias += sample.weight * 0.18;
                signals.outer_spread_bias += sample.weight * 0.18;
            }
            ReliefClass::Rolling => {
                signals.width_variability += sample.weight * 0.10;
                signals.lateral_variability += sample.weight * 0.10;
                signals.meander_bias += sample.weight * 0.12;
            }
            ReliefClass::Hill => {
                signals.incision_bias += sample.weight * 0.16;
                signals.depth_variability += sample.weight * 0.12;
                signals.meander_bias += sample.weight * 0.08;
                signals.confinement_bias += sample.weight * 0.10;
            }
            ReliefClass::Mountain => {
                signals.incision_bias += sample.weight * 0.28;
                signals.depth_variability += sample.weight * 0.18;
                signals.meander_bias += sample.weight * 0.08;
                signals.confinement_bias += sample.weight * 0.22;
                signals.transition_softness -= sample.weight * 0.10;
            }
        }
    }

    if total_weight > f32::EPSILON {
        let inv = total_weight.recip();
        signals.floodplain_bias *= inv;
        signals.lake_bias *= inv;
        signals.wetland_bias *= inv;
        signals.dryland_bias *= inv;
        signals.outlet_bias *= inv;
        signals.incision_bias *= inv;
        signals.transition_softness *= inv;
        signals.lateral_variability *= inv;
        signals.width_variability *= inv;
        signals.depth_variability *= inv;
        signals.meander_bias *= inv;
        signals.outer_spread_bias *= inv;
        signals.confinement_bias *= inv;
    }

    signals.floodplain_bias = signals.floodplain_bias.clamp(0.0, 1.35);
    signals.lake_bias = signals.lake_bias.clamp(0.0, 1.35);
    signals.wetland_bias = signals.wetland_bias.clamp(0.0, 1.35);
    signals.dryland_bias = signals.dryland_bias.clamp(0.0, 1.35);
    signals.outlet_bias = signals.outlet_bias.clamp(0.0, 1.25);
    signals.incision_bias = (0.24 + signals.incision_bias).clamp(0.12, 1.35);
    signals.transition_softness = (0.20 + signals.transition_softness).clamp(0.12, 1.30);
    signals.lateral_variability = (0.26 + signals.lateral_variability).clamp(0.16, 1.30);
    signals.width_variability = (0.24 + signals.width_variability).clamp(0.14, 1.30);
    signals.depth_variability = (0.22 + signals.depth_variability).clamp(0.12, 1.30);
    signals.meander_bias = (0.24 + signals.meander_bias).clamp(0.14, 1.35);
    signals.outer_spread_bias = (0.18 + signals.outer_spread_bias).clamp(0.08, 1.35);
    signals.confinement_bias = (0.18 + signals.confinement_bias).clamp(0.08, 1.30);
    signals
}

fn corridor_anchor_world_y(
    inputs: &ChunkGenerationV2Inputs,
    world_x: f32,
    world_z: f32,
    corridor: RiverCorridorConstraint,
) -> f32 {
    let field = sample_atlas_fields_fractional(&inputs.atlas_fields, world_x, world_z);
    let region = blended_region_hydrology_signals(&sample_region_weights(
        &inputs.region_classes,
        world_x,
        world_z,
    ));
    let base = macro_elevation_to_world_y(field.macro_elevation);
    let width_pull = corridor.half_width_blocks.sqrt()
        * match corridor.kind {
            RiverPathKind::Trunk => 0.72,
            RiverPathKind::Tributary => 0.56,
        };
    let flow_pull = field.river_flow_potential * 5.8
        + field.wetness * 3.2
        + field.riverine_factor * 2.2
        + field.basinness * 1.8
        + region.floodplain_bias * 1.2
        + region.lake_bias * 1.4
        + region.outlet_bias * 0.6;
    let dry_raise = field.aridity * 1.8 + region.dryland_bias * 1.1;
    let outlet_pull = field.coast_factor * 2.1 + region.outlet_bias * 0.9;

    (base - 14.0 - width_pull - flow_pull - outlet_pull + dry_raise)
        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y)
}

fn final_channel_radius(
    corridor: RiverCorridorConstraint,
    field: crate::world::atlas::AtlasCell,
    region: RegionHydrologySignals,
) -> f32 {
    let base_width = corridor.half_width_blocks.max(1.0).sqrt() * 1.18;
    let kind_scale = match corridor.kind {
        RiverPathKind::Trunk => 0.92,
        RiverPathKind::Tributary => 0.74,
    };
    let flow_scale = 1.0
        + field.river_flow_potential * 0.16
        + field.riverine_factor * 0.10
        + field.wetness * 0.08
        + region.floodplain_bias * 0.06
        - region.dryland_bias * 0.10;
    let base_radius = base_width * kind_scale * flow_scale
        + match corridor.kind {
            RiverPathKind::Trunk => 1.7,
            RiverPathKind::Tributary => 1.1,
        };

    base_radius.clamp(
        match corridor.kind {
            RiverPathKind::Trunk => 5.0,
            RiverPathKind::Tributary => 3.4,
        },
        match corridor.kind {
            RiverPathKind::Trunk => base_width * 1.75 + 3.8,
            RiverPathKind::Tributary => base_width * 1.45 + 2.8,
        },
    )
}

fn final_floodplain_radius(
    channel_radius: f32,
    field: crate::world::atlas::AtlasCell,
    region: RegionHydrologySignals,
) -> f32 {
    let shoulder_extension = (3.2
        + region.floodplain_bias * 5.2
        + region.lake_bias * 1.4
        + field.wetness * 2.5
        - region.dryland_bias * 1.6)
        .clamp(2.8, 13.0);

    let min_radius = channel_radius + 3.0;
    let max_radius = (channel_radius * 2.35 + 7.0).max(min_radius);
    (channel_radius + shoulder_extension).clamp(min_radius, max_radius)
}

fn final_water_radius(
    channel_radius: f32,
    corridor: RiverCorridorConstraint,
    field: crate::world::atlas::AtlasCell,
    region: RegionHydrologySignals,
) -> f32 {
    let kind_bias = match corridor.kind {
        RiverPathKind::Trunk => 0.54,
        RiverPathKind::Tributary => 0.44,
    };
    let width_ratio = (kind_bias
        + field.river_flow_potential * 0.06
        + region.lake_bias * 0.05
        - region.dryland_bias * 0.05)
        .clamp(0.36, 0.62);

    (channel_radius * width_ratio + 0.8).clamp(1.8, channel_radius * 0.84)
}

fn final_channel_depth(
    corridor: RiverCorridorConstraint,
    field: crate::world::atlas::AtlasCell,
    region: RegionHydrologySignals,
    positive_concavity: f32,
) -> f32 {
    let kind_depth = match corridor.kind {
        RiverPathKind::Trunk => 3.8,
        RiverPathKind::Tributary => 2.6,
    };
    let erosion_energy = (corridor.downstream_grade_per_block / 0.0022).clamp(0.55, 3.8);

    (kind_depth
        + corridor.half_width_blocks.sqrt() * 0.60
        + erosion_energy * 2.10
        + field.river_flow_potential * 2.05
        + field.riverine_factor * 1.10
        + field.slope * 1.05
        + positive_concavity * 0.90
        + region.lake_bias * 0.16
        - region.dryland_bias * 0.36)
        .clamp(2.8, 12.5)
}

fn centerline_meander_offset_blocks(
    corridor: RiverCorridorConstraint,
    along_t: f32,
    segment_length_blocks: f32,
    centerline_world_x: f32,
    centerline_world_z: f32,
    channel_radius: f32,
    floodplain_radius: f32,
    floodplain_bias: f32,
    wetness: f32,
    riverine_factor: f32,
    dryland_bias: f32,
    slope: f32,
    meander_bias: f32,
    lateral_variability: f32,
    confinement_bias: f32,
    lower_reach_signal: f32,
    branch_seed: u64,
) -> f32 {
    let along_t = along_t.clamp(0.0, 1.0);
    let length_factor =
        (segment_length_blocks / lerp_f32(120.0, 340.0, lower_reach_signal)).clamp(0.80, 1.80);
    let meander_support = (0.28
        + meander_bias * 0.30
        + lateral_variability * 0.10
        + floodplain_bias * 0.18
        + wetness * 0.10
        + riverine_factor * 0.10
        - dryland_bias * 0.08
        - slope * 0.12
        - confinement_bias * 0.08
        - corridor.downstream_grade_per_block * 34.0)
        .clamp(0.10, 1.10);
    let kind_bonus = match corridor.kind {
        RiverPathKind::Trunk => 2.6,
        RiverPathKind::Tributary => 1.8,
    };
    let meander_limit = (corridor.half_width_blocks * (0.24 + lower_reach_signal * 0.20)
        + channel_radius * 0.95
        + 6.0)
        .min(floodplain_radius * 0.60)
        .max(0.0);
    let amplitude = ((channel_radius * (0.62 + meander_support * 0.78) + kind_bonus) * length_factor)
        .clamp(0.0, meander_limit);
    if amplitude <= f32::EPSILON {
        return 0.0;
    }

    let envelope = (PI * along_t)
        .sin()
        .max(0.0)
        .powf(lerp_f32(0.72, 1.08, lower_reach_signal));
    let branch_phase = ((branch_seed & 0xFFFF) as f32 / 65_535.0) * TAU;
    let base_cycles = 1.0
        + 2.0
            * (segment_length_blocks / lerp_f32(220.0, 520.0, lower_reach_signal))
                .floor()
                .clamp(0.0, 1.0);
    let primary_cycles = (base_cycles
        + lerp_f32(-0.18, 0.12, lower_reach_signal)
        + meander_bias * 0.10
        - confinement_bias * 0.06)
        .clamp(
            match corridor.kind {
                RiverPathKind::Trunk => 1.0,
                RiverPathKind::Tributary => 1.4,
            },
            match corridor.kind {
                RiverPathKind::Trunk => 3.4,
                RiverPathKind::Tributary => 3.8,
            },
        );
    let harmonic_sign = if ((corridor.river_id >> 1) & 1) == 0 {
        1.0
    } else {
        -1.0
    };
    let primary = (TAU * primary_cycles * along_t).sin();
    let secondary =
        (TAU * (primary_cycles + 1.35) * along_t + branch_phase).sin() * harmonic_sign;
    let tertiary = (TAU * (primary_cycles * 0.5 + 0.85) * along_t - branch_phase * 0.7).sin();
    let noise_wave = seedless_value_fbm(
        centerline_world_x + along_t * 57.0,
        centerline_world_z - along_t * 41.0,
        lerp_f32(18.0, 110.0, lower_reach_signal),
        4,
        2.0,
        0.55,
        HYDRO_SALT_CENTERLINE ^ branch_seed,
    );
    let wave = primary * 0.50 + secondary * 0.24 + tertiary * 0.12 + noise_wave * 0.22;

    amplitude * envelope * wave
}

fn final_floodplain_lowering(
    field: crate::world::atlas::AtlasCell,
    region: RegionHydrologySignals,
    positive_concavity: f32,
    slope_signal: f32,
) -> f32 {
    (0.22
        + field.wetness * 0.42
        + field.riverine_factor * 0.22
        + region.floodplain_bias * 0.34
        + region.wetland_bias * 0.16
        + positive_concavity * 0.18
        - slope_signal * 0.10
        - region.dryland_bias * 0.14)
        .clamp(0.10, 1.35)
}

fn macro_elevation_to_world_y(macro_elevation: f32) -> f32 {
    (SEA_LEVEL_Y as f32 - 8.0 + macro_elevation * 140.0).clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y)
}

fn branch_noise_seed(corridor: RiverCorridorConstraint) -> u64 {
    let branch_bits = ((corridor.river_id as u64) << 16)
        ^ ((corridor.order as u64) << 8)
        ^ kind_rank(corridor.kind) as u64;
    seedless_splitmix64(branch_bits ^ HYDRO_HASH_K1)
}

fn seedless_splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(HYDRO_HASH_K1);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn seedless_hash01(x: i64, z: i64, salt: u64) -> f32 {
    let x_bits = (x as u64).wrapping_mul(HYDRO_HASH_K2);
    let z_bits = (z as u64).wrapping_mul(HYDRO_HASH_K3);
    let bits = seedless_splitmix64(salt ^ x_bits ^ z_bits) >> 11;
    let max = ((1_u64 << 53) - 1) as f64;
    (bits as f64 / max) as f32
}

fn seedless_value_noise01(x: f32, z: f32, lattice_scale: f32, salt: u64) -> f32 {
    let sample_x = x / lattice_scale.max(1.0);
    let sample_z = z / lattice_scale.max(1.0);
    let x0 = sample_x.floor() as i64;
    let z0 = sample_z.floor() as i64;
    let tx = smootherstep01(sample_x - x0 as f32);
    let tz = smootherstep01(sample_z - z0 as f32);
    let n00 = seedless_hash01(x0, z0, salt);
    let n10 = seedless_hash01(x0 + 1, z0, salt);
    let n01 = seedless_hash01(x0, z0 + 1, salt);
    let n11 = seedless_hash01(x0 + 1, z0 + 1, salt);
    let nx0 = n00 + (n10 - n00) * tx;
    let nx1 = n01 + (n11 - n01) * tx;
    nx0 + (nx1 - nx0) * tz
}

fn seedless_value_noise_signed(x: f32, z: f32, lattice_scale: f32, salt: u64) -> f32 {
    seedless_value_noise01(x, z, lattice_scale, salt) * 2.0 - 1.0
}

fn seedless_value_fbm(
    x: f32,
    z: f32,
    base_scale: f32,
    octaves: u32,
    lacunarity: f32,
    gain: f32,
    salt: u64,
) -> f32 {
    let mut amplitude = 1.0_f32;
    let mut scale = base_scale.max(1.0);
    let mut total = 0.0_f32;
    let mut amplitude_sum = 0.0_f32;

    for octave in 0..octaves {
        let octave_salt = salt.wrapping_add((octave as u64).wrapping_mul(HYDRO_HASH_K1));
        total += seedless_value_noise_signed(x, z, scale, octave_salt) * amplitude;
        amplitude_sum += amplitude;
        amplitude *= gain;
        scale /= lacunarity.max(1.01);
    }

    if amplitude_sum <= f32::EPSILON {
        0.0
    } else {
        (total / amplitude_sum).clamp(-1.0, 1.0)
    }
}

fn seedless_ridged_fbm(
    x: f32,
    z: f32,
    base_scale: f32,
    octaves: u32,
    lacunarity: f32,
    gain: f32,
    salt: u64,
) -> f32 {
    let mut amplitude = 1.0_f32;
    let mut scale = base_scale.max(1.0);
    let mut total = 0.0_f32;
    let mut amplitude_sum = 0.0_f32;

    for octave in 0..octaves {
        let octave_salt = salt.wrapping_add((octave as u64).wrapping_mul(HYDRO_HASH_K2));
        let signal = seedless_value_noise_signed(x, z, scale, octave_salt);
        let ridged = 1.0 - signal.abs();
        total += (ridged * 2.0 - 1.0) * amplitude;
        amplitude_sum += amplitude;
        amplitude *= gain;
        scale /= lacunarity.max(1.01);
    }

    if amplitude_sum <= f32::EPSILON {
        0.0
    } else {
        (total / amplitude_sum).clamp(-1.0, 1.0)
    }
}

fn seedless_domain_warp(
    x: f32,
    z: f32,
    warp_scale: f32,
    amplitude: f32,
    salt_x: u64,
    salt_z: u64,
) -> (f32, f32) {
    let dx = seedless_value_fbm(x, z, warp_scale, 3, 2.0, 0.5, salt_x) * amplitude;
    let dz = seedless_value_fbm(x, z, warp_scale, 3, 2.0, 0.5, salt_z) * amplitude;
    (x + dx, z + dz)
}

fn radial_influence(distance: f32, radius: f32, sharpness: f32) -> f32 {
    if radius <= f32::EPSILON {
        return 0.0;
    }

    let t = (distance / radius).clamp(0.0, 1.0);
    (1.0 - smootherstep01(t)).powf(sharpness.clamp(0.38, 2.40))
}

fn column_index(local_x: i32, local_z: i32) -> usize {
    local_z as usize * CHUNK_EDGE_I32 as usize + local_x as usize
}

fn project_point_onto_segment(
    point: (f32, f32),
    start: (f32, f32),
    end: (f32, f32),
) -> ProjectedSegmentPoint {
    let seg_x = end.0 - start.0;
    let seg_z = end.1 - start.1;
    let length_sq = seg_x * seg_x + seg_z * seg_z;

    if length_sq <= f32::EPSILON {
        return ProjectedSegmentPoint {
            signed_distance_blocks: 0.0,
            longitudinal_error_blocks: 0.0,
            segment_t: 0.0,
            segment_length_blocks: 0.0,
        };
    }

    let segment_length_blocks = length_sq.sqrt();
    let tangent_x = seg_x / segment_length_blocks;
    let tangent_z = seg_z / segment_length_blocks;
    let normal_x = -tangent_z;
    let normal_z = tangent_x;
    let point_rel_x = point.0 - start.0;
    let point_rel_z = point.1 - start.1;
    let signed_distance_blocks = point_rel_x * normal_x + point_rel_z * normal_z;
    let projected_blocks = point_rel_x * tangent_x + point_rel_z * tangent_z;
    let clamped_blocks = projected_blocks.clamp(0.0, segment_length_blocks);
    let segment_t = (clamped_blocks / segment_length_blocks).clamp(0.0, 1.0);
    let longitudinal_error_blocks = if projected_blocks < 0.0 {
        -projected_blocks
    } else if projected_blocks > segment_length_blocks {
        projected_blocks - segment_length_blocks
    } else {
        0.0
    };

    ProjectedSegmentPoint {
        signed_distance_blocks,
        longitudinal_error_blocks,
        segment_t,
        segment_length_blocks,
    }
}

fn lerp_f32(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t.clamp(0.0, 1.0)
}

fn smootherstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn kind_rank(kind: RiverPathKind) -> u8 {
    match kind {
        RiverPathKind::Trunk => 0,
        RiverPathKind::Tributary => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::v2::{
        build_chunk_base_heightfield_prototype, build_chunk_corridor_window,
        build_chunk_meso_applied_prototype, build_chunk_realization_field_patch,
        build_chunk_smoothed_prototype, prepare_chunk_v2_inputs,
    };
    use crate::world::meta::WorldMeta;

    #[derive(Debug, Clone, Copy)]
    enum SharedAxis {
        X,
        Z,
    }

    fn build_hydrology(
        chunk: ChunkCoord,
        meta: &WorldMeta,
    ) -> (
        ChunkGenerationV2Inputs,
        ChunkCorridorWindow,
        SmoothedPrototype,
        HydrologySolve,
    ) {
        let inputs = prepare_chunk_v2_inputs(chunk, meta);
        let realization = build_chunk_realization_field_patch(chunk, &inputs);
        let corridor_window = build_chunk_corridor_window(chunk, &inputs);
        let prototype = build_chunk_base_heightfield_prototype(
            chunk,
            &inputs,
            &realization,
            &corridor_window,
        );
        let meso =
            build_chunk_meso_applied_prototype(chunk, &inputs, &corridor_window, &prototype);
        let smoothed = build_chunk_smoothed_prototype(chunk, &corridor_window, &meso);
        let hydrology = build_chunk_hydrology_solve(chunk, &inputs, &corridor_window, &smoothed);

        (inputs, corridor_window, smoothed, hydrology)
    }

    fn find_chunk_with_visible_water(
        meta: &WorldMeta,
    ) -> (
        ChunkCoord,
        ChunkGenerationV2Inputs,
        ChunkCorridorWindow,
        SmoothedPrototype,
        HydrologySolve,
    ) {
        let seed_chunks = [
            ChunkCoord(40, 0, -29),
            ChunkCoord(39, 0, -29),
            ChunkCoord(40, 0, -30),
            ChunkCoord(40, 0, -28),
            ChunkCoord(30, 0, -20),
            ChunkCoord(31, 0, -20),
            ChunkCoord(4, 0, -3),
            ChunkCoord(0, 0, 0),
            ChunkCoord(15, 0, 15),
            ChunkCoord(16, 0, 16),
            ChunkCoord(-1, 0, -1),
            ChunkCoord(127, 0, 0),
            ChunkCoord(128, 0, 0),
        ];

        for seed in seed_chunks {
            for offset_z in -2..=2 {
                for offset_x in -2..=2 {
                    let chunk = ChunkCoord(seed.0 + offset_x, 0, seed.2 + offset_z);
                    let (inputs, corridor_window, smoothed, hydrology) = build_hydrology(chunk, meta);
                    let water_columns = hydrology
                        .columns
                        .iter()
                        .filter(|column| column.water_surface_height.is_some())
                        .count();
                    if hydrology.connected_waterlines > 0 && water_columns >= 8 {
                        return (chunk, inputs, corridor_window, smoothed, hydrology);
                    }
                }
            }
        }

        panic!("expected at least one sampled chunk to produce visible hydrology");
    }

    fn find_chunk_with_channel_band(
        meta: &WorldMeta,
    ) -> (
        ChunkCoord,
        ChunkGenerationV2Inputs,
        ChunkCorridorWindow,
        SmoothedPrototype,
        HydrologySolve,
    ) {
        let seed_chunks = [
            ChunkCoord(40, 0, -29),
            ChunkCoord(39, 0, -29),
            ChunkCoord(40, 0, -30),
            ChunkCoord(40, 0, -28),
            ChunkCoord(30, 0, -20),
            ChunkCoord(31, 0, -20),
            ChunkCoord(4, 0, -3),
            ChunkCoord(0, 0, 0),
            ChunkCoord(15, 0, 15),
            ChunkCoord(16, 0, 16),
            ChunkCoord(-1, 0, -1),
            ChunkCoord(127, 0, 0),
            ChunkCoord(128, 0, 0),
        ];

        for seed in seed_chunks {
            for offset_z in -2..=2 {
                for offset_x in -2..=2 {
                    let chunk = ChunkCoord(seed.0 + offset_x, 0, seed.2 + offset_z);
                    let (inputs, corridor_window, smoothed, hydrology) = build_hydrology(chunk, meta);
                    let water_columns = hydrology
                        .columns
                        .iter()
                        .filter(|column| column.water_surface_height.is_some())
                        .count();
                    let channel_columns = hydrology
                        .columns
                        .iter()
                        .filter(|column| column.mode == HydrologyMode::Channel)
                        .count();
                    if hydrology.connected_waterlines > 0
                        && water_columns >= 64
                        && water_columns < 640
                        && channel_columns >= water_columns / 2
                    {
                        return (chunk, inputs, corridor_window, smoothed, hydrology);
                    }
                }
            }
        }

        panic!("expected at least one sampled chunk to produce a bounded visible channel band");
    }

    fn seam_has_shared_visible_water(
        left: &HydrologySolve,
        right: &HydrologySolve,
        axis: SharedAxis,
    ) -> bool {
        for row in 0..CHUNK_EDGE_I32 as usize {
            let (left_column, right_column) = match axis {
                SharedAxis::X => (
                    &left.columns[row * CHUNK_EDGE_I32 as usize + (CHUNK_EDGE_I32 as usize - 1)],
                    &right.columns[row * CHUNK_EDGE_I32 as usize],
                ),
                SharedAxis::Z => (
                    &left.columns[(CHUNK_EDGE_I32 as usize - 1) * CHUNK_EDGE_I32 as usize + row],
                    &right.columns[row],
                ),
            };
            if let (Some(left_water), Some(right_water)) =
                (left_column.water_surface_height, right_column.water_surface_height)
            {
                let water_delta = (left_water - right_water).abs();
                let terrain_delta = (left_column.terrain_height - right_column.terrain_height).abs();
                if water_delta <= 1.25 && terrain_delta <= 2.0 {
                    return true;
                }
            }
        }
        false
    }

    fn find_neighboring_chunk_pair_with_shared_water(
        meta: &WorldMeta,
    ) -> ((ChunkCoord, HydrologySolve), (ChunkCoord, HydrologySolve), SharedAxis) {
        let seed_chunks = [
            ChunkCoord(40, 0, -29),
            ChunkCoord(39, 0, -29),
            ChunkCoord(40, 0, -30),
            ChunkCoord(40, 0, -28),
            ChunkCoord(30, 0, -20),
            ChunkCoord(31, 0, -20),
            ChunkCoord(4, 0, -3),
            ChunkCoord(0, 0, 0),
            ChunkCoord(15, 0, 15),
            ChunkCoord(16, 0, 16),
            ChunkCoord(-1, 0, -1),
            ChunkCoord(127, 0, 0),
            ChunkCoord(128, 0, 0),
        ];

        for seed in seed_chunks {
            for offset_z in -2..=2 {
                for offset_x in -2..=2 {
                    let base_chunk = ChunkCoord(seed.0 + offset_x, 0, seed.2 + offset_z);
                    let (_, _, _, base_hydrology) = build_hydrology(base_chunk, meta);

                    let east_chunk = ChunkCoord(base_chunk.0 + 1, 0, base_chunk.2);
                    let (_, _, _, east_hydrology) = build_hydrology(east_chunk, meta);
                    if seam_has_shared_visible_water(&base_hydrology, &east_hydrology, SharedAxis::X) {
                        return (
                            (base_chunk, base_hydrology),
                            (east_chunk, east_hydrology),
                            SharedAxis::X,
                        );
                    }

                    let south_chunk = ChunkCoord(base_chunk.0, 0, base_chunk.2 + 1);
                    let (_, _, _, south_hydrology) = build_hydrology(south_chunk, meta);
                    if seam_has_shared_visible_water(&base_hydrology, &south_hydrology, SharedAxis::Z) {
                        return (
                            (base_chunk, base_hydrology),
                            (south_chunk, south_hydrology),
                            SharedAxis::Z,
                        );
                    }
                }
            }
        }

        panic!("expected at least one neighboring chunk pair to share visible water on the seam");
    }

    #[test]
    fn hydrology_is_deterministic_and_emits_a_full_grid() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(15, 0, 15);
        let (_, _, _, a) = build_hydrology(chunk, &meta);
        let (_, _, _, b) = build_hydrology(chunk, &meta);

        assert_eq!(a, b);
        assert_eq!(a.chunk, chunk);
        assert_eq!(a.columns.len(), (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize);
        assert!(a.columns.iter().all(|column| {
            column.terrain_height.is_finite()
                && column.channel_floor_height.unwrap_or(column.terrain_height).is_finite()
                && column.water_surface_height.unwrap_or(column.terrain_height).is_finite()
                && column.saturation.is_finite()
                && column.saturation >= 0.0
                && column.saturation <= 1.0
        }));
    }

    #[test]
    fn hydrology_outputs_stay_finite_for_the_previous_preview_artifact_window() {
        let meta = WorldMeta::new(42);
        let chunks = [
            ChunkCoord(41, 0, -31),
            ChunkCoord(42, 0, -31),
            ChunkCoord(41, 0, -30),
            ChunkCoord(42, 0, -30),
            ChunkCoord(41, 0, -29),
            ChunkCoord(42, 0, -29),
            ChunkCoord(41, 0, -28),
        ];

        for chunk in chunks {
            let (_, _, _, hydrology) = build_hydrology(chunk, &meta);
            assert!(
                hydrology.columns.iter().all(|column| {
                    column.terrain_height.is_finite()
                        && column.channel_floor_height.unwrap_or(column.terrain_height).is_finite()
                        && column.water_surface_height.unwrap_or(column.terrain_height).is_finite()
                        && column.saturation.is_finite()
                        && column.saturation >= 0.0
                        && column.saturation <= 1.0
                }),
                "hydrology outputs must stay finite in regression chunk {:?}",
                chunk,
            );
        }
    }

    #[test]
    fn hydrology_carves_and_fills_a_visible_waterway() {
        let meta = WorldMeta::new(42);
        let (_, _, _, smoothed, hydrology) = find_chunk_with_visible_water(&meta);
        let water_columns = hydrology
            .columns
            .iter()
            .filter(|column| column.water_surface_height.is_some())
            .count();
        let channel_columns = hydrology
            .columns
            .iter()
            .filter(|column| column.mode == HydrologyMode::Channel)
            .count();
        let carved_columns = hydrology
            .columns
            .iter()
            .zip(smoothed.columns.iter())
            .filter(|(column, smoothed)| column.terrain_height < smoothed.height - 0.10)
            .count();

        assert!(hydrology.connected_waterlines > 0);
        assert!(water_columns >= 8);
        assert!(channel_columns > 0);
        assert!(carved_columns >= water_columns.min(12));
    }

    #[test]
    fn river_channels_stay_narrower_than_the_entire_chunk() {
        let meta = WorldMeta::new(42);
        let (_, _, _, _, hydrology) = find_chunk_with_channel_band(&meta);
        let water_columns = hydrology
            .columns
            .iter()
            .filter(|column| column.water_surface_height.is_some())
            .count();
        let channel_columns = hydrology
            .columns
            .iter()
            .filter(|column| column.mode == HydrologyMode::Channel)
            .count();
        let floodplain_columns = hydrology
            .columns
            .iter()
            .filter(|column| column.mode == HydrologyMode::Floodplain)
            .count();

        assert!(water_columns >= 64);
        assert!(water_columns < 640);
        assert!(channel_columns >= water_columns / 2);
        assert!(floodplain_columns > 0);
    }

    #[test]
    fn channel_carve_depth_varies_along_visible_waterway() {
        let meta = WorldMeta::new(42);
        let (_, _, _, smoothed, hydrology) = find_chunk_with_visible_water(&meta);
        let mut min_cut = f32::MAX;
        let mut max_cut = 0.0_f32;
        let mut samples = 0_usize;

        for (after, before) in hydrology.columns.iter().zip(smoothed.columns.iter()) {
            if after.mode != HydrologyMode::Channel {
                continue;
            }
            let cut = (before.height - after.terrain_height).max(0.0);
            min_cut = min_cut.min(cut);
            max_cut = max_cut.max(cut);
            samples += 1;
        }

        assert!(samples >= 16);
        assert!(max_cut - min_cut >= 0.75);
    }

    #[test]
    fn water_columns_keep_channel_floor_below_their_surface() {
        let meta = WorldMeta::new(42);
        let (_, _, _, _, hydrology) = find_chunk_with_visible_water(&meta);

        for column in hydrology.columns.iter().filter(|column| column.water_surface_height.is_some()) {
            let water = column
                .water_surface_height
                .expect("filtered water columns should carry a water surface");
            let floor = column
                .channel_floor_height
                .expect("water columns should carry a channel or basin floor");

            assert!(floor < water);
            assert!(column.terrain_height <= water - MIN_VISIBLE_WATER_DEPTH);
        }
    }

    #[test]
    fn neighboring_chunks_keep_shared_edge_water_surfaces_close() {
        let meta = WorldMeta::new(42);
        let ((_, left), (_, right), axis) = find_neighboring_chunk_pair_with_shared_water(&meta);
        assert!(seam_has_shared_visible_water(&left, &right, axis));
    }

    #[test]
    fn meander_profile_swings_and_returns_to_zero_at_segment_ends() {
        let corridor = RiverCorridorConstraint {
            river_id: 0xA5A5_1357,
            kind: RiverPathKind::Trunk,
            order: 2,
            start_x: 0.0,
            start_z: 0.0,
            end_x: 256.0,
            end_z: 0.0,
            center_x: 128.0,
            center_z: 0.0,
            half_width_blocks: 48.0,
            downstream_grade_per_block: 0.0018,
        };
        let branch_seed = branch_noise_seed(corridor);

        let start = centerline_meander_offset_blocks(
            corridor, 0.0, 256.0, 0.0, 0.0, 9.0, 18.0, 0.75, 0.70, 0.65, 0.08, 0.10, 0.80, 0.70,
            0.30, 0.25, branch_seed,
        );
        let quarter = centerline_meander_offset_blocks(
            corridor, 0.25, 256.0, 64.0, 0.0, 9.0, 18.0, 0.75, 0.70, 0.65, 0.08, 0.10, 0.80,
            0.70, 0.30, 0.25, branch_seed,
        );
        let three_quarter = centerline_meander_offset_blocks(
            corridor, 0.75, 256.0, 192.0, 0.0, 9.0, 18.0, 0.75, 0.70, 0.65, 0.08, 0.10, 0.80,
            0.70, 0.30, 0.25, branch_seed,
        );
        let end = centerline_meander_offset_blocks(
            corridor, 1.0, 256.0, 256.0, 0.0, 9.0, 18.0, 0.75, 0.70, 0.65, 0.08, 0.10, 0.80,
            0.70, 0.30, 0.25, branch_seed,
        );

        assert!(start.abs() <= 0.001);
        assert!(end.abs() <= 0.001);
        assert!(quarter.abs() >= 0.75);
        assert!(three_quarter.abs() >= 0.75);
        assert!(quarter * three_quarter < 0.0);
    }

    #[test]
    fn channel_water_surfaces_stay_within_local_bank_support() {
        let meta = WorldMeta::new(42);
        let (_, _, smoothed, hydrology) = build_hydrology(ChunkCoord(40, 0, -29), &meta);

        for (after, before) in hydrology.columns.iter().zip(smoothed.columns.iter()) {
            if after.mode != HydrologyMode::Channel {
                continue;
            }
            let Some(water) = after.water_surface_height else {
                continue;
            };

            assert!(
                water <= before.height + 0.05,
                "channel water rose above local smoothed support: before={:.3} water={:.3}",
                before.height,
                water
            );
            assert!(
                after.terrain_height <= before.height - 0.05,
                "channel water should sit over a carved trough: before={:.3} terrain={:.3}",
                before.height,
                after.terrain_height
            );
        }
    }
}
