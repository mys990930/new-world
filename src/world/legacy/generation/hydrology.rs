use std::collections::BTreeSet;
use std::f32::consts::{PI, TAU};

use crate::world::atlas::{
    BiomeFamily, CoastalContext, HydrologyContext, ReliefClass, RiverPathKind, TerrainFormFamily,
};
use crate::world::coord::{CHUNK_EDGE_I32, ChunkCoord};

use super::super::{SEA_LEVEL_Y, WORLD_FLOOR_Y};
use super::corridors::sample_corridor_axis;
use super::{
    ChunkCorridorWindow, ChunkGenerationInputs, RegionSampleWeight, RiverCorridorConstraint,
    SmoothedPrototype, sample_atlas_fields_fractional, sample_region_weights,
};

mod bars;
mod channel_carve;
mod context;
mod floodplain_bench;
mod masks;
mod water_profile;
mod water_surface;

use context::{BranchKey, CorridorHydrologyResponse, RegionHydrologySignals};

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
const HYDRO_SALT_CARVE_BREAKUP: u64 = 0xA511_3001_0000_0010;
const HYDRO_SALT_BANK_SHELF: u64 = 0xA511_3001_0000_0011;
const HYDRO_SALT_GRAVEL_BAR: u64 = 0xA511_3001_0000_0012;
const MAX_SHORELINE_FREEBOARD_BLOCKS: f32 = 2.45;
const SHORELINE_PILLAR_SUPPRESSION_RADIUS: i32 = 18;
const SHORELINE_PILLAR_LOCAL_PEAK_EPSILON: f32 = 0.05;
const SHORELINE_PILLAR_MIN_RELIEF_BLOCKS: f32 = 1.0;
const SHORELINE_PILLAR_MIN_ABOVE_WATER_BLOCKS: f32 = 1.25;
const SHORELINE_EDGE_CONTEXT_BAND: i32 = 3;
const SHORELINE_EDGE_PILLAR_MIN_SATURATION: f32 = 0.25;
const SHORELINE_EDGE_LOWLAND_MAX_HEIGHT: f32 = SEA_LEVEL_Y as f32 + 18.0;
const WEAK_SHORELINE_RELAX_RADIUS: i32 = 3;
const WEAK_SHORELINE_BASE_FREEBOARD: f32 = 0.86;
const WEAK_SHORELINE_DISTANCE_RISE: f32 = 0.58;
const WEAK_SHORELINE_MIN_RELIEF_BLOCKS: f32 = 0.70;
const WEAK_SHORELINE_MIN_EXCESS_BLOCKS: f32 = 0.08;
const WATER_BORDER_MAX_CONSTRAINT_PASSES: usize = 32;
const WATER_BORDER_SUPPORT_CARVE_EPSILON: f32 = 0.04;

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
    pub gravel_bar_strength: f32,
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
    inputs: &ChunkGenerationInputs,
    corridor_window: &ChunkCorridorWindow,
    smoothed: &SmoothedPrototype,
) -> HydrologySolve {
    debug_assert_eq!(inputs.chunk, chunk);
    debug_assert_eq!(corridor_window.chunk, chunk);
    debug_assert_eq!(smoothed.chunk, chunk);

    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let mut columns = Vec::with_capacity(smoothed.columns.len());
    let mut column_branch_keys = Vec::with_capacity(smoothed.columns.len());

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let index = column_index(local_x, local_z);
            let smoothed_column = smoothed.columns[index];
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let sample_world_x = world_x as f32 + 0.5;
            let sample_world_z = world_z as f32 + 0.5;
            let field = sample_atlas_fields_fractional(
                &inputs.atlas_fields,
                sample_world_x,
                sample_world_z,
            );
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
                        &corridor_window.corridors,
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
                            gravel_bar_strength: response.gravel_bar_strength,
                            mode: response.mode,
                        },
                        smoothed_column,
                    );
                    column_branch_keys.push(Some(response.branch_key));
                    column
                }
                None => {
                    column_branch_keys.push(None);
                    sanitize_hydrology_column(
                        basin_fallback_column(field, region_signals, smoothed_column),
                        smoothed_column,
                    )
                }
            };

            columns.push(column);
        }
    }

    suppress_isolated_shoreline_pillars(&mut columns);
    relax_weak_shoreline_columns(&mut columns, &column_branch_keys);
    constrain_water_components_to_border_support(&mut columns);

    let connected_waterlines = columns
        .iter()
        .zip(column_branch_keys.iter())
        .filter_map(|(column, branch_key)| {
            column
                .water_surface_height
                .is_some()
                .then_some(*branch_key)
                .flatten()
        })
        .collect::<BTreeSet<_>>()
        .len();

    HydrologySolve {
        chunk,
        columns,
        connected_waterlines,
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
        column
            .terrain_height
            .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y)
    } else {
        fallback_terrain
    };
    let saturation = sanitize_unit_interval(column.saturation, 0.0);
    let gravel_bar_strength = sanitize_unit_interval(column.gravel_bar_strength, 0.0);
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
        channel_floor_height = Some(
            floor
                .min(terrain_height)
                .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y),
        );
    }

    HydrologyColumn {
        terrain_height,
        water_surface_height,
        channel_floor_height,
        saturation,
        gravel_bar_strength,
        mode,
    }
}

#[derive(Debug, Clone, Copy)]
struct ReachCarveStyle {
    shallow_source_signal: f32,
    lower_broad_signal: f32,
    meander_signal: f32,
    straight_signal: f32,
    depth_scale: f32,
    min_water_depth: f32,
    max_water_depth: f32,
    channel_width_scale: f32,
    water_width_scale: f32,
    bank_width_scale: f32,
    floodplain_width_scale: f32,
    outer_width_scale: f32,
    core_profile_strength: f32,
    floodplain_profile_strength: f32,
    floodplain_lowering_scale: f32,
    floodplain_freeboard: f32,
    bar_strength_scale: f32,
}

fn corridor_hydrology_response(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
    all_corridors: &[RiverCorridorConstraint],
    field: crate::world::atlas::AtlasCell,
    region_signals: RegionHydrologySignals,
    local_x: f32,
    local_z: f32,
    smoothed: super::smoothing::SmoothedColumn,
    corridor: RiverCorridorConstraint,
) -> Option<CorridorHydrologyResponse> {
    let projected = sample_corridor_axis(corridor, local_x, local_z);
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
    let start_field =
        sample_atlas_fields_fractional(&inputs.atlas_fields, start_world_x, start_world_z);
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
    let raw_centerline_world_x = lerp_f32(start_world_x, end_world_x, projected.segment_t);
    let raw_centerline_world_z = lerp_f32(start_world_z, end_world_z, projected.segment_t);
    let centerline_world_x =
        raw_centerline_world_x + projected.normal_x * projected.axis_offset_blocks;
    let centerline_world_z =
        raw_centerline_world_z + projected.normal_z * projected.axis_offset_blocks;
    let start_anchor = corridor_anchor_world_y(inputs, start_world_x, start_world_z, corridor);
    let end_anchor = corridor_anchor_world_y(inputs, end_world_x, end_world_z, corridor);
    let base_channel_radius_start = final_channel_radius(corridor, start_field, start_region);
    let base_channel_radius_end = final_channel_radius(corridor, end_field, end_region);
    let base_channel_radius = lerp_f32(base_channel_radius_start, base_channel_radius_end, along_t);
    let water_radius_start = final_water_radius(
        base_channel_radius_start,
        corridor,
        start_field,
        start_region,
    );
    let water_radius_end =
        final_water_radius(base_channel_radius_end, corridor, end_field, end_region);
    let base_water_radius = lerp_f32(water_radius_start, water_radius_end, along_t);
    let floodplain_radius_start =
        final_floodplain_radius(base_channel_radius_start, start_field, start_region);
    let floodplain_radius_end =
        final_floodplain_radius(base_channel_radius_end, end_field, end_region);
    let base_floodplain_radius = lerp_f32(floodplain_radius_start, floodplain_radius_end, along_t);
    let segment_floodplain_bias = lerp_f32(
        start_region.floodplain_bias,
        end_region.floodplain_bias,
        along_t,
    );
    let segment_dryland_bias =
        lerp_f32(start_region.dryland_bias, end_region.dryland_bias, along_t);
    let segment_outlet_bias = lerp_f32(start_region.outlet_bias, end_region.outlet_bias, along_t);
    let segment_wetness = lerp_f32(start_field.wetness, end_field.wetness, along_t);
    let segment_riverine = lerp_f32(
        start_field.riverine_factor,
        end_field.riverine_factor,
        along_t,
    );
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
    let carve_breakup_noise = seedless_value_fbm(
        edge_warp_x - 47.0,
        edge_warp_z + 83.0,
        lerp_f32(8.0, 54.0, lower_reach_signal),
        4,
        2.0,
        0.56,
        HYDRO_SALT_CARVE_BREAKUP ^ branch_seed,
    );
    let bank_shelf_noise = seedless_value_noise_signed(
        edge_warp_x + 91.0,
        edge_warp_z - 57.0,
        lerp_f32(10.0, 76.0, lower_reach_signal),
        HYDRO_SALT_BANK_SHELF ^ branch_seed,
    );
    let gravel_bar_noise = seedless_value_fbm(
        center_warp_x - 131.0,
        center_warp_z + 109.0,
        lerp_f32(16.0, 128.0, lower_reach_signal),
        4,
        2.0,
        0.54,
        HYDRO_SALT_GRAVEL_BAR ^ branch_seed,
    );
    let meander_offset = centerline_meander_offset_blocks(
        corridor,
        projected.downstream_blocks,
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
    let curvature_signal = centerline_curvature_signal(
        corridor,
        projected.downstream_blocks,
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
    let reach_style = resolve_reach_carve_style(
        corridor,
        lower_reach_signal,
        curvature_signal,
        segment_wetness,
        segment_riverine,
        segment_slope,
        field.river_flow_potential,
        region_signals,
        slope_signal,
        depth_noise,
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
    let side_asymmetry =
        (asymmetry_noise * (0.12 + region_signals.lateral_variability * 0.20)).clamp(-0.36, 0.36);
    let side_scale = if signed_distance_blocks >= 0.0 {
        1.0 + side_asymmetry
    } else {
        1.0 - side_asymmetry
    }
    .clamp(0.72, 1.30);
    let channel_radius = (base_channel_radius
        * (1.0 + lateral_width_mod)
        * side_scale
        * reach_style.channel_width_scale)
        .clamp(
            match corridor.kind {
                RiverPathKind::Trunk => 4.0,
                RiverPathKind::Tributary => 2.2,
            },
            corridor.half_width_blocks * 0.46
                + match corridor.kind {
                    RiverPathKind::Trunk => 14.0,
                    RiverPathKind::Tributary => 9.0,
                },
        );
    let water_width_mod = (width_noise * (0.18 + region_signals.width_variability * 0.18)
        + edge_noise * 0.12
        - confinement * 0.05)
        .clamp(-0.36, 0.34);
    let water_radius = (base_water_radius
        * (0.82 + water_width_mod)
        * side_scale.clamp(0.78, 1.12)
        * reach_style.water_width_scale)
        .clamp(1.1, channel_radius * 0.86);
    let bank_span = ((2.2
        + channel_radius
            * (0.20 + region_signals.transition_softness * 0.12 + lower_reach_signal * 0.10)
        + region_signals.outer_spread_bias * 1.4
        - confinement * 0.85)
        * reach_style.bank_width_scale
        * (1.0 + transition_noise * (0.08 + region_signals.lateral_variability * 0.10)))
        .clamp(1.6, channel_radius * 1.3 + 8.0);
    let bank_radius_min = channel_radius + 1.2;
    let bank_radius_max = (corridor.half_width_blocks * 0.56
        + lerp_f32(8.0, 18.0, lower_reach_signal))
    .max(bank_radius_min);
    let bank_radius = (channel_radius + bank_span).clamp(bank_radius_min, bank_radius_max);
    let floodplain_span = ((3.2
        + region_signals.floodplain_bias * 4.8
        + region_signals.outer_spread_bias * 4.6
        + lower_reach_signal * 10.0
        + segment_wetness * 2.2
        - region_signals.dryland_bias * 1.8)
        * reach_style.floodplain_width_scale
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
    let outer_span = (((4.0
        + region_signals.transition_softness * 7.2
        + region_signals.outer_spread_bias * 11.0
        + lower_reach_signal * 22.0
        - region_signals.confinement_bias * 3.5)
        + outer_noise.abs() * lerp_f32(4.0, 44.0, lower_reach_signal))
        * reach_style.outer_width_scale)
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
    let distance_blocks = (signed_distance_blocks * signed_distance_blocks
        + projected.longitudinal_error_blocks.powi(2))
    .sqrt();
    let core_influence =
        radial_influence(distance_blocks, channel_radius, 0.88 + confinement * 0.60);
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

    let start_channel_depth =
        final_channel_depth(corridor, start_field, start_region, positive_concavity);
    let end_channel_depth =
        final_channel_depth(corridor, end_field, end_region, positive_concavity);
    let start_floodplain_lowering =
        final_floodplain_lowering(start_field, start_region, positive_concavity, slope_signal);
    let end_floodplain_lowering =
        final_floodplain_lowering(end_field, end_region, positive_concavity, slope_signal);
    let base_channel_depth = lerp_f32(start_channel_depth, end_channel_depth, along_t);
    let raw_channel_depth = base_channel_depth
        * (1.0 + depth_noise * (0.12 + region_signals.depth_variability * 0.28))
        * (1.0 + region_signals.incision_bias * 0.10 + region_signals.confinement_bias * 0.08)
        + region_signals.incision_bias * 0.90
        - region_signals.transition_softness * 0.24;
    let local_channel_depth = (raw_channel_depth * reach_style.depth_scale
        + reach_style.lower_broad_signal * 0.85
        + reach_style.meander_signal * 0.55
        - reach_style.shallow_source_signal * 0.75
        - reach_style.straight_signal * 0.34)
        .clamp(reach_style.min_water_depth, reach_style.max_water_depth);
    let water_profile = water_profile::resolve_water_profile(water_profile::WaterProfileInput {
        smoothed_height: smoothed.height,
        remaining_relief_budget: smoothed.remaining_relief_budget,
        start_anchor,
        end_anchor,
        along_t,
        incision_bias: region_signals.incision_bias,
        local_channel_depth,
    });
    let profile_water_surface = water_profile.water_surface;
    let core_floor_target = water_profile.core_floor_target;
    let core_total_cut = water_profile.core_total_cut;
    let desired_water_depth = water_surface::desired_water_depth(
        local_channel_depth,
        field.river_flow_potential,
        corridor.downstream_grade_per_block,
        depth_noise,
        water_sheet_influence,
    );
    let carved_water_surface = water_surface::carve_supported_water_surface(
        profile_water_surface,
        core_floor_target,
        desired_water_depth,
    );
    let floodplain_lowering_base =
        lerp_f32(start_floodplain_lowering, end_floodplain_lowering, along_t)
            * reach_style.floodplain_lowering_scale
            * (1.0 + transition_noise * (0.16 + region_signals.transition_softness * 0.18))
            + outer_noise.max(0.0) * 0.28;
    let carve_layers =
        channel_carve::resolve_channel_carve_layers(channel_carve::ChannelCarveInput {
            core_total_cut,
            floodplain_lowering_base,
            transition_softness: region_signals.transition_softness,
            floodplain_bias: region_signals.floodplain_bias,
            outer_spread_bias: region_signals.outer_spread_bias,
            lower_reach_signal,
            edge_noise,
            carve_breakup_noise,
            depth_variability: region_signals.depth_variability,
            bank_shelf_noise,
            lateral_variability: region_signals.lateral_variability,
            curvature_signal,
            outer_noise,
            gravel_bar_noise,
            transition_noise,
            depth_noise,
            confinement,
        });
    let bench =
        floodplain_bench::resolve_floodplain_bench(floodplain_bench::FloodplainBenchInput {
            water_radius,
            bank_radius,
            floodplain_radius,
            outer_radius,
            distance_blocks,
            lower_reach_signal,
            transition_softness: region_signals.transition_softness,
            outer_spread_bias: region_signals.outer_spread_bias,
            core_influence,
            bank_influence,
            floodplain_influence,
            outer_influence,
            signed_distance_blocks,
            curvature_signal,
            bank_shelf_noise,
            gravel_bar_noise,
            outer_delta: carve_layers.outer_delta,
            flood_delta: carve_layers.flood_delta,
            bank_delta: carve_layers.bank_delta,
        });
    let terrain_height_base = channel_carve::apply_channel_carve_layers(
        smoothed.height,
        carve_layers,
        outer_influence,
        floodplain_influence,
        bank_influence,
        core_influence,
        bench.bank_shelf_lift,
        bench.flood_bench_lift,
        core_floor_target,
        carved_water_surface,
        reach_style.core_profile_strength,
        reach_style.floodplain_profile_strength,
        reach_style.floodplain_freeboard,
    );
    let confluence_signal = local_confluence_signal(
        all_corridors,
        corridor,
        local_x,
        local_z,
        water_radius,
        bank_radius,
    );
    let depositional_water_surface = carved_water_surface;
    let gravel_bar = bars::apply_gravel_bar(bars::GravelBarInput {
        terrain_height_base,
        smoothed_height: smoothed.height,
        depositional_water_surface,
        core_floor_target,
        water_radius,
        bank_radius,
        base_channel_radius,
        channel_radius,
        distance_blocks,
        signed_distance_blocks,
        lower_reach_signal,
        transition_softness: region_signals.transition_softness,
        water_sheet_influence,
        core_influence,
        bank_influence,
        floodplain_influence,
        inside_bend_alignment: bench.inside_bend_alignment,
        curvature_signal,
        confluence_signal,
        slope_signal,
        downstream_grade_per_block: corridor.downstream_grade_per_block,
        gravel_bar_noise,
        regime_bar_strength: reach_style.bar_strength_scale,
    });
    let terrain_height = apply_shoreline_support_relaxation(ShorelineSupportInput {
        terrain_height: gravel_bar.terrain_height,
        depositional_water_surface,
        water_radius,
        bank_radius,
        floodplain_radius,
        distance_blocks,
        lower_reach_signal,
        transition_softness: region_signals.transition_softness,
        outer_spread_bias: region_signals.outer_spread_bias,
        water_sheet_influence,
        core_influence,
        bank_influence,
        floodplain_influence,
        confinement,
        slope_signal,
        edge_noise,
    });
    let gravel_bar_strength = gravel_bar.strength;
    let floor_hold_influence = radial_influence(
        distance_blocks,
        bank_radius.max(water_radius + 0.6),
        0.86 + confinement * 0.18,
    );
    let channel_floor_y = lerp_f32(terrain_height, core_floor_target, floor_hold_influence)
        .min(terrain_height)
        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);

    let basin_signal = masks::basin_signal(field, region_signals, positive_concavity, slope_signal);
    let saturation = masks::saturation(
        field,
        region_signals,
        wet_margin_influence,
        outer_influence,
        floodplain_influence,
        bank_influence,
        positive_concavity,
        basin_signal,
    );
    let lake_bias = basin_signal * floodplain_influence.max(outer_influence * 0.65);
    let water_presence = masks::water_presence(
        region_signals,
        water_sheet_influence,
        core_influence,
        bank_influence,
        lake_bias,
        positive_concavity,
    );
    let channel_has_visible_water = masks::channel_has_visible_water(
        water_sheet_influence,
        distance_blocks,
        water_radius,
        bank_influence,
    );
    let standing_water =
        water_surface::resolve_visible_water_surface(water_surface::VisibleWaterInput {
            water_presence,
            channel_has_visible_water,
            lake_bias,
            anchored_water_surface: carved_water_surface,
            terrain_height,
            channel_floor_y,
        });

    let mode = masks::classify_hydrology_mode(
        standing_water,
        lake_bias,
        core_influence,
        basin_signal,
        channel_has_visible_water,
        saturation,
        floodplain_influence,
        outer_influence,
    );

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
        gravel_bar_strength,
        mode,
        influence,
    })
}

#[derive(Debug, Clone, Copy)]
struct ShorelineSupportInput {
    terrain_height: f32,
    depositional_water_surface: f32,
    water_radius: f32,
    bank_radius: f32,
    floodplain_radius: f32,
    distance_blocks: f32,
    lower_reach_signal: f32,
    transition_softness: f32,
    outer_spread_bias: f32,
    water_sheet_influence: f32,
    core_influence: f32,
    bank_influence: f32,
    floodplain_influence: f32,
    confinement: f32,
    slope_signal: f32,
    edge_noise: f32,
}

fn apply_shoreline_support_relaxation(input: ShorelineSupportInput) -> f32 {
    let water_edge_distance = input.distance_blocks - input.water_radius;
    let outside_water_mask = smootherstep01((water_edge_distance + 0.18) / 0.92)
        * (1.0 - input.water_sheet_influence * 0.86)
        * (1.0 - input.core_influence * 0.94);
    if outside_water_mask <= 0.0 {
        return input.terrain_height;
    }

    let bank_span = (input.bank_radius - input.water_radius).max(1.0);
    let floodplain_span = (input.floodplain_radius - input.water_radius).max(bank_span + 1.0);
    let bank_zone = 1.0 - smootherstep01((water_edge_distance - bank_span).max(0.0) / 3.2);
    let floodplain_zone =
        1.0 - smootherstep01((water_edge_distance - floodplain_span).max(0.0) / 8.0);
    let wet_lowland_support = (input.lower_reach_signal * 0.30
        + input.transition_softness * 0.24
        + input.outer_spread_bias * 0.14
        + (1.0 - input.slope_signal).clamp(0.0, 1.0) * 0.16
        - input.confinement * 0.16)
        .clamp(0.0, 1.0);
    let relaxation = (outside_water_mask
        * input.bank_influence.max(input.floodplain_influence * 0.64)
        * (bank_zone * 0.78 + floodplain_zone * 0.22)
        * (0.42 + wet_lowland_support * 0.92).clamp(0.0, 1.0)
        * (0.92 + input.edge_noise.max(0.0) * 0.10))
        .clamp(0.0, 0.94);
    if relaxation <= 0.0 {
        return input.terrain_height;
    }

    let distance_ratio = (water_edge_distance.max(0.0) / floodplain_span).clamp(0.0, 1.0);
    let near_bank_freeboard = (0.70
        + input.transition_softness * 0.22
        + input.lower_reach_signal * 0.18
        + input.slope_signal * 0.22)
        .clamp(0.48, MAX_SHORELINE_FREEBOARD_BLOCKS);
    let outer_rise = (distance_ratio * distance_ratio)
        * (1.15 + input.confinement * 1.20 + input.slope_signal * 0.90);
    let shoreline_target = (input.depositional_water_surface
        + near_bank_freeboard
        + outer_rise.min(MAX_SHORELINE_FREEBOARD_BLOCKS))
    .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);

    if input.terrain_height <= shoreline_target {
        input.terrain_height
    } else {
        lerp_f32(input.terrain_height, shoreline_target, relaxation)
            .min(input.terrain_height)
            .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y)
    }
}

fn suppress_isolated_shoreline_pillars(columns: &mut [HydrologyColumn]) {
    if columns.len() != (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize {
        return;
    }

    let original = columns.to_vec();
    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let index = column_index(local_x, local_z);
            let column = original[index];
            if column.water_surface_height.is_some() {
                continue;
            }

            let nearest_water = nearest_visible_water_height_within_columns(
                &original,
                local_x,
                local_z,
                SHORELINE_PILLAR_SUPPRESSION_RADIUS,
            );
            let edge_shoreline_context = nearest_water.is_none()
                && column.saturation >= SHORELINE_EDGE_PILLAR_MIN_SATURATION
                && column.terrain_height <= SHORELINE_EDGE_LOWLAND_MAX_HEIGHT
                && is_near_chunk_edge(local_x, local_z, SHORELINE_EDGE_CONTEXT_BAND);
            if nearest_water.is_none() && !edge_shoreline_context {
                continue;
            }

            let Some((neighbor_min, neighbor_max)) =
                direct_neighbor_terrain_range(&original, local_x, local_z)
            else {
                continue;
            };

            let local_peak = column.terrain_height - neighbor_max;
            let local_relief = column.terrain_height - neighbor_min;
            let above_water = nearest_water
                .map(|(water_height, _)| column.terrain_height - water_height)
                .unwrap_or(SHORELINE_PILLAR_MIN_ABOVE_WATER_BLOCKS);
            if local_peak <= SHORELINE_PILLAR_LOCAL_PEAK_EPSILON
                || local_relief < SHORELINE_PILLAR_MIN_RELIEF_BLOCKS
                || above_water < SHORELINE_PILLAR_MIN_ABOVE_WATER_BLOCKS
            {
                continue;
            }

            let supported_height = match nearest_water {
                Some((water_height, water_distance)) => neighbor_max
                    .min(
                        water_height
                            + MAX_SHORELINE_FREEBOARD_BLOCKS
                            + water_distance as f32 * 0.03,
                    )
                    .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y),
                None => neighbor_max.clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y),
            };
            let target = supported_height.min(column.terrain_height);
            if target < columns[index].terrain_height {
                columns[index].terrain_height = target;
                columns[index].channel_floor_height = columns[index]
                    .channel_floor_height
                    .map(|floor| floor.min(target));
                columns[index].saturation = columns[index].saturation.max(0.34);
                if columns[index].mode == HydrologyMode::Dry {
                    columns[index].mode = HydrologyMode::Wetland;
                }
            }
        }
    }
}

fn relax_weak_shoreline_columns(
    columns: &mut [HydrologyColumn],
    column_branch_keys: &[Option<BranchKey>],
) {
    if columns.len() != (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize
        || column_branch_keys.len() != columns.len()
    {
        return;
    }

    let original = columns.to_vec();
    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let index = column_index(local_x, local_z);
            let column = original[index];
            if column.water_surface_height.is_some() {
                continue;
            }

            let weak_shoreline_column = column_branch_keys[index].is_none()
                || column.channel_floor_height.is_none()
                || matches!(column.mode, HydrologyMode::Dry | HydrologyMode::Wetland);
            if !weak_shoreline_column {
                continue;
            }

            let Some((water_height, water_distance)) = nearest_visible_water_height_within_columns(
                &original,
                local_x,
                local_z,
                WEAK_SHORELINE_RELAX_RADIUS,
            ) else {
                continue;
            };
            let Some((neighbor_min, _)) =
                direct_neighbor_terrain_range(&original, local_x, local_z)
            else {
                continue;
            };

            let local_relief = column.terrain_height - neighbor_min;
            if local_relief < WEAK_SHORELINE_MIN_RELIEF_BLOCKS {
                continue;
            }

            let distance_freeboard = WEAK_SHORELINE_BASE_FREEBOARD
                + (water_distance.saturating_sub(1) as f32) * WEAK_SHORELINE_DISTANCE_RISE;
            let shoreline_target = (water_height + distance_freeboard)
                .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y)
                .min(column.terrain_height);
            if column.terrain_height - shoreline_target < WEAK_SHORELINE_MIN_EXCESS_BLOCKS {
                continue;
            }

            columns[index].terrain_height = shoreline_target;
            columns[index].channel_floor_height = columns[index]
                .channel_floor_height
                .map(|floor| floor.min(shoreline_target));
            columns[index].saturation = columns[index].saturation.max(0.40);
            if matches!(
                columns[index].mode,
                HydrologyMode::Dry | HydrologyMode::Wetland
            ) {
                columns[index].mode = HydrologyMode::Floodplain;
            }
        }
    }
}

fn constrain_water_components_to_border_support(columns: &mut [HydrologyColumn]) {
    if columns.len() != (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize {
        return;
    }

    for _ in 0..WATER_BORDER_MAX_CONSTRAINT_PASSES {
        let original = columns.to_vec();
        let mut visited = vec![false; columns.len()];
        let mut changed = false;

        for local_z in 0..CHUNK_EDGE_I32 {
            for local_x in 0..CHUNK_EDGE_I32 {
                let index = column_index(local_x, local_z);
                if visited[index] || original[index].water_surface_height.is_none() {
                    continue;
                }

                let (component, supported_surface) =
                    collect_water_component_support(&original, &mut visited, local_x, local_z);

                for component_index in component {
                    let column = columns[component_index];
                    let supported_terrain = (supported_surface
                        - MIN_VISIBLE_WATER_DEPTH
                        - WATER_BORDER_SUPPORT_CARVE_EPSILON)
                        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
                    if columns[component_index].terrain_height > supported_terrain {
                        columns[component_index].terrain_height = supported_terrain;
                        changed = true;
                    }

                    let terrain_height = columns[component_index].terrain_height;
                    let floor = column
                        .channel_floor_height
                        .unwrap_or(terrain_height)
                        .min(terrain_height)
                        .clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y);
                    if supported_surface - terrain_height >= MIN_VISIBLE_WATER_DEPTH
                        && supported_surface - floor >= MIN_VISIBLE_WATER_DEPTH
                    {
                        if column.water_surface_height != Some(supported_surface) {
                            columns[component_index].water_surface_height = Some(supported_surface);
                            changed = true;
                        }
                        columns[component_index].channel_floor_height = Some(floor);
                    } else {
                        if columns[component_index].water_surface_height.is_some() {
                            columns[component_index].water_surface_height = None;
                            changed = true;
                        }
                        columns[component_index].channel_floor_height = columns[component_index]
                            .channel_floor_height
                            .map(|old_floor| old_floor.min(floor));
                        columns[component_index].saturation =
                            columns[component_index].saturation.max(0.46);
                        if matches!(
                            columns[component_index].mode,
                            HydrologyMode::Channel | HydrologyMode::Lake
                        ) {
                            columns[component_index].mode = HydrologyMode::Floodplain;
                        }
                    }
                }
            }
        }

        if !changed {
            break;
        }
    }
}

fn collect_water_component_support(
    columns: &[HydrologyColumn],
    visited: &mut [bool],
    start_x: i32,
    start_z: i32,
) -> (Vec<usize>, f32) {
    let mut component = Vec::new();
    let mut stack = vec![(start_x, start_z)];
    let mut supported_surface = MAX_HYDROLOGY_Y;

    while let Some((local_x, local_z)) = stack.pop() {
        let index = column_index(local_x, local_z);
        if visited[index] {
            continue;
        }
        visited[index] = true;

        let column = columns[index];
        let Some(water_height) = column.water_surface_height else {
            continue;
        };
        supported_surface = supported_surface.min(water_height);
        component.push(index);

        for (offset_x, offset_z) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let neighbor_x = local_x + offset_x;
            let neighbor_z = local_z + offset_z;
            if !(0..CHUNK_EDGE_I32).contains(&neighbor_x)
                || !(0..CHUNK_EDGE_I32).contains(&neighbor_z)
            {
                continue;
            }

            let neighbor_index = column_index(neighbor_x, neighbor_z);
            let neighbor = columns[neighbor_index];
            if neighbor.water_surface_height.is_some() {
                if !visited[neighbor_index] {
                    stack.push((neighbor_x, neighbor_z));
                }
            } else {
                supported_surface = supported_surface.min(neighbor.terrain_height);
            }
        }
    }

    (
        component,
        supported_surface.clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y),
    )
}

fn is_near_chunk_edge(local_x: i32, local_z: i32, radius: i32) -> bool {
    local_x < radius
        || local_z < radius
        || local_x >= CHUNK_EDGE_I32 - radius
        || local_z >= CHUNK_EDGE_I32 - radius
}

fn direct_neighbor_terrain_range(
    columns: &[HydrologyColumn],
    local_x: i32,
    local_z: i32,
) -> Option<(f32, f32)> {
    let mut min_height = f32::MAX;
    let mut max_height = f32::MIN;
    let mut samples = 0_usize;

    for offset_z in -1..=1 {
        for offset_x in -1..=1 {
            if offset_x == 0 && offset_z == 0 {
                continue;
            }

            let neighbor_x = local_x + offset_x;
            let neighbor_z = local_z + offset_z;
            if !(0..CHUNK_EDGE_I32).contains(&neighbor_x)
                || !(0..CHUNK_EDGE_I32).contains(&neighbor_z)
            {
                continue;
            }

            let height = columns[column_index(neighbor_x, neighbor_z)].terrain_height;
            min_height = min_height.min(height);
            max_height = max_height.max(height);
            samples += 1;
        }
    }

    (samples >= 3).then_some((min_height, max_height))
}

fn nearest_visible_water_height_within_columns(
    columns: &[HydrologyColumn],
    local_x: i32,
    local_z: i32,
    radius: i32,
) -> Option<(f32, i32)> {
    let mut nearest = None;
    let mut nearest_distance = i32::MAX;

    for offset_z in -radius..=radius {
        for offset_x in -radius..=radius {
            if offset_x == 0 && offset_z == 0 {
                continue;
            }

            let neighbor_x = local_x + offset_x;
            let neighbor_z = local_z + offset_z;
            if !(0..CHUNK_EDGE_I32).contains(&neighbor_x)
                || !(0..CHUNK_EDGE_I32).contains(&neighbor_z)
            {
                continue;
            }

            let distance = offset_x.abs().max(offset_z.abs());
            if distance > nearest_distance {
                continue;
            }

            if let Some(water_height) =
                columns[column_index(neighbor_x, neighbor_z)].water_surface_height
            {
                nearest = Some((water_height, distance));
                nearest_distance = distance;
            }
        }
    }

    nearest
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
            water_surface_height: (water_surface_y - channel_floor_height
                >= MIN_VISIBLE_WATER_DEPTH)
                .then_some(water_surface_y),
            channel_floor_height: Some(channel_floor_height),
            saturation,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Lake,
        };
    }

    HydrologyColumn {
        terrain_height: smoothed.height.clamp(MIN_HYDROLOGY_Y, MAX_HYDROLOGY_Y),
        water_surface_height: None,
        channel_floor_height: None,
        saturation,
        gravel_bar_strength: 0.0,
        mode: if saturation >= 0.58 {
            HydrologyMode::Wetland
        } else {
            HydrologyMode::Dry
        },
    }
}

fn blended_region_hydrology_signals(
    region_samples: &[RegionSampleWeight],
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
    inputs: &ChunkGenerationInputs,
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
    let shoulder_extension =
        (3.2 + region.floodplain_bias * 5.2 + region.lake_bias * 1.4 + field.wetness * 2.5
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
    let width_ratio = (kind_bias + field.river_flow_potential * 0.06 + region.lake_bias * 0.05
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

#[allow(clippy::too_many_arguments)]
fn resolve_reach_carve_style(
    corridor: RiverCorridorConstraint,
    lower_reach_signal: f32,
    curvature_signal: f32,
    segment_wetness: f32,
    segment_riverine: f32,
    segment_slope: f32,
    river_flow_potential: f32,
    region: RegionHydrologySignals,
    slope_signal: f32,
    depth_noise: f32,
) -> ReachCarveStyle {
    let grade_signal = (corridor.downstream_grade_per_block / 0.0046).clamp(0.0, 1.0);
    let flow_signal =
        (river_flow_potential * 0.54 + segment_riverine * 0.26 + segment_wetness * 0.20)
            .clamp(0.0, 1.0);
    let curvature_abs = curvature_signal.abs();
    let meander_signal = smootherstep01((curvature_abs - 0.010) / 0.060)
        * (0.54 + region.meander_bias * 0.26 + region.lateral_variability * 0.14).clamp(0.42, 1.12);
    let straight_signal = (1.0 - smootherstep01((curvature_abs - 0.004) / 0.034))
        * (0.68 + (1.0 - lower_reach_signal) * 0.16 + region.confinement_bias * 0.12)
            .clamp(0.0, 1.0);
    let lower_broad_signal = (lower_reach_signal * 0.58
        + flow_signal * 0.20
        + region.floodplain_bias * 0.16
        + region.outer_spread_bias * 0.12
        - grade_signal * 0.22
        - slope_signal * 0.12
        - region.confinement_bias * 0.10)
        .clamp(0.0, 1.0);
    let source_kind_bias = match corridor.kind {
        RiverPathKind::Trunk => 0.0,
        RiverPathKind::Tributary => 0.14,
    };
    let shallow_source_signal = ((1.0 - lower_reach_signal) * 0.50
        + (1.0 - flow_signal) * 0.18
        + grade_signal * 0.16
        + source_kind_bias
        - region.floodplain_bias * 0.14
        - segment_wetness * 0.08)
        .clamp(0.0, 1.0);
    let trunk_bonus = match corridor.kind {
        RiverPathKind::Trunk => 1.0,
        RiverPathKind::Tributary => 0.0,
    };
    let depth_scale = (0.86
        + lower_broad_signal * 0.42
        + meander_signal * 0.22
        + flow_signal * 0.18
        + depth_noise.max(0.0) * 0.06
        - shallow_source_signal * 0.48
        - straight_signal * 0.18)
        .clamp(0.34, 1.72);
    let min_water_depth =
        (0.72 + trunk_bonus * 0.18 + lower_broad_signal * 1.20 + meander_signal * 0.24
            - shallow_source_signal * 0.22
            - straight_signal * 0.12)
            .clamp(0.55, 3.30);
    let max_water_depth = (2.35
        + trunk_bonus * 1.60
        + lower_broad_signal * 6.20
        + meander_signal * 2.20
        + flow_signal * 1.40
        - shallow_source_signal * 1.10
        - straight_signal * 0.72)
        .clamp(min_water_depth + 0.35, 12.5);

    ReachCarveStyle {
        shallow_source_signal,
        lower_broad_signal,
        meander_signal,
        straight_signal,
        depth_scale,
        min_water_depth,
        max_water_depth,
        channel_width_scale: (0.82 + lower_broad_signal * 0.34 + meander_signal * 0.10
            - shallow_source_signal * 0.22
            - straight_signal * 0.08)
            .clamp(0.58, 1.52),
        water_width_scale: (0.72
            + lower_broad_signal * 0.44
            + flow_signal * 0.10
            + meander_signal * 0.06
            - shallow_source_signal * 0.24
            - straight_signal * 0.06)
            .clamp(0.48, 1.46),
        bank_width_scale: (0.82
            + lower_broad_signal * 0.38
            + region.transition_softness * 0.10
            + meander_signal * 0.08
            - shallow_source_signal * 0.16
            - straight_signal * 0.10)
            .clamp(0.60, 1.55),
        floodplain_width_scale: (0.78
            + lower_broad_signal * 0.72
            + region.outer_spread_bias * 0.16
            + meander_signal * 0.10
            - shallow_source_signal * 0.18
            - straight_signal * 0.12)
            .clamp(0.55, 1.90),
        outer_width_scale: (0.82 + lower_broad_signal * 0.52 + region.transition_softness * 0.10
            - shallow_source_signal * 0.14
            - straight_signal * 0.08)
            .clamp(0.60, 1.72),
        core_profile_strength: (0.68 + lower_broad_signal * 0.22 + meander_signal * 0.16
            - straight_signal * 0.10)
            .clamp(0.48, 0.98),
        floodplain_profile_strength: (0.18
            + lower_broad_signal * 0.56
            + region.outer_spread_bias * 0.14
            + meander_signal * 0.10
            - shallow_source_signal * 0.10
            - straight_signal * 0.08)
            .clamp(0.06, 0.82),
        floodplain_lowering_scale: (0.62 + lower_broad_signal * 0.72 + meander_signal * 0.12
            - shallow_source_signal * 0.16
            - straight_signal * 0.16)
            .clamp(0.42, 1.55),
        floodplain_freeboard: (0.72
            + region.transition_softness * 0.28
            + lower_broad_signal * 0.24
            + segment_slope * 0.16
            - shallow_source_signal * 0.20)
            .clamp(0.38, 1.85),
        bar_strength_scale: (0.42
            + meander_signal * 0.68
            + lower_broad_signal * 0.18
            + region.floodplain_bias * 0.12
            - straight_signal * 0.34
            - shallow_source_signal * 0.12)
            .clamp(0.08, 1.35),
    }
}

fn centerline_meander_offset_blocks(
    corridor: RiverCorridorConstraint,
    downstream_blocks: f32,
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
    let amplitude = ((channel_radius * (0.62 + meander_support * 0.78) + kind_bonus)
        * length_factor)
        .clamp(0.0, meander_limit);
    if amplitude <= f32::EPSILON {
        return 0.0;
    }

    let source_fade = if corridor.downstream_cells_start <= 0.01 {
        smootherstep01((downstream_blocks / 96.0).clamp(0.0, 1.0))
    } else {
        1.0
    };
    let envelope = source_fade
        * (0.74
            + 0.26
                * (PI * along_t)
                    .sin()
                    .max(0.0)
                    .powf(lerp_f32(0.72, 1.08, lower_reach_signal)));
    let branch_phase = ((branch_seed & 0xFFFF) as f32 / 65_535.0) * TAU;
    let primary_wavelength = (lerp_f32(220.0, 520.0, lower_reach_signal)
        * (1.0 - meander_bias * 0.10 + confinement_bias * 0.08))
        .clamp(160.0, 720.0);
    let harmonic_sign = if ((corridor.river_id >> 1) & 1) == 0 {
        1.0
    } else {
        -1.0
    };
    let primary = (downstream_blocks / primary_wavelength * TAU + branch_phase).sin();
    let secondary = (downstream_blocks / (primary_wavelength * 0.58) * TAU + branch_phase * 1.31)
        .sin()
        * harmonic_sign;
    let tertiary =
        (downstream_blocks / (primary_wavelength * 1.87) * TAU - branch_phase * 0.7).sin();
    let noise_wave = seedless_value_fbm(
        centerline_world_x + downstream_blocks * 0.17,
        centerline_world_z - downstream_blocks * 0.11,
        lerp_f32(18.0, 110.0, lower_reach_signal),
        4,
        2.0,
        0.55,
        HYDRO_SALT_CENTERLINE ^ branch_seed,
    );
    let wave = primary * 0.50 + secondary * 0.24 + tertiary * 0.12 + noise_wave * 0.22;

    amplitude * envelope * wave
}

#[allow(clippy::too_many_arguments)]
fn centerline_curvature_signal(
    corridor: RiverCorridorConstraint,
    downstream_blocks: f32,
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
    if segment_length_blocks <= f32::EPSILON {
        return 0.0;
    }

    let step_t = (lerp_f32(14.0, 32.0, lower_reach_signal) / segment_length_blocks.max(1.0))
        .clamp(0.035, 0.22);
    let prev_t = (along_t - step_t).clamp(0.0, 1.0);
    let next_t = (along_t + step_t).clamp(0.0, 1.0);
    if (next_t - prev_t) <= 0.0001 {
        return 0.0;
    }

    let prev_offset = centerline_meander_offset_blocks(
        corridor,
        downstream_blocks - step_t * segment_length_blocks,
        prev_t,
        segment_length_blocks,
        centerline_world_x - step_t * segment_length_blocks,
        centerline_world_z,
        channel_radius,
        floodplain_radius,
        floodplain_bias,
        wetness,
        riverine_factor,
        dryland_bias,
        slope,
        meander_bias,
        lateral_variability,
        confinement_bias,
        lower_reach_signal,
        branch_seed,
    );
    let current_offset = centerline_meander_offset_blocks(
        corridor,
        downstream_blocks,
        along_t,
        segment_length_blocks,
        centerline_world_x,
        centerline_world_z,
        channel_radius,
        floodplain_radius,
        floodplain_bias,
        wetness,
        riverine_factor,
        dryland_bias,
        slope,
        meander_bias,
        lateral_variability,
        confinement_bias,
        lower_reach_signal,
        branch_seed,
    );
    let next_offset = centerline_meander_offset_blocks(
        corridor,
        downstream_blocks + step_t * segment_length_blocks,
        next_t,
        segment_length_blocks,
        centerline_world_x + step_t * segment_length_blocks,
        centerline_world_z,
        channel_radius,
        floodplain_radius,
        floodplain_bias,
        wetness,
        riverine_factor,
        dryland_bias,
        slope,
        meander_bias,
        lateral_variability,
        confinement_bias,
        lower_reach_signal,
        branch_seed,
    );
    let denominator = (step_t * step_t * segment_length_blocks.max(1.0)).max(1.0);

    ((next_offset - current_offset * 2.0 + prev_offset) / denominator).clamp(-1.0, 1.0)
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

fn band_influence(distance: f32, center: f32, half_width: f32) -> f32 {
    if half_width <= f32::EPSILON {
        return 0.0;
    }

    let distance_from_center = (distance - center).abs();
    let t = (distance_from_center / half_width).clamp(0.0, 1.0);
    1.0 - smootherstep01(t)
}

fn local_confluence_signal(
    all_corridors: &[RiverCorridorConstraint],
    corridor: RiverCorridorConstraint,
    local_x: f32,
    local_z: f32,
    water_radius: f32,
    bank_radius: f32,
) -> f32 {
    let mut signal = 0.0_f32;

    if corridor.parent_river_id.is_some() {
        let distance =
            ((local_x - corridor.end_x).powi(2) + (local_z - corridor.end_z).powi(2)).sqrt();
        let radius = (bank_radius + corridor.half_width_blocks.sqrt() * 0.80 + 8.0)
            .clamp(6.0, corridor.half_width_blocks * 0.28 + 26.0);
        signal = signal.max(radial_influence(distance, radius, 0.70));
    }

    for other in all_corridors {
        if other.river_id == corridor.river_id || other.parent_river_id != Some(corridor.river_id) {
            continue;
        }

        let distance = ((local_x - other.end_x).powi(2) + (local_z - other.end_z).powi(2)).sqrt();
        let radius =
            (bank_radius.max(water_radius + 1.0) + other.half_width_blocks.sqrt() * 0.95 + 8.0)
                .clamp(
                    6.0,
                    corridor.half_width_blocks * 0.30 + other.half_width_blocks * 0.12 + 28.0,
                );
        signal = signal.max(radial_influence(distance, radius, 0.66));
    }

    signal.clamp(0.0, 1.0)
}

fn column_index(local_x: i32, local_z: i32) -> usize {
    local_z as usize * CHUNK_EDGE_I32 as usize + local_x as usize
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

    fn style_test_corridor(
        kind: RiverPathKind,
        half_width_blocks: f32,
        downstream_grade_per_block: f32,
    ) -> RiverCorridorConstraint {
        RiverCorridorConstraint {
            river_id: 0x1357_2468,
            basin_id: 0x1357_2468,
            main_stem_river_id: 0x1357_2468,
            parent_river_id: None,
            kind,
            order: match kind {
                RiverPathKind::Trunk => 3,
                RiverPathKind::Tributary => 1,
            },
            start_x: 0.0,
            start_z: 0.0,
            end_x: 256.0,
            end_z: 0.0,
            center_x: 128.0,
            center_z: 0.0,
            half_width_blocks,
            downstream_grade_per_block,
            downstream_cells_start: 0.0,
            downstream_cells_end: 1.0,
        }
    }

    #[test]
    fn meander_profile_uses_branch_progress_without_segment_reset() {
        let corridor = RiverCorridorConstraint {
            river_id: 0xA5A5_1357,
            basin_id: 0xA5A5_1357,
            main_stem_river_id: 0xA5A5_1357,
            parent_river_id: None,
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
            downstream_cells_start: 0.0,
            downstream_cells_end: 1.0,
        };
        let branch_seed = branch_noise_seed(corridor);

        let start = centerline_meander_offset_blocks(
            corridor,
            0.0,
            0.0,
            256.0,
            0.0,
            0.0,
            9.0,
            18.0,
            0.75,
            0.70,
            0.65,
            0.08,
            0.10,
            0.80,
            0.70,
            0.30,
            0.25,
            branch_seed,
        );
        let quarter = centerline_meander_offset_blocks(
            corridor,
            64.0,
            0.25,
            256.0,
            64.0,
            0.0,
            9.0,
            18.0,
            0.75,
            0.70,
            0.65,
            0.08,
            0.10,
            0.80,
            0.70,
            0.30,
            0.25,
            branch_seed,
        );
        let three_quarter = centerline_meander_offset_blocks(
            corridor,
            192.0,
            0.75,
            256.0,
            192.0,
            0.0,
            9.0,
            18.0,
            0.75,
            0.70,
            0.65,
            0.08,
            0.10,
            0.80,
            0.70,
            0.30,
            0.25,
            branch_seed,
        );
        let end = centerline_meander_offset_blocks(
            corridor,
            256.0,
            1.0,
            256.0,
            256.0,
            0.0,
            9.0,
            18.0,
            0.75,
            0.70,
            0.65,
            0.08,
            0.10,
            0.80,
            0.70,
            0.30,
            0.25,
            branch_seed,
        );

        assert!(start.abs() <= 0.001);
        assert!(end.abs() >= 0.10);
        assert!(quarter.abs() >= 0.75);
        assert!(three_quarter.abs() >= 0.75);
        assert!((end - quarter).abs() >= 0.10);
    }

    #[test]
    fn lower_broad_reaches_request_wider_deeper_carves_than_shallow_sources() {
        let lower_corridor = style_test_corridor(RiverPathKind::Trunk, 56.0, 0.0007);
        let source_corridor = style_test_corridor(RiverPathKind::Tributary, 18.0, 0.0058);
        let lower_region = RegionHydrologySignals {
            floodplain_bias: 0.82,
            wetland_bias: 0.38,
            transition_softness: 0.76,
            width_variability: 0.45,
            meander_bias: 0.34,
            outer_spread_bias: 0.72,
            ..RegionHydrologySignals::default()
        };
        let source_region = RegionHydrologySignals {
            incision_bias: 0.28,
            confinement_bias: 0.62,
            depth_variability: 0.24,
            ..RegionHydrologySignals::default()
        };

        let lower_style = resolve_reach_carve_style(
            lower_corridor,
            0.92,
            0.018,
            0.88,
            0.92,
            0.08,
            0.94,
            lower_region,
            0.06,
            0.10,
        );
        let source_style = resolve_reach_carve_style(
            source_corridor,
            0.06,
            0.006,
            0.18,
            0.22,
            0.62,
            0.20,
            source_region,
            0.72,
            -0.08,
        );

        assert!(lower_style.lower_broad_signal > source_style.lower_broad_signal);
        assert!(source_style.shallow_source_signal > lower_style.shallow_source_signal);
        assert!(lower_style.max_water_depth > source_style.max_water_depth + 3.0);
        assert!(lower_style.water_width_scale > source_style.water_width_scale);
        assert!(lower_style.floodplain_width_scale > source_style.floodplain_width_scale);
        assert!(lower_style.floodplain_profile_strength > source_style.floodplain_profile_strength);
    }

    #[test]
    fn meandering_reaches_favor_stronger_bars_than_straight_reaches() {
        let corridor = style_test_corridor(RiverPathKind::Trunk, 42.0, 0.0018);
        let region = RegionHydrologySignals {
            floodplain_bias: 0.42,
            transition_softness: 0.46,
            lateral_variability: 0.44,
            meander_bias: 0.78,
            outer_spread_bias: 0.34,
            ..RegionHydrologySignals::default()
        };

        let meander_style = resolve_reach_carve_style(
            corridor, 0.46, 0.090, 0.58, 0.62, 0.16, 0.58, region, 0.18, 0.20,
        );
        let straight_style = resolve_reach_carve_style(
            corridor, 0.46, 0.000, 0.58, 0.62, 0.16, 0.58, region, 0.18, 0.20,
        );

        assert!(meander_style.meander_signal > straight_style.meander_signal);
        assert!(straight_style.straight_signal > meander_style.straight_signal);
        assert!(meander_style.bar_strength_scale > straight_style.bar_strength_scale);
        assert!(meander_style.core_profile_strength > straight_style.core_profile_strength);
    }

    #[test]
    fn water_profile_never_caps_surface_to_local_terrain() {
        let profile = water_profile::resolve_water_profile(water_profile::WaterProfileInput {
            smoothed_height: 4.0,
            remaining_relief_budget: 12.0,
            start_anchor: 12.0,
            end_anchor: 8.0,
            along_t: 0.25,
            incision_bias: 0.4,
            local_channel_depth: 3.0,
        });

        assert_eq!(profile.water_surface, 11.0);
    }

    #[test]
    fn water_profile_never_rises_downstream() {
        let upstream = water_profile::resolve_water_profile(water_profile::WaterProfileInput {
            smoothed_height: 18.0,
            remaining_relief_budget: 8.0,
            start_anchor: 10.0,
            end_anchor: 14.0,
            along_t: 0.0,
            incision_bias: 0.2,
            local_channel_depth: 3.0,
        });
        let downstream = water_profile::resolve_water_profile(water_profile::WaterProfileInput {
            smoothed_height: 18.0,
            remaining_relief_budget: 8.0,
            start_anchor: 10.0,
            end_anchor: 14.0,
            along_t: 1.0,
            incision_bias: 0.2,
            local_channel_depth: 3.0,
        });

        assert!(downstream.water_surface <= upstream.water_surface);
        assert_eq!(downstream.water_surface, upstream.water_surface);
    }

    #[test]
    fn visible_water_uses_profile_anchor_without_local_depth_rewrite() {
        let standing_water =
            water_surface::resolve_visible_water_surface(water_surface::VisibleWaterInput {
                water_presence: 0.95,
                channel_has_visible_water: true,
                lake_bias: 0.0,
                anchored_water_surface: 8.0,
                terrain_height: 6.2,
                channel_floor_y: 3.5,
            });

        assert_eq!(standing_water, Some(8.0));
    }

    #[test]
    fn visible_water_does_not_raise_to_clear_local_terrain() {
        let standing_water =
            water_surface::resolve_visible_water_surface(water_surface::VisibleWaterInput {
                water_presence: 0.95,
                channel_has_visible_water: true,
                lake_bias: 0.0,
                anchored_water_surface: 8.0,
                terrain_height: 7.8,
                channel_floor_y: 3.5,
            });

        assert_eq!(standing_water, None);
    }

    #[test]
    fn carve_supported_water_surface_uses_depth_reference_below_profile_anchor() {
        let water_surface = water_surface::carve_supported_water_surface(12.0, 6.0, 2.25);

        assert_eq!(water_surface, 8.25);
    }

    #[test]
    fn carve_supported_water_surface_never_raises_above_profile_anchor() {
        let water_surface = water_surface::carve_supported_water_surface(7.0, 6.8, 1.25);

        assert_eq!(water_surface, 7.0);
    }

    #[test]
    fn shoreline_support_relaxation_lowers_near_water_spikes() {
        let relaxed = apply_shoreline_support_relaxation(ShorelineSupportInput {
            terrain_height: 12.0,
            depositional_water_surface: 2.0,
            water_radius: 3.0,
            bank_radius: 8.0,
            floodplain_radius: 18.0,
            distance_blocks: 3.65,
            lower_reach_signal: 0.82,
            transition_softness: 0.74,
            outer_spread_bias: 0.66,
            water_sheet_influence: 0.04,
            core_influence: 0.02,
            bank_influence: 0.92,
            floodplain_influence: 0.70,
            confinement: 0.16,
            slope_signal: 0.08,
            edge_noise: 0.20,
        });

        assert!(
            relaxed <= 5.25,
            "near-water bank should relax toward water support instead of remaining a pillar: {relaxed:.3}"
        );
    }

    #[test]
    fn shoreline_support_relaxation_does_not_raise_supported_banks() {
        let relaxed = apply_shoreline_support_relaxation(ShorelineSupportInput {
            terrain_height: 2.8,
            depositional_water_surface: 2.0,
            water_radius: 3.0,
            bank_radius: 8.0,
            floodplain_radius: 18.0,
            distance_blocks: 3.65,
            lower_reach_signal: 0.82,
            transition_softness: 0.74,
            outer_spread_bias: 0.66,
            water_sheet_influence: 0.04,
            core_influence: 0.02,
            bank_influence: 0.92,
            floodplain_influence: 0.70,
            confinement: 0.16,
            slope_signal: 0.08,
            edge_noise: 0.20,
        });

        assert_eq!(relaxed, 2.8);
    }

    #[test]
    fn isolated_shoreline_pillar_suppression_lowers_dry_local_peak() {
        let mut columns = vec![
            HydrologyColumn {
                terrain_height: 1.0,
                water_surface_height: None,
                channel_floor_height: None,
                saturation: 0.0,
                gravel_bar_strength: 0.0,
                mode: HydrologyMode::Dry,
            };
            (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize
        ];
        let peak_index = column_index(12, 12);
        columns[peak_index].terrain_height = 12.0;
        columns[column_index(24, 12)] = HydrologyColumn {
            terrain_height: 1.0,
            water_surface_height: Some(2.25),
            channel_floor_height: Some(0.75),
            saturation: 1.0,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Channel,
        };

        suppress_isolated_shoreline_pillars(&mut columns);

        assert!(
            columns[peak_index].terrain_height <= 1.05,
            "isolated shoreline pillar should be clamped to local support: {:.3}",
            columns[peak_index].terrain_height
        );
        assert_eq!(columns[peak_index].mode, HydrologyMode::Wetland);
    }

    #[test]
    fn isolated_shoreline_pillar_suppression_removes_low_residual_peaks() {
        let mut columns = vec![
            HydrologyColumn {
                terrain_height: 4.0,
                water_surface_height: None,
                channel_floor_height: None,
                saturation: 0.0,
                gravel_bar_strength: 0.0,
                mode: HydrologyMode::Dry,
            };
            (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize
        ];
        let peak_index = column_index(16, 16);
        columns[peak_index].terrain_height = 6.0;
        columns[column_index(22, 16)] = HydrologyColumn {
            terrain_height: 1.0,
            water_surface_height: Some(2.25),
            channel_floor_height: Some(0.75),
            saturation: 1.0,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Channel,
        };

        suppress_isolated_shoreline_pillars(&mut columns);

        assert!(
            columns[peak_index].terrain_height <= 4.05,
            "small dry local peak near water should be removed, not left as a residual pillar: {:.3}",
            columns[peak_index].terrain_height
        );
        assert_eq!(columns[peak_index].mode, HydrologyMode::Wetland);
    }

    #[test]
    fn isolated_shoreline_pillar_suppression_removes_saturated_edge_peaks() {
        let mut columns = vec![
            HydrologyColumn {
                terrain_height: 4.0,
                water_surface_height: None,
                channel_floor_height: None,
                saturation: 0.38,
                gravel_bar_strength: 0.0,
                mode: HydrologyMode::Dry,
            };
            (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize
        ];
        let peak_index = column_index(16, 1);
        columns[peak_index].terrain_height = 6.0;

        suppress_isolated_shoreline_pillars(&mut columns);

        assert!(
            columns[peak_index].terrain_height <= 4.05,
            "saturated dry edge peak should be treated as a possible cross-chunk shoreline pillar: {:.3}",
            columns[peak_index].terrain_height
        );
        assert_eq!(columns[peak_index].mode, HydrologyMode::Wetland);
    }

    #[test]
    fn weak_shoreline_relaxation_lowers_water_adjacent_dry_fallback_column() {
        let mut columns = vec![
            HydrologyColumn {
                terrain_height: 12.0,
                water_surface_height: None,
                channel_floor_height: None,
                saturation: 0.0,
                gravel_bar_strength: 0.0,
                mode: HydrologyMode::Dry,
            };
            (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize
        ];
        let mut branch_keys = vec![None; columns.len()];
        let water_index = column_index(16, 16);
        columns[water_index] = HydrologyColumn {
            terrain_height: 4.2,
            water_surface_height: Some(6.0),
            channel_floor_height: Some(3.8),
            saturation: 1.0,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Channel,
        };
        branch_keys[water_index] = Some(BranchKey {
            river_id: 1,
            kind_rank: 0,
            order: 2,
        });
        let fallback_index = column_index(17, 16);

        relax_weak_shoreline_columns(&mut columns, &branch_keys);

        assert!(
            columns[fallback_index].terrain_height <= 6.90,
            "fallback dry shoreline column should be carved down to a low bank instead of remaining tall: {:.3}",
            columns[fallback_index].terrain_height
        );
        assert_eq!(columns[fallback_index].mode, HydrologyMode::Floodplain);
    }

    #[test]
    fn weak_shoreline_relaxation_keeps_supported_corridor_bank() {
        let mut columns = vec![
            HydrologyColumn {
                terrain_height: 12.0,
                water_surface_height: None,
                channel_floor_height: None,
                saturation: 0.0,
                gravel_bar_strength: 0.0,
                mode: HydrologyMode::Dry,
            };
            (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize
        ];
        let branch_key = BranchKey {
            river_id: 1,
            kind_rank: 0,
            order: 2,
        };
        let mut branch_keys = vec![None; columns.len()];
        let water_index = column_index(16, 16);
        columns[water_index] = HydrologyColumn {
            terrain_height: 4.2,
            water_surface_height: Some(6.0),
            channel_floor_height: Some(3.8),
            saturation: 1.0,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Channel,
        };
        branch_keys[water_index] = Some(branch_key);
        let corridor_bank_index = column_index(17, 16);
        columns[corridor_bank_index].channel_floor_height = Some(10.5);
        columns[corridor_bank_index].mode = HydrologyMode::Floodplain;
        branch_keys[corridor_bank_index] = Some(branch_key);

        relax_weak_shoreline_columns(&mut columns, &branch_keys);

        assert_eq!(columns[corridor_bank_index].terrain_height, 12.0);
        assert_eq!(columns[corridor_bank_index].mode, HydrologyMode::Floodplain);
    }

    #[test]
    fn water_border_constraint_lowers_component_to_lower_bank_support() {
        let mut columns = vec![
            HydrologyColumn {
                terrain_height: 7.2,
                water_surface_height: None,
                channel_floor_height: None,
                saturation: 0.0,
                gravel_bar_strength: 0.0,
                mode: HydrologyMode::Dry,
            };
            (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize
        ];
        let water_index = column_index(16, 16);
        columns[water_index] = HydrologyColumn {
            terrain_height: 4.0,
            water_surface_height: Some(6.1),
            channel_floor_height: Some(3.0),
            saturation: 1.0,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Channel,
        };
        let margin_index = column_index(17, 16);
        columns[margin_index].terrain_height = 5.2;
        columns[margin_index].saturation = 0.62;
        columns[margin_index].mode = HydrologyMode::Floodplain;

        constrain_water_components_to_border_support(&mut columns);

        assert_eq!(columns[water_index].water_surface_height, Some(5.2));
        assert_eq!(columns[margin_index].water_surface_height, None);
        assert_eq!(columns[margin_index].mode, HydrologyMode::Floodplain);
    }

    #[test]
    fn water_border_constraint_keeps_surface_with_high_bank_support() {
        let mut columns = vec![
            HydrologyColumn {
                terrain_height: 6.2,
                water_surface_height: None,
                channel_floor_height: None,
                saturation: 0.0,
                gravel_bar_strength: 0.0,
                mode: HydrologyMode::Dry,
            };
            (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize
        ];
        let water_index = column_index(16, 16);
        columns[water_index] = HydrologyColumn {
            terrain_height: 4.0,
            water_surface_height: Some(6.1),
            channel_floor_height: Some(3.0),
            saturation: 1.0,
            gravel_bar_strength: 0.0,
            mode: HydrologyMode::Channel,
        };

        constrain_water_components_to_border_support(&mut columns);

        assert_eq!(columns[water_index].water_surface_height, Some(6.1));
        assert_eq!(columns[column_index(17, 16)].water_surface_height, None);
        assert_eq!(columns[water_index].mode, HydrologyMode::Channel);
    }
}
