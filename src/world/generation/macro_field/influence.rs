use rayon::prelude::*;
use std::collections::HashMap;

use super::context::{EstuaryFanRef, MacroFieldRasterContext};
use super::height::{
    boundary_roughness_offset, envelope, lerp, ridge_envelope, roughened_distance, smoothstep01,
};
use super::river::{
    nearest_point_on_segment, point_segment_distance, river_boundary_roughness_blocks,
    river_boundary_roughness_offset, river_core_strength_for_roughened_distance,
    river_hints_from_strength, river_shoulder_radius_blocks,
    river_valley_strength_for_roughened_distance, river_water_radius_blocks, signed_side,
    squared_distance,
};
use super::types::{MacroFieldSample, MacroFieldTileConfig, MacroFieldTileStats};
use crate::world::generation::boundary::NoisyBoundaryCurve;
use crate::world::generation::graph::{VoronoiEdgeId, WorldPlanePoint};

pub(super) const RIDGE_FIELD_SOURCE_MIN_RIDGENESS: f32 = 0.44;
pub(super) const RIVER_CORE_STRENGTH_THRESHOLD: f32 = 0.88;
pub(super) const RIVER_CONCAVE_CUSP_MIN_STRENGTH_RATIO: f32 = 0.72;
pub(super) const RIVER_CONCAVE_CUSP_MIN_NEIGHBORS: usize = 5;
pub(super) const RIVER_CONCAVE_CUSP_MAX_PASSES: usize = 2;
const RIVER_SHOULDER_CONCAVE_BAY_MIN_SUPPORT: f32 = 0.08;
const RIVER_SHOULDER_CONCAVE_BAY_PROMOTE_RATIO: f32 = 0.82;
const RIVER_SHOULDER_CONCAVE_BAY_MAX_PASSES: usize = 2;
const RIVER_SHOULDER_CONCAVE_BAY_MAX_RADIUS_BLOCKS: f32 = 8.0;
const RIVER_SHOULDER_CONCAVE_BAY_MAX_RADIUS_SAMPLES: isize = 8;
pub(super) const ESTUARY_FAN_EDGE_ROUGHNESS_BLOCKS: f32 = 24.0;
const ESTUARY_FAN_INLET_OVERLAP_WIDTH_SCALE: f32 = 1.25;
const TERMINAL_RIVER_TAPER_WIDTH_SCALE: f32 = 2.50;
const TERMINAL_MOUTH_WATER_WIDTH_BOOST: f32 = 0.65;
const TERMINAL_MOUTH_VALLEY_WIDTH_BOOST: f32 = 0.36;
const TERMINAL_MOUTH_DEPTH_BOOST: f32 = 0.18;
const TERMINAL_MOUTH_FLOW_BOOST: f32 = 0.30;
const TERMINAL_MOUTH_TAPER_BLEND: f32 = 1.0;
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct MacroFieldInfluenceStats {
    pub(super) ridge_source_curve_count: usize,
    pub(super) river_source_curve_count: usize,
    pub(super) coast_source_curve_count: usize,
    pub(super) ridge_source_pixel_count: usize,
    pub(super) river_source_pixel_count: usize,
    pub(super) coast_source_pixel_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MacroFieldInfluenceSample {
    pub(super) ridge_influence: f32,
    pub(super) coast_influence: f32,
    pub(super) river_core_strength: f32,
    pub(super) river_shoulder_strength: f32,
    pub(super) river_valley_strength: f32,
    pub(super) river_distance_blocks: f32,
    pub(super) river_centerline_position: Option<WorldPlanePoint>,
    pub(super) river_longitudinal_blocks: f32,
    pub(super) river_flow_hint: f32,
    pub(super) river_core_depth_hint: f32,
    pub(super) river_bank_roughness_hint: f32,
    pub(super) river_gravel_hint: f32,
    pub(super) river_cutbank_hint: f32,
    pub(super) estuary_strength: f32,
    pub(super) estuary_water_strength: f32,
    pub(super) estuary_flow_hint: f32,
    pub(super) estuary_bed_depth_hint: f32,
    pub(super) estuary_water_depth_hint: f32,
    pub(super) estuary_along_blocks: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct MacroFieldInfluenceFields {
    pub(super) ridge_distance_blocks: Vec<f32>,
    pub(super) coast_distance_blocks: Vec<f32>,
    pub(super) river_distance_blocks: Vec<f32>,
    pub(super) river_core_strength: Vec<f32>,
    pub(super) river_shoulder_strength: Vec<f32>,
    pub(super) river_valley_strength: Vec<f32>,
    pub(super) river_centerline_x: Vec<f32>,
    pub(super) river_centerline_z: Vec<f32>,
    pub(super) river_longitudinal_blocks: Vec<f32>,
    pub(super) river_flow_hint: Vec<f32>,
    pub(super) river_core_depth_hint: Vec<f32>,
    pub(super) river_bank_roughness_hint: Vec<f32>,
    pub(super) river_gravel_hint: Vec<f32>,
    pub(super) river_cutbank_hint: Vec<f32>,
    pub(super) estuary_strength: Vec<f32>,
    pub(super) estuary_water_strength: Vec<f32>,
    pub(super) estuary_flow_hint: Vec<f32>,
    pub(super) estuary_bed_depth_hint: Vec<f32>,
    pub(super) estuary_water_depth_hint: Vec<f32>,
    pub(super) estuary_along_blocks: Vec<f32>,
    pub(super) stats: MacroFieldInfluenceStats,
}

impl MacroFieldInfluenceFields {
    pub(super) fn sample(
        &self,
        index: usize,
        config: MacroFieldTileConfig,
    ) -> MacroFieldInfluenceSample {
        let ridge_distance = self.ridge_distance_blocks[index];
        let coast_distance = self.coast_distance_blocks[index];
        let river_distance = self.river_distance_blocks[index];
        let centerline_x = self.river_centerline_x[index];
        let centerline_z = self.river_centerline_z[index];
        let river_centerline_position = if centerline_x.is_finite() && centerline_z.is_finite() {
            Some(WorldPlanePoint::new(centerline_x, centerline_z))
        } else {
            None
        };
        let river_flow_hint = self.river_flow_hint[index];
        let river_core_strength = self.river_core_strength[index];
        let river_shoulder_strength = self.river_shoulder_strength[index];
        let river_valley_strength = self.river_valley_strength[index];

        MacroFieldInfluenceSample {
            ridge_influence: ridge_envelope(ridge_distance, config.ridge_radius_blocks),
            coast_influence: envelope(
                roughened_distance(
                    coast_distance,
                    config.sample_position(index),
                    config.boundary_roughness_blocks,
                    0xC0A5_7001,
                ),
                config.coast_radius_blocks,
            ),
            river_core_strength: river_core_strength.clamp(0.0, 1.0),
            river_shoulder_strength: river_shoulder_strength.clamp(0.0, 1.0),
            river_valley_strength: river_valley_strength.clamp(0.0, 1.0),
            river_distance_blocks: river_distance,
            river_centerline_position,
            river_longitudinal_blocks: if self.river_longitudinal_blocks[index].is_finite() {
                self.river_longitudinal_blocks[index]
            } else {
                0.0
            },
            river_flow_hint,
            river_core_depth_hint: self.river_core_depth_hint[index].clamp(0.0, 1.0),
            river_bank_roughness_hint: self.river_bank_roughness_hint[index].clamp(0.0, 1.0),
            river_gravel_hint: self.river_gravel_hint[index].clamp(0.0, 1.0),
            river_cutbank_hint: self.river_cutbank_hint[index].clamp(0.0, 1.0),
            estuary_strength: self.estuary_strength[index].clamp(0.0, 1.0),
            estuary_water_strength: self.estuary_water_strength[index].clamp(0.0, 1.0),
            estuary_flow_hint: self.estuary_flow_hint[index].clamp(0.0, 1.0),
            estuary_bed_depth_hint: self.estuary_bed_depth_hint[index].clamp(0.0, 1.0),
            estuary_water_depth_hint: self.estuary_water_depth_hint[index].clamp(0.0, 1.0),
            estuary_along_blocks: self.estuary_along_blocks[index].max(0.0),
        }
    }
}

pub(super) fn rasterize_influence_fields(
    context: &MacroFieldRasterContext<'_>,
    config: MacroFieldTileConfig,
) -> MacroFieldInfluenceFields {
    let ridge_sources = context
        .ridge_curves
        .iter()
        .map(|curve| (*curve, 1.0))
        .collect::<Vec<_>>();
    let coast_sources = context
        .coast_curves
        .iter()
        .map(|curve| (*curve, 1.0))
        .collect::<Vec<_>>();
    let river_sources = context
        .river_curves
        .iter()
        .map(|river| RiverRasterSource {
            is_terminal_outlet: river.is_terminal_outlet,
            edge: river.edge,
            points: &river.points,
            longitudinal_start_blocks: river.longitudinal_start_blocks,
            flow_hint: river.flow_hint,
            water_width_blocks: river.water_width_blocks,
            valley_width_blocks: river.valley_width_blocks,
            bed_depth_blocks: river.bed_depth_blocks,
            terminal_mouth_factor: river.terminal_mouth_factor,
            component_id: 0,
        })
        .collect::<Vec<_>>();

    let ridge = rasterize_curve_distance_field(&ridge_sources, config, config.ridge_radius_blocks);
    let coast = rasterize_curve_distance_field(&coast_sources, config, config.coast_radius_blocks);
    let mut river = rasterize_curve_anti_aliased_polyline_field(
        &river_sources,
        config,
        config.river_radius_blocks,
    );
    smooth_river_concave_cusps(&mut river, config.width as usize, config.height as usize);
    let estuary = rasterize_estuary_fan_field(&context.estuary_fans, config);
    let stats = MacroFieldInfluenceStats {
        ridge_source_curve_count: ridge_sources.len(),
        river_source_curve_count: river_sources.len(),
        coast_source_curve_count: coast_sources.len(),
        ridge_source_pixel_count: ridge.source_pixel_count,
        river_source_pixel_count: river.source_pixel_count,
        coast_source_pixel_count: coast.source_pixel_count,
    };

    MacroFieldInfluenceFields {
        ridge_distance_blocks: ridge.distance_blocks,
        coast_distance_blocks: coast.distance_blocks,
        river_distance_blocks: river.distance_blocks,
        river_core_strength: river.river_core_strength,
        river_shoulder_strength: river.river_shoulder_strength,
        river_valley_strength: river.river_valley_strength,
        river_centerline_x: river.river_centerline_x,
        river_centerline_z: river.river_centerline_z,
        river_longitudinal_blocks: river.river_longitudinal_blocks,
        river_flow_hint: river.flow_hint,
        river_core_depth_hint: river.river_core_depth_hint,
        river_bank_roughness_hint: river.river_bank_roughness_hint,
        river_gravel_hint: river.river_gravel_hint,
        river_cutbank_hint: river.river_cutbank_hint,
        estuary_strength: estuary.strength,
        estuary_water_strength: estuary.water_strength,
        estuary_flow_hint: estuary.flow_hint,
        estuary_bed_depth_hint: estuary.bed_depth_hint,
        estuary_water_depth_hint: estuary.water_depth_hint,
        estuary_along_blocks: estuary.along_blocks,
        stats,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct EstuaryFanField {
    pub(super) strength: Vec<f32>,
    pub(super) water_strength: Vec<f32>,
    pub(super) flow_hint: Vec<f32>,
    pub(super) bed_depth_hint: Vec<f32>,
    pub(super) water_depth_hint: Vec<f32>,
    pub(super) along_blocks: Vec<f32>,
}

pub(super) fn rasterize_estuary_fan_field(
    fans: &[EstuaryFanRef],
    config: MacroFieldTileConfig,
) -> EstuaryFanField {
    let sample_count = config.sample_count();
    if fans.is_empty() {
        return EstuaryFanField {
            strength: vec![0.0; sample_count],
            water_strength: vec![0.0; sample_count],
            flow_hint: vec![0.0; sample_count],
            bed_depth_hint: vec![0.0; sample_count],
            water_depth_hint: vec![0.0; sample_count],
            along_blocks: vec![0.0; sample_count],
        };
    }

    let samples = (0..sample_count)
        .into_par_iter()
        .map(|index| {
            let position = config.sample_position(index);
            fans.iter()
                .map(|fan| estuary_fan_sample(*fan, position))
                .fold(EstuaryFanSample::default(), strongest_estuary_sample)
        })
        .collect::<Vec<_>>();

    EstuaryFanField {
        strength: samples.iter().map(|sample| sample.strength).collect(),
        water_strength: samples.iter().map(|sample| sample.water_strength).collect(),
        flow_hint: samples.iter().map(|sample| sample.flow_hint).collect(),
        bed_depth_hint: samples.iter().map(|sample| sample.bed_depth_hint).collect(),
        water_depth_hint: samples
            .iter()
            .map(|sample| sample.water_depth_hint)
            .collect(),
        along_blocks: samples.iter().map(|sample| sample.along_blocks).collect(),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct EstuaryFanSample {
    pub(super) strength: f32,
    pub(super) water_strength: f32,
    pub(super) flow_hint: f32,
    pub(super) bed_depth_hint: f32,
    pub(super) water_depth_hint: f32,
    pub(super) along_blocks: f32,
}

pub(super) fn estuary_fan_sample(
    fan: EstuaryFanRef,
    position: WorldPlanePoint,
) -> EstuaryFanSample {
    let dx = position.x - fan.origin.x;
    let dz = position.z - fan.origin.z;
    let along = dx * fan.direction_x + dz * fan.direction_z;
    let inlet_overlap_blocks =
        (fan.start_half_width_blocks * ESTUARY_FAN_INLET_OVERLAP_WIDTH_SCALE).max(1.0);
    if along < -inlet_overlap_blocks || along > fan.length_blocks {
        return EstuaryFanSample::default();
    }

    let lateral = (dx * -fan.direction_z + dz * fan.direction_x).abs();
    let progress = (along.max(0.0) / fan.length_blocks.max(f32::EPSILON)).clamp(0.0, 1.0);
    let half_width = estuary_fan_half_width_blocks(fan, progress);
    let edge_noise = boundary_roughness_offset(
        position,
        ESTUARY_FAN_EDGE_ROUGHNESS_BLOCKS * (0.45 + progress * 0.75),
        0xE57A_27F4_0001,
    );
    let edge_noise_weight = smoothstep01((lateral / half_width.max(f32::EPSILON)).clamp(0.0, 1.0));
    let rough_lateral = (lateral + edge_noise * edge_noise_weight).max(0.0);
    let carve_half_width = half_width;
    let carve_cross =
        1.0 - smoothstep01((rough_lateral / carve_half_width.max(f32::EPSILON)).clamp(0.0, 1.0));
    let water_cross =
        1.0 - smoothstep01((rough_lateral / half_width.max(f32::EPSILON)).clamp(0.0, 1.0));
    if carve_cross <= f32::EPSILON && water_cross <= f32::EPSILON {
        return EstuaryFanSample::default();
    }

    let flow_t = smoothstep01(fan.flow_hint.clamp(0.0, 1.0));
    let inlet_extension_t = if along < 0.0 {
        smoothstep01(((along + inlet_overlap_blocks) / inlet_overlap_blocks).clamp(0.0, 1.0))
    } else {
        1.0
    };
    let upstream_overlap = along < 0.0;
    let inlet_blend = if upstream_overlap {
        inlet_extension_t * 0.58
    } else {
        0.72 + smoothstep01((progress / 0.20).clamp(0.0, 1.0)) * 0.28
    };
    let along_strength = 1.0 - smoothstep01(progress);
    let shelf_tail = 1.0 - smoothstep01((progress - 0.78) / 0.22);
    let tail_floor = 0.48 - flow_t * 0.12;
    let strength =
        (carve_cross * inlet_blend * along_strength.max(shelf_tail * tail_floor)).clamp(0.0, 1.0);
    let water_edge_t = (rough_lateral / half_width.max(f32::EPSILON)).clamp(0.0, 1.0);
    let water_strength = ((if water_edge_t <= 0.92 {
        let inner_t = smoothstep01((water_edge_t / 0.92).clamp(0.0, 1.0));
        lerp(1.0, RIVER_CORE_STRENGTH_THRESHOLD + 0.02, inner_t)
    } else {
        let edge_t = smoothstep01(((water_edge_t - 0.92) / 0.08).clamp(0.0, 1.0));
        lerp(RIVER_CORE_STRENGTH_THRESHOLD + 0.02, 0.0, edge_t)
    }) * inlet_extension_t)
        .clamp(0.0, 1.0);
    EstuaryFanSample {
        strength,
        water_strength,
        flow_hint: fan.flow_hint,
        bed_depth_hint: fan.bed_depth_hint,
        water_depth_hint: fan.water_depth_hint,
        along_blocks: along,
    }
}

pub(super) fn estuary_fan_half_width_blocks(fan: EstuaryFanRef, progress: f32) -> f32 {
    let t = smoothstep01(progress.clamp(0.0, 1.0));
    lerp(fan.start_half_width_blocks, fan.end_half_width_blocks, t)
}

fn strongest_estuary_sample(left: EstuaryFanSample, right: EstuaryFanSample) -> EstuaryFanSample {
    if right.strength > left.strength
        || ((right.strength - left.strength).abs() <= f32::EPSILON
            && right.water_strength > left.water_strength)
    {
        right
    } else {
        left
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct RasterDistanceField {
    pub(super) distance_blocks: Vec<f32>,
    pub(super) river_core_strength: Vec<f32>,
    pub(super) river_shoulder_strength: Vec<f32>,
    pub(super) river_valley_strength: Vec<f32>,
    pub(super) river_centerline_x: Vec<f32>,
    pub(super) river_centerline_z: Vec<f32>,
    pub(super) river_longitudinal_blocks: Vec<f32>,
    pub(super) flow_hint: Vec<f32>,
    pub(super) river_core_depth_hint: Vec<f32>,
    pub(super) river_bank_roughness_hint: Vec<f32>,
    pub(super) river_gravel_hint: Vec<f32>,
    pub(super) river_cutbank_hint: Vec<f32>,
    pub(super) source_pixel_count: usize,
}

pub(super) struct RiverRasterRow {
    pub(super) distance_blocks: Vec<f32>,
    pub(super) river_core_strength: Vec<f32>,
    pub(super) river_shoulder_strength: Vec<f32>,
    pub(super) river_valley_strength: Vec<f32>,
    pub(super) river_centerline_x: Vec<f32>,
    pub(super) river_centerline_z: Vec<f32>,
    pub(super) river_longitudinal_blocks: Vec<f32>,
    pub(super) centerline_weighted_x_sum: Vec<f32>,
    pub(super) centerline_weighted_z_sum: Vec<f32>,
    pub(super) centerline_weight_sum: Vec<f32>,
    pub(super) longitudinal_weighted_sum: Vec<f32>,
    pub(super) longitudinal_weight_sum: Vec<f32>,
    pub(super) flow_weighted_sum: Vec<f32>,
    pub(super) flow_weight_sum: Vec<f32>,
    pub(super) river_core_depth_hint: Vec<f32>,
    pub(super) river_bank_roughness_hint: Vec<f32>,
    pub(super) river_gravel_hint: Vec<f32>,
    pub(super) river_cutbank_hint: Vec<f32>,
    pub(super) source_pixel_count: usize,
}

impl RiverRasterRow {
    pub(super) fn new(width: usize) -> Self {
        Self {
            distance_blocks: vec![f32::INFINITY; width],
            river_core_strength: vec![0.0; width],
            river_shoulder_strength: vec![0.0; width],
            river_valley_strength: vec![0.0; width],
            river_centerline_x: vec![f32::NAN; width],
            river_centerline_z: vec![f32::NAN; width],
            river_longitudinal_blocks: vec![f32::NAN; width],
            centerline_weighted_x_sum: vec![0.0; width],
            centerline_weighted_z_sum: vec![0.0; width],
            centerline_weight_sum: vec![0.0; width],
            longitudinal_weighted_sum: vec![0.0; width],
            longitudinal_weight_sum: vec![0.0; width],
            flow_weighted_sum: vec![0.0; width],
            flow_weight_sum: vec![0.0; width],
            river_core_depth_hint: vec![0.0; width],
            river_bank_roughness_hint: vec![0.0; width],
            river_gravel_hint: vec![0.0; width],
            river_cutbank_hint: vec![0.0; width],
            source_pixel_count: 0,
        }
    }
}

pub(super) fn smooth_river_concave_cusps(
    river: &mut RasterDistanceField,
    width: usize,
    height: usize,
) {
    let sample_count = width * height;
    if width == 0 || height == 0 || river.river_core_strength.len() != sample_count {
        return;
    }

    for _ in 0..RIVER_CONCAVE_CUSP_MAX_PASSES {
        let promote = (0..sample_count)
            .filter(|&index| is_river_concave_cusp(river, width, height, index))
            .collect::<Vec<_>>();
        if promote.is_empty() {
            break;
        }

        for index in promote {
            river.river_core_strength[index] =
                river.river_core_strength[index].max(RIVER_CORE_STRENGTH_THRESHOLD);
            river.river_valley_strength[index] =
                river.river_valley_strength[index].max(RIVER_CORE_STRENGTH_THRESHOLD);
            copy_strongest_neighbor_river_hints(river, width, height, index);
        }
    }
}

pub(super) fn is_river_concave_cusp(
    river: &RasterDistanceField,
    width: usize,
    height: usize,
    index: usize,
) -> bool {
    let strength = river.river_core_strength[index];
    if strength >= RIVER_CORE_STRENGTH_THRESHOLD
        || strength < RIVER_CORE_STRENGTH_THRESHOLD * RIVER_CONCAVE_CUSP_MIN_STRENGTH_RATIO
        || river.flow_hint[index] <= 0.0
        || !river.distance_blocks[index].is_finite()
    {
        return false;
    }

    let x = index % width;
    let z = index / width;
    river_neighbor_count(river, width, height, x, z) >= RIVER_CONCAVE_CUSP_MIN_NEIGHBORS
        && has_orthogonal_river_support(river, width, height, x, z)
}

pub(super) fn river_neighbor_count(
    river: &RasterDistanceField,
    width: usize,
    height: usize,
    x: usize,
    z: usize,
) -> usize {
    let mut count = 0;
    for dz in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            if neighbor_is_river(river, width, height, x, z, dx, dz) {
                count += 1;
            }
        }
    }
    count
}

pub(super) fn has_orthogonal_river_support(
    river: &RasterDistanceField,
    width: usize,
    height: usize,
    x: usize,
    z: usize,
) -> bool {
    let north = neighbor_is_river(river, width, height, x, z, 0, -1);
    let south = neighbor_is_river(river, width, height, x, z, 0, 1);
    let west = neighbor_is_river(river, width, height, x, z, -1, 0);
    let east = neighbor_is_river(river, width, height, x, z, 1, 0);

    (north || south) && (west || east)
}

pub(super) fn neighbor_is_river(
    river: &RasterDistanceField,
    width: usize,
    height: usize,
    x: usize,
    z: usize,
    dx: isize,
    dz: isize,
) -> bool {
    let Some(nx) = x.checked_add_signed(dx) else {
        return false;
    };
    let Some(nz) = z.checked_add_signed(dz) else {
        return false;
    };
    if nx >= width || nz >= height {
        return false;
    }

    river.river_core_strength[nz * width + nx] >= RIVER_CORE_STRENGTH_THRESHOLD
}

pub(super) fn copy_strongest_neighbor_river_hints(
    river: &mut RasterDistanceField,
    width: usize,
    height: usize,
    index: usize,
) {
    let x = index % width;
    let z = index / width;
    let mut best: Option<usize> = None;
    for dz in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            let Some(nx) = x.checked_add_signed(dx) else {
                continue;
            };
            let Some(nz) = z.checked_add_signed(dz) else {
                continue;
            };
            if nx >= width || nz >= height {
                continue;
            }
            let neighbor = nz * width + nx;
            if river.river_core_strength[neighbor] >= RIVER_CORE_STRENGTH_THRESHOLD
                && best.is_none_or(|best_index| {
                    river.river_core_strength[neighbor] > river.river_core_strength[best_index]
                })
            {
                best = Some(neighbor);
            }
        }
    }

    if let Some(neighbor) = best {
        river.river_core_depth_hint[index] =
            river.river_core_depth_hint[index].max(river.river_core_depth_hint[neighbor]);
        river.river_bank_roughness_hint[index] =
            river.river_bank_roughness_hint[index].max(river.river_bank_roughness_hint[neighbor]);
        river.river_gravel_hint[index] =
            river.river_gravel_hint[index].max(river.river_gravel_hint[neighbor]);
        river.river_cutbank_hint[index] =
            river.river_cutbank_hint[index].max(river.river_cutbank_hint[neighbor]);
        if !river.river_centerline_x[index].is_finite()
            || !river.river_centerline_z[index].is_finite()
        {
            river.river_centerline_x[index] = river.river_centerline_x[neighbor];
            river.river_centerline_z[index] = river.river_centerline_z[neighbor];
        }
        if !river.river_longitudinal_blocks[index].is_finite() {
            river.river_longitudinal_blocks[index] = river.river_longitudinal_blocks[neighbor];
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ShoulderBayPromotion {
    index: usize,
    shoulder_strength: f32,
    valley_strength: f32,
}

pub(super) fn close_river_shoulder_concave_bays(
    river: &mut RasterDistanceField,
    width: usize,
    height: usize,
    sample_spacing_blocks: f32,
) {
    let sample_count = width * height;
    if width == 0 || height == 0 || river.river_shoulder_strength.len() != sample_count {
        return;
    }

    let max_radius_samples = ((RIVER_SHOULDER_CONCAVE_BAY_MAX_RADIUS_BLOCKS
        / sample_spacing_blocks.max(f32::EPSILON))
    .ceil() as isize)
        .clamp(1, RIVER_SHOULDER_CONCAVE_BAY_MAX_RADIUS_SAMPLES);
    for _ in 0..RIVER_SHOULDER_CONCAVE_BAY_MAX_PASSES {
        let promote = (0..sample_count)
            .filter_map(|index| {
                river_shoulder_concave_bay_promotion(
                    river,
                    width,
                    height,
                    index,
                    max_radius_samples,
                )
            })
            .collect::<Vec<_>>();
        if promote.is_empty() {
            break;
        }

        for promotion in promote {
            let index = promotion.index;
            river.river_shoulder_strength[index] =
                river.river_shoulder_strength[index].max(promotion.shoulder_strength);
            river.river_valley_strength[index] =
                river.river_valley_strength[index].max(promotion.valley_strength);
            copy_strongest_neighbor_shoulder_hints(river, width, height, index);
        }
    }
}

fn river_shoulder_concave_bay_promotion(
    river: &RasterDistanceField,
    width: usize,
    height: usize,
    index: usize,
    max_radius_samples: isize,
) -> Option<ShoulderBayPromotion> {
    let current_shoulder = river.river_shoulder_strength[index].clamp(0.0, 1.0);
    if current_shoulder >= 1.0 - f32::EPSILON {
        return None;
    }

    let x = index % width;
    let z = index / width;
    let immediate_support = strongest_neighbor_shoulder_strength(river, width, height, x, z);
    if immediate_support <= RIVER_SHOULDER_CONCAVE_BAY_MIN_SUPPORT
        || current_shoulder >= immediate_support * RIVER_SHOULDER_CONCAVE_BAY_PROMOTE_RATIO
    {
        return None;
    }

    let mut target_shoulder = current_shoulder;
    let mut target_valley = river.river_valley_strength[index].clamp(0.0, 1.0);
    for [(left_dx, left_dz), (right_dx, right_dz)] in [
        [(-1isize, 0isize), (1isize, 0isize)],
        [(0isize, -1isize), (0isize, 1isize)],
        [(-1isize, -1isize), (1isize, 1isize)],
        [(-1isize, 1isize), (1isize, -1isize)],
    ] {
        for radius in 1..=max_radius_samples {
            let Some(left) = offset_index(width, height, x, z, left_dx * radius, left_dz * radius)
            else {
                continue;
            };
            let Some(right) =
                offset_index(width, height, x, z, right_dx * radius, right_dz * radius)
            else {
                continue;
            };
            let support_shoulder =
                river.river_shoulder_strength[left].min(river.river_shoulder_strength[right]);
            if support_shoulder <= RIVER_SHOULDER_CONCAVE_BAY_MIN_SUPPORT {
                continue;
            }
            let promoted_shoulder = support_shoulder * RIVER_SHOULDER_CONCAVE_BAY_PROMOTE_RATIO;
            if promoted_shoulder <= current_shoulder {
                continue;
            }

            let support_valley =
                river.river_valley_strength[left].min(river.river_valley_strength[right]);
            target_shoulder = target_shoulder.max(promoted_shoulder);
            target_valley =
                target_valley.max(support_valley * RIVER_SHOULDER_CONCAVE_BAY_PROMOTE_RATIO);
        }
    }

    (target_shoulder > current_shoulder + f32::EPSILON).then_some(ShoulderBayPromotion {
        index,
        shoulder_strength: target_shoulder.clamp(0.0, 1.0),
        valley_strength: target_valley.clamp(0.0, 1.0),
    })
}

fn strongest_neighbor_shoulder_strength(
    river: &RasterDistanceField,
    width: usize,
    height: usize,
    x: usize,
    z: usize,
) -> f32 {
    let mut strongest: f32 = 0.0;
    for dz in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            if let Some(index) = offset_index(width, height, x, z, dx, dz) {
                strongest = strongest.max(river.river_shoulder_strength[index]);
            }
        }
    }
    strongest
}

fn copy_strongest_neighbor_shoulder_hints(
    river: &mut RasterDistanceField,
    width: usize,
    height: usize,
    index: usize,
) {
    let x = index % width;
    let z = index / width;
    let mut best: Option<usize> = None;
    for dz in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            let Some(neighbor) = offset_index(width, height, x, z, dx, dz) else {
                continue;
            };
            if river.river_shoulder_strength[neighbor] > RIVER_SHOULDER_CONCAVE_BAY_MIN_SUPPORT
                && best.is_none_or(|best_index| {
                    river.river_shoulder_strength[neighbor]
                        > river.river_shoulder_strength[best_index]
                })
            {
                best = Some(neighbor);
            }
        }
    }

    if let Some(neighbor) = best {
        river.distance_blocks[index] =
            river.distance_blocks[index].min(river.distance_blocks[neighbor]);
        river.flow_hint[index] = river.flow_hint[index].max(river.flow_hint[neighbor]);
        river.river_core_depth_hint[index] =
            river.river_core_depth_hint[index].max(river.river_core_depth_hint[neighbor]);
        river.river_bank_roughness_hint[index] =
            river.river_bank_roughness_hint[index].max(river.river_bank_roughness_hint[neighbor]);
        river.river_gravel_hint[index] =
            river.river_gravel_hint[index].max(river.river_gravel_hint[neighbor]);
        river.river_cutbank_hint[index] =
            river.river_cutbank_hint[index].max(river.river_cutbank_hint[neighbor]);
        if !river.river_centerline_x[index].is_finite()
            || !river.river_centerline_z[index].is_finite()
        {
            river.river_centerline_x[index] = river.river_centerline_x[neighbor];
            river.river_centerline_z[index] = river.river_centerline_z[neighbor];
        }
        if !river.river_longitudinal_blocks[index].is_finite() {
            river.river_longitudinal_blocks[index] = river.river_longitudinal_blocks[neighbor];
        }
    }
}

fn offset_index(
    width: usize,
    height: usize,
    x: usize,
    z: usize,
    dx: isize,
    dz: isize,
) -> Option<usize> {
    let nx = x.checked_add_signed(dx)?;
    let nz = z.checked_add_signed(dz)?;
    (nx < width && nz < height).then_some(nz * width + nx)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RiverRasterSource<'a> {
    pub(super) is_terminal_outlet: bool,
    pub(super) edge: VoronoiEdgeId,
    pub(super) points: &'a [WorldPlanePoint],
    pub(super) longitudinal_start_blocks: f32,
    pub(super) flow_hint: f32,
    pub(super) water_width_blocks: f32,
    pub(super) valley_width_blocks: f32,
    pub(super) bed_depth_blocks: f32,
    pub(super) terminal_mouth_factor: f32,
    pub(super) component_id: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct RiverRasterWorkSource {
    pub(super) is_terminal_outlet: bool,
    pub(super) edge: VoronoiEdgeId,
    pub(super) points: Vec<WorldPlanePoint>,
    pub(super) cumulative_lengths: Vec<f32>,
    pub(super) open_start_cap: bool,
    pub(super) open_end_cap: bool,
    pub(super) longitudinal_start_blocks: f32,
    pub(super) flow_hint: f32,
    pub(super) water_width_blocks: f32,
    pub(super) valley_width_blocks: f32,
    pub(super) bed_depth_blocks: f32,
    pub(super) terminal_mouth_factor: f32,
    pub(super) component_id: usize,
}

pub(super) fn rounded_river_raster_sources(
    sources: &[RiverRasterSource<'_>],
    configured_radius_blocks: f32,
) -> Vec<RiverRasterWorkSource> {
    sources
        .iter()
        .map(|source| {
            let points = rounded_river_raster_points(
                source.points,
                source.flow_hint,
                source.water_width_blocks,
                source.valley_width_blocks,
                configured_radius_blocks,
            );
            RiverRasterWorkSource {
                is_terminal_outlet: source.is_terminal_outlet,
                edge: source.edge,
                cumulative_lengths: cumulative_polyline_lengths(&points),
                points,
                open_start_cap: source.is_terminal_outlet,
                open_end_cap: false,
                longitudinal_start_blocks: source.longitudinal_start_blocks,
                flow_hint: source.flow_hint,
                water_width_blocks: source.water_width_blocks,
                valley_width_blocks: source.valley_width_blocks,
                bed_depth_blocks: source.bed_depth_blocks,
                terminal_mouth_factor: source.terminal_mouth_factor,
                component_id: source.component_id,
            }
        })
        .collect()
}

pub(super) fn cumulative_polyline_lengths(points: &[WorldPlanePoint]) -> Vec<f32> {
    let mut lengths = Vec::with_capacity(points.len());
    let mut total = 0.0;
    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            total += distance_between_points(points[index - 1], *point);
        }
        lengths.push(total);
    }
    lengths
}

pub(super) fn segment_longitudinal_blocks(
    source: &RiverRasterWorkSource,
    segment_index: usize,
    t: f32,
) -> f32 {
    let start = source
        .cumulative_lengths
        .get(segment_index)
        .copied()
        .unwrap_or(0.0);
    let end = source
        .cumulative_lengths
        .get(segment_index + 1)
        .copied()
        .unwrap_or(start);
    source.longitudinal_start_blocks + lerp(start, end, t.clamp(0.0, 1.0))
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EffectiveRiverMorphology {
    flow_hint: f32,
    water_width_blocks: f32,
    valley_width_blocks: f32,
    bed_depth_blocks: f32,
}

fn terminal_mouth_progress(
    source: &RiverRasterWorkSource,
    segment_index: usize,
    segment_t: f32,
) -> f32 {
    let end_factor = source.terminal_mouth_factor.clamp(0.0, 1.0);
    if end_factor <= f32::EPSILON {
        return 0.0;
    }

    let total_length = source.cumulative_lengths.last().copied().unwrap_or(0.0);
    if total_length <= f32::EPSILON {
        return end_factor;
    }

    let start = source
        .cumulative_lengths
        .get(segment_index)
        .copied()
        .unwrap_or(0.0);
    let end = source
        .cumulative_lengths
        .get(segment_index + 1)
        .copied()
        .unwrap_or(start);
    let along_source = lerp(start, end, segment_t.clamp(0.0, 1.0));
    let local_t = (along_source / total_length).clamp(0.0, 1.0);
    let start_factor = (end_factor - 0.55).max(0.0);
    lerp(start_factor, end_factor, smoothstep01(local_t)).clamp(0.0, 1.0)
}

fn effective_river_morphology(
    source: &RiverRasterWorkSource,
    terminal_mouth_progress: f32,
) -> EffectiveRiverMorphology {
    let width_gate = terminal_mouth_width_gate(source);
    let mouth = (terminal_mouth_progress * width_gate).clamp(0.0, 1.0);
    let base_water_width_blocks = terminal_mouth_base_water_width_blocks(source, width_gate);
    let base_valley_width_blocks =
        terminal_mouth_base_valley_width_blocks(source, width_gate, base_water_width_blocks);
    let water_width_blocks =
        base_water_width_blocks * (1.0 + TERMINAL_MOUTH_WATER_WIDTH_BOOST * mouth);
    let boosted_valley_width_blocks =
        base_valley_width_blocks * (1.0 + TERMINAL_MOUTH_VALLEY_WIDTH_BOOST * mouth);
    let valley_width_blocks = if source.is_terminal_outlet {
        let short_throat_width = (base_water_width_blocks * 1.08).max(water_width_blocks + 1.0);
        lerp(short_throat_width, boosted_valley_width_blocks, width_gate)
            .max(water_width_blocks + 1.0)
    } else {
        boosted_valley_width_blocks
    };

    EffectiveRiverMorphology {
        flow_hint: (source.flow_hint
            + (1.0 - source.flow_hint) * TERMINAL_MOUTH_FLOW_BOOST * mouth)
            .clamp(0.0, 1.0),
        water_width_blocks,
        valley_width_blocks,
        bed_depth_blocks: source.bed_depth_blocks * (1.0 + TERMINAL_MOUTH_DEPTH_BOOST * mouth),
    }
}

fn terminal_mouth_width_gate(source: &RiverRasterWorkSource) -> f32 {
    if !source.is_terminal_outlet {
        return 1.0;
    }

    let total_length = source.cumulative_lengths.last().copied().unwrap_or(0.0);
    let width_reference = (source.water_width_blocks.max(1.0) * 3.00)
        .max(source.valley_width_blocks.max(1.0) * 0.85)
        .max(1.0);
    smoothstep01((total_length / width_reference).clamp(0.0, 1.0))
}

fn terminal_mouth_base_water_width_blocks(source: &RiverRasterWorkSource, width_gate: f32) -> f32 {
    if source.is_terminal_outlet {
        lerp(
            source.water_width_blocks * 0.48,
            source.water_width_blocks,
            width_gate,
        )
    } else {
        source.water_width_blocks
    }
}

fn terminal_mouth_base_valley_width_blocks(
    source: &RiverRasterWorkSource,
    width_gate: f32,
    base_water_width_blocks: f32,
) -> f32 {
    if source.is_terminal_outlet {
        let terminal_valley_width =
            base_water_width_blocks * lerp(1.08, 1.85, smoothstep01(width_gate));
        terminal_valley_width.max(base_water_width_blocks + 1.0)
    } else {
        source.valley_width_blocks
    }
}

fn terminal_mouth_max_water_width_blocks(source: &RiverRasterWorkSource) -> f32 {
    let width_gate = terminal_mouth_width_gate(source);
    let mouth = source.terminal_mouth_factor.clamp(0.0, 1.0) * width_gate;
    terminal_mouth_base_water_width_blocks(source, width_gate)
        * (1.0 + TERMINAL_MOUTH_WATER_WIDTH_BOOST * mouth)
}

fn terminal_mouth_max_valley_width_blocks(source: &RiverRasterWorkSource) -> f32 {
    let width_gate = terminal_mouth_width_gate(source);
    let mouth = source.terminal_mouth_factor.clamp(0.0, 1.0) * width_gate;
    let base_water_width_blocks = terminal_mouth_base_water_width_blocks(source, width_gate);
    let water_width_blocks =
        base_water_width_blocks * (1.0 + TERMINAL_MOUTH_WATER_WIDTH_BOOST * mouth);
    let base_valley_width_blocks =
        terminal_mouth_base_valley_width_blocks(source, width_gate, base_water_width_blocks);
    let boosted_valley_width_blocks =
        base_valley_width_blocks * (1.0 + TERMINAL_MOUTH_VALLEY_WIDTH_BOOST * mouth);
    if source.is_terminal_outlet {
        let short_throat_width = (base_water_width_blocks * 1.08).max(water_width_blocks + 1.0);
        lerp(short_throat_width, boosted_valley_width_blocks, width_gate)
            .max(water_width_blocks + 1.0)
    } else {
        boosted_valley_width_blocks
    }
}

fn terminal_mouth_max_flow_hint(source: &RiverRasterWorkSource) -> f32 {
    let mouth = source.terminal_mouth_factor.clamp(0.0, 1.0) * terminal_mouth_width_gate(source);
    (source.flow_hint + (1.0 - source.flow_hint) * TERMINAL_MOUTH_FLOW_BOOST * mouth)
        .clamp(0.0, 1.0)
}

fn river_segment_bend_strength(source: &RiverRasterWorkSource, segment_index: usize) -> f32 {
    if source.points.len() < 3 || segment_index + 1 >= source.points.len() {
        return 0.0;
    }
    let start = source.points[segment_index];
    let end = source.points[segment_index + 1];
    let start_turn = segment_index
        .checked_sub(1)
        .map(|previous| signed_turn_strength(source.points[previous], start, end))
        .unwrap_or(0.0);
    let end_turn = source
        .points
        .get(segment_index + 2)
        .map(|next| signed_turn_strength(start, end, *next))
        .unwrap_or(0.0);

    (start_turn + end_turn).clamp(-1.0, 1.0)
}

fn river_bend_bar_hints(
    position: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    bend_strength: f32,
    core_strength: f32,
    shoulder_strength: f32,
    flow_hint: f32,
) -> (f32, f32) {
    let bend = bend_strength.clamp(-1.0, 1.0);
    if bend.abs() <= 0.08 {
        return (0.0, 0.0);
    }
    let side = signed_side(position, start, end);
    if side.abs() <= f32::EPSILON {
        return (0.0, 0.0);
    }

    let flow_t = smoothstep01(flow_hint.clamp(0.0, 1.0));
    let bend_t = smoothstep01((bend.abs() - 0.08) / 0.72);
    let active_strength = core_strength.max(shoulder_strength * 0.72).clamp(0.0, 1.0);
    let inside_bend = (side * bend) < 0.0;
    if inside_bend {
        (active_strength * bend_t * lerp(0.18, 0.34, flow_t), 0.0)
    } else {
        (0.0, active_strength * bend_t * lerp(0.24, 0.44, flow_t))
    }
}

fn signed_turn_strength(a: WorldPlanePoint, b: WorldPlanePoint, c: WorldPlanePoint) -> f32 {
    let ab_x = b.x - a.x;
    let ab_z = b.z - a.z;
    let bc_x = c.x - b.x;
    let bc_z = c.z - b.z;
    let ab_len = (ab_x * ab_x + ab_z * ab_z).sqrt();
    let bc_len = (bc_x * bc_x + bc_z * bc_z).sqrt();
    let denom = ab_len * bc_len;
    if denom <= f32::EPSILON {
        0.0
    } else {
        ((ab_x * bc_z - ab_z * bc_x) / denom).clamp(-1.0, 1.0)
    }
}

pub(super) fn rounded_river_raster_points(
    points: &[WorldPlanePoint],
    flow_hint: f32,
    water_width_blocks: f32,
    valley_width_blocks: f32,
    configured_radius_blocks: f32,
) -> Vec<WorldPlanePoint> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let water_radius =
        river_water_radius_blocks(flow_hint, water_width_blocks, configured_radius_blocks);
    let valley_radius =
        river_shoulder_radius_blocks(flow_hint, valley_width_blocks, configured_radius_blocks);
    let corner_cut_blocks = (water_radius * 1.2)
        .max(valley_radius * 0.18)
        .clamp(2.0, configured_radius_blocks * 0.16);
    let mut rounded = Vec::with_capacity(points.len() * 2);
    rounded.push(points[0]);

    for window in points.windows(3) {
        let previous = window[0];
        let corner = window[1];
        let next = window[2];
        let incoming = distance_between_points(previous, corner);
        let outgoing = distance_between_points(corner, next);
        let cut = corner_cut_blocks.min(incoming * 0.42).min(outgoing * 0.42);
        if cut <= f32::EPSILON {
            rounded.push(corner);
            continue;
        }

        let before = lerp_world_point(corner, previous, cut / incoming.max(f32::EPSILON));
        let after = lerp_world_point(corner, next, cut / outgoing.max(f32::EPSILON));
        if rounded
            .last()
            .is_none_or(|last| squared_distance(*last, before) > 0.001)
        {
            rounded.push(before);
        }
        rounded.push(after);
    }

    if let Some(last) = points.last().copied() {
        if rounded
            .last()
            .is_none_or(|point| squared_distance(*point, last) > 0.001)
        {
            rounded.push(last);
        }
    }

    smooth_river_raster_centerline(rounded, flow_hint, corner_cut_blocks)
}

pub(super) fn distance_between_points(left: WorldPlanePoint, right: WorldPlanePoint) -> f32 {
    squared_distance(left, right).sqrt()
}

pub(super) fn lerp_world_point(
    from: WorldPlanePoint,
    to: WorldPlanePoint,
    t: f32,
) -> WorldPlanePoint {
    let t = t.clamp(0.0, 1.0);
    WorldPlanePoint::new(lerp(from.x, to.x, t), lerp(from.z, to.z, t))
}

pub(super) fn smooth_river_raster_centerline(
    mut points: Vec<WorldPlanePoint>,
    flow_hint: f32,
    corner_cut_blocks: f32,
) -> Vec<WorldPlanePoint> {
    if points.len() <= 3 {
        return points;
    }

    let flow_t = smoothstep01(flow_hint.clamp(0.0, 1.0));
    let passes = if flow_t > 0.45 { 2 } else { 1 };
    let smooth_amount = lerp(0.20, 0.48, flow_t);
    let max_step_blocks = (corner_cut_blocks * lerp(0.28, 0.62, flow_t)).max(1.0);

    for _ in 0..passes {
        let previous = points.clone();
        for index in 1..points.len() - 1 {
            let target = midpoint(previous[index - 1], previous[index + 1]);
            let candidate = lerp_world_point(previous[index], target, smooth_amount);
            points[index] = clamp_point_displacement(previous[index], candidate, max_step_blocks);
        }
    }

    points
}

pub(super) fn midpoint(a: WorldPlanePoint, b: WorldPlanePoint) -> WorldPlanePoint {
    WorldPlanePoint::new((a.x + b.x) * 0.5, (a.z + b.z) * 0.5)
}

pub(super) fn clamp_point_displacement(
    origin: WorldPlanePoint,
    candidate: WorldPlanePoint,
    max_distance_blocks: f32,
) -> WorldPlanePoint {
    let dx = candidate.x - origin.x;
    let dz = candidate.z - origin.z;
    let distance = (dx * dx + dz * dz).sqrt();
    if distance <= max_distance_blocks || distance <= f32::EPSILON {
        candidate
    } else {
        let scale = max_distance_blocks / distance;
        WorldPlanePoint::new(origin.x + dx * scale, origin.z + dz * scale)
    }
}

pub(super) fn assign_river_raster_components(sources: &mut [RiverRasterWorkSource]) {
    let mut parent = (0..sources.len()).collect::<Vec<_>>();
    for left in 0..sources.len() {
        for right in left + 1..sources.len() {
            if mark_open_caps_for_touching_sources(sources, left, right) {
                union_component(&mut parent, left, right);
            }
        }
    }

    let mut component_by_root = HashMap::new();
    let mut next_component = 0;
    for index in 0..sources.len() {
        let root = find_component(&mut parent, index);
        let component = *component_by_root.entry(root).or_insert_with(|| {
            let component = next_component;
            next_component += 1;
            component
        });
        sources[index].component_id = component;
    }
}

fn mark_open_caps_for_touching_sources(
    sources: &mut [RiverRasterWorkSource],
    left: usize,
    right: usize,
) -> bool {
    let left_start = sources[left].points.first().copied();
    let left_end = sources[left].points.last().copied();
    let right_start = sources[right].points.first().copied();
    let right_end = sources[right].points.last().copied();
    let threshold_blocks = endpoint_touch_threshold_blocks(&sources[left], &sources[right]);
    let threshold = threshold_blocks * threshold_blocks;
    let mut touches = false;

    if endpoints_touch(left_start, right_start, threshold) {
        sources[left].open_start_cap = true;
        sources[right].open_start_cap = true;
        touches = true;
    }
    if endpoints_touch(left_start, right_end, threshold) {
        sources[left].open_start_cap = true;
        sources[right].open_end_cap = true;
        touches = true;
    }
    if endpoints_touch(left_end, right_start, threshold) {
        sources[left].open_end_cap = true;
        sources[right].open_start_cap = true;
        touches = true;
    }
    if endpoints_touch(left_end, right_end, threshold) {
        sources[left].open_end_cap = true;
        sources[right].open_end_cap = true;
        touches = true;
    }

    touches
}

fn endpoint_touch_threshold_blocks(
    left: &RiverRasterWorkSource,
    right: &RiverRasterWorkSource,
) -> f32 {
    (left.water_width_blocks.min(right.water_width_blocks) * 0.20).clamp(0.75, 12.0)
}

fn endpoints_touch(
    left: Option<WorldPlanePoint>,
    right: Option<WorldPlanePoint>,
    threshold_squared: f32,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => squared_distance(left, right) <= threshold_squared,
        _ => false,
    }
}

pub(super) fn union_component(parent: &mut [usize], left: usize, right: usize) {
    let left_root = find_component(parent, left);
    let right_root = find_component(parent, right);
    if left_root != right_root {
        parent[right_root] = left_root;
    }
}

pub(super) fn find_component(parent: &mut [usize], index: usize) -> usize {
    if parent[index] != index {
        let parent_index = parent[index];
        parent[index] = find_component(parent, parent_index);
    }
    parent[index]
}

pub(super) fn rasterize_curve_distance_field(
    sources: &[(&NoisyBoundaryCurve, f32)],
    config: MacroFieldTileConfig,
    radius_blocks: f32,
) -> RasterDistanceField {
    let sample_count = config.sample_count();
    if sources.is_empty() {
        return RasterDistanceField {
            distance_blocks: vec![f32::INFINITY; sample_count],
            river_core_strength: vec![0.0; sample_count],
            river_shoulder_strength: vec![0.0; sample_count],
            river_valley_strength: vec![0.0; sample_count],
            river_centerline_x: vec![f32::NAN; sample_count],
            river_centerline_z: vec![f32::NAN; sample_count],
            river_longitudinal_blocks: vec![f32::NAN; sample_count],
            flow_hint: vec![0.0; sample_count],
            river_core_depth_hint: vec![0.0; sample_count],
            river_bank_roughness_hint: vec![0.0; sample_count],
            river_gravel_hint: vec![0.0; sample_count],
            river_cutbank_hint: vec![0.0; sample_count],
            source_pixel_count: 0,
        };
    }

    let margin = ((radius_blocks / config.sample_spacing_blocks).ceil() as usize).saturating_add(2);
    let width = config.width as usize;
    let height = config.height as usize;
    let ext_width = width + margin * 2;
    let ext_height = height + margin * 2;
    let ext_len = ext_width * ext_height;
    let ext_origin = WorldPlanePoint::new(
        config.origin.x - margin as f32 * config.sample_spacing_blocks,
        config.origin.z - margin as f32 * config.sample_spacing_blocks,
    );
    let mut distance = vec![f32::INFINITY; ext_len];
    let mut flow_hint = vec![0.0; ext_len];

    for (curve, strength) in sources {
        rasterize_curve_sources(
            &mut distance,
            &mut flow_hint,
            ext_width,
            ext_height,
            ext_origin,
            config.sample_spacing_blocks,
            curve,
            *strength,
        );
    }

    let source_pixel_count = distance.iter().filter(|distance| **distance == 0.0).count();
    propagate_chamfer_distance(
        &mut distance,
        &mut flow_hint,
        ext_width,
        ext_height,
        config.sample_spacing_blocks,
        radius_blocks,
    );

    let mut cropped_distance = Vec::with_capacity(sample_count);
    let mut cropped_flow = Vec::with_capacity(sample_count);
    for z in 0..height {
        let ext_row = (z + margin) * ext_width;
        for x in 0..width {
            let index = ext_row + x + margin;
            cropped_distance.push(distance[index]);
            cropped_flow.push(flow_hint[index]);
        }
    }

    RasterDistanceField {
        distance_blocks: cropped_distance,
        river_core_strength: vec![0.0; sample_count],
        river_shoulder_strength: vec![0.0; sample_count],
        river_valley_strength: vec![0.0; sample_count],
        river_centerline_x: vec![f32::NAN; sample_count],
        river_centerline_z: vec![f32::NAN; sample_count],
        river_longitudinal_blocks: vec![f32::NAN; sample_count],
        flow_hint: cropped_flow,
        river_core_depth_hint: vec![0.0; sample_count],
        river_bank_roughness_hint: vec![0.0; sample_count],
        river_gravel_hint: vec![0.0; sample_count],
        river_cutbank_hint: vec![0.0; sample_count],
        source_pixel_count,
    }
}

pub(super) fn rasterize_curve_anti_aliased_polyline_field(
    sources: &[RiverRasterSource<'_>],
    config: MacroFieldTileConfig,
    radius_blocks: f32,
) -> RasterDistanceField {
    let sample_count = config.sample_count();
    if sources.is_empty() {
        return RasterDistanceField {
            distance_blocks: vec![f32::INFINITY; sample_count],
            river_core_strength: vec![0.0; sample_count],
            river_shoulder_strength: vec![0.0; sample_count],
            river_valley_strength: vec![0.0; sample_count],
            river_centerline_x: vec![f32::NAN; sample_count],
            river_centerline_z: vec![f32::NAN; sample_count],
            river_longitudinal_blocks: vec![f32::NAN; sample_count],
            flow_hint: vec![0.0; sample_count],
            river_core_depth_hint: vec![0.0; sample_count],
            river_bank_roughness_hint: vec![0.0; sample_count],
            river_gravel_hint: vec![0.0; sample_count],
            river_cutbank_hint: vec![0.0; sample_count],
            source_pixel_count: 0,
        };
    }

    let mut sources = rounded_river_raster_sources(sources, radius_blocks);
    assign_river_raster_components(&mut sources);

    let width = config.width as usize;
    let height = config.height as usize;
    let rows = (0..height)
        .into_par_iter()
        .map(|z| rasterize_river_row(&sources, width, height, config, radius_blocks, z))
        .collect::<Vec<_>>();
    let source_pixel_count = rows.iter().map(|row| row.source_pixel_count).sum();
    let mut distance_blocks = Vec::with_capacity(sample_count);
    let mut river_core_strength = Vec::with_capacity(sample_count);
    let mut river_shoulder_strength = Vec::with_capacity(sample_count);
    let mut river_valley_strength = Vec::with_capacity(sample_count);
    let mut river_centerline_x = Vec::with_capacity(sample_count);
    let mut river_centerline_z = Vec::with_capacity(sample_count);
    let mut river_longitudinal_blocks_raw = Vec::with_capacity(sample_count);
    let mut longitudinal_weighted_sum = Vec::with_capacity(sample_count);
    let mut longitudinal_weight_sum = Vec::with_capacity(sample_count);
    let mut flow_weighted_sum = Vec::with_capacity(sample_count);
    let mut flow_weight_sum = Vec::with_capacity(sample_count);
    let mut river_core_depth_hint = Vec::with_capacity(sample_count);
    let mut river_bank_roughness_hint = Vec::with_capacity(sample_count);
    let mut river_gravel_hint = Vec::with_capacity(sample_count);
    let mut river_cutbank_hint = Vec::with_capacity(sample_count);
    for row in rows {
        distance_blocks.extend(row.distance_blocks);
        river_core_strength.extend(row.river_core_strength);
        river_shoulder_strength.extend(row.river_shoulder_strength);
        river_valley_strength.extend(row.river_valley_strength);
        river_centerline_x.extend(row.river_centerline_x);
        river_centerline_z.extend(row.river_centerline_z);
        river_longitudinal_blocks_raw.extend(row.river_longitudinal_blocks);
        longitudinal_weighted_sum.extend(row.longitudinal_weighted_sum);
        longitudinal_weight_sum.extend(row.longitudinal_weight_sum);
        flow_weighted_sum.extend(row.flow_weighted_sum);
        flow_weight_sum.extend(row.flow_weight_sum);
        river_core_depth_hint.extend(row.river_core_depth_hint);
        river_bank_roughness_hint.extend(row.river_bank_roughness_hint);
        river_gravel_hint.extend(row.river_gravel_hint);
        river_cutbank_hint.extend(row.river_cutbank_hint);
    }
    let flow_hint = flow_weighted_sum
        .into_iter()
        .zip(flow_weight_sum)
        .map(|(sum, weight)| {
            if weight > f32::EPSILON {
                (sum / weight).clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .collect();
    let river_longitudinal_blocks = river_longitudinal_blocks_raw
        .into_iter()
        .zip(longitudinal_weighted_sum)
        .zip(longitudinal_weight_sum)
        .map(|((nearest, sum), weight)| {
            if weight > f32::EPSILON {
                sum / weight
            } else {
                nearest
            }
        })
        .collect();
    let mut field = RasterDistanceField {
        distance_blocks,
        river_core_strength,
        river_shoulder_strength,
        river_valley_strength,
        river_centerline_x,
        river_centerline_z,
        river_longitudinal_blocks,
        flow_hint,
        river_core_depth_hint,
        river_bank_roughness_hint,
        river_gravel_hint,
        river_cutbank_hint,
        source_pixel_count,
    };
    close_river_shoulder_concave_bays(&mut field, width, height, config.sample_spacing_blocks);
    field
}

pub(super) fn rasterize_river_row(
    sources: &[RiverRasterWorkSource],
    width: usize,
    height: usize,
    config: MacroFieldTileConfig,
    radius_blocks: f32,
    z: usize,
) -> RiverRasterRow {
    let mut row = RiverRasterRow::new(width);
    let mut river_owner_component = vec![usize::MAX; width];
    for source in sources {
        for (segment_index, segment) in source.points.windows(2).enumerate() {
            rasterize_segment_anti_aliased_stroke_row(
                &mut row,
                &mut river_owner_component,
                width,
                height,
                z,
                config,
                segment[0],
                segment[1],
                segment_index,
                radius_blocks,
                source,
            );
        }
    }
    row.source_pixel_count = row
        .river_valley_strength
        .iter()
        .filter(|strength| **strength > 0.001)
        .count();
    resolve_river_row_weighted_hints(&mut row);
    row
}

pub(super) fn resolve_river_row_weighted_hints(row: &mut RiverRasterRow) {
    for x in 0..row.distance_blocks.len() {
        let centerline_weight = row.centerline_weight_sum[x];
        if centerline_weight > f32::EPSILON {
            row.river_centerline_x[x] = row.centerline_weighted_x_sum[x] / centerline_weight;
            row.river_centerline_z[x] = row.centerline_weighted_z_sum[x] / centerline_weight;
        }
        let longitudinal_weight = row.longitudinal_weight_sum[x];
        if longitudinal_weight > f32::EPSILON {
            row.river_longitudinal_blocks[x] =
                row.longitudinal_weighted_sum[x] / longitudinal_weight;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rasterize_segment_anti_aliased_stroke_row(
    row: &mut RiverRasterRow,
    river_owner_component: &mut [usize],
    width: usize,
    height: usize,
    z: usize,
    config: MacroFieldTileConfig,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    segment_index: usize,
    radius_blocks: f32,
    source: &RiverRasterWorkSource,
) {
    let spacing = config.sample_spacing_blocks;
    let aa_margin = spacing * 0.75;
    let active_radius_blocks =
        river_raster_active_radius_blocks(source, radius_blocks, config.boundary_roughness_blocks);
    let min_x = ((start.x.min(end.x) - active_radius_blocks - aa_margin - config.origin.x)
        / spacing)
        .floor()
        .max(0.0) as usize;
    let max_x = ((start.x.max(end.x) + active_radius_blocks + aa_margin - config.origin.x)
        / spacing)
        .ceil()
        .min((width.saturating_sub(1)) as f32) as usize;
    let min_z = ((start.z.min(end.z) - active_radius_blocks - aa_margin - config.origin.z)
        / spacing)
        .floor()
        .max(0.0) as usize;
    let max_z = ((start.z.max(end.z) + active_radius_blocks + aa_margin - config.origin.z)
        / spacing)
        .ceil()
        .min((height.saturating_sub(1)) as f32) as usize;
    if min_x > max_x || min_z > max_z {
        return;
    }
    if z < min_z || z > max_z {
        return;
    }

    let subpixel_offsets = [
        (0.0, 0.0),
        (-0.35, -0.35),
        (0.35, -0.35),
        (-0.35, 0.35),
        (0.35, 0.35),
    ];
    let subpixel_count = subpixel_offsets.len() as f32;
    for x in min_x..=max_x {
        let global_index = z * width + x;
        let position = config.sample_position(global_index);
        let Some((distance, centerline_t)) =
            river_segment_distance_and_t(source, segment_index, position, start, end)
        else {
            continue;
        };
        if distance > active_radius_blocks + aa_margin {
            continue;
        }
        let centerline_position = nearest_point_on_segment(position, start, end);
        let longitudinal_blocks = segment_longitudinal_blocks(source, segment_index, centerline_t);
        let bend_strength = river_segment_bend_strength(source, segment_index);
        let mut profile_sum = 0.0;
        let mut core_sum = 0.0;
        let mut shoulder_sum = 0.0;
        let mut closest_subpixel_distance = distance;
        let mut bed_sum = 0.0;
        let mut rough_sum = 0.0;
        let mut gravel_sum = 0.0;
        let mut cutbank_sum = 0.0;
        let mut flow_sum = 0.0;
        let mut flow_sample_count = 0.0;
        for (offset_x, offset_z) in subpixel_offsets {
            let subpixel = WorldPlanePoint::new(
                position.x + offset_x * spacing,
                position.z + offset_z * spacing,
            );
            let Some((subpixel_distance, subpixel_t)) =
                river_segment_distance_and_t(source, segment_index, subpixel, start, end)
            else {
                continue;
            };
            let mouth_progress = terminal_mouth_progress(source, segment_index, subpixel_t);
            let effective = effective_river_morphology(source, mouth_progress);
            let profile_distance = subpixel_distance;
            let roughness_offset = river_boundary_roughness_offset(
                position,
                effective.flow_hint,
                effective.water_width_blocks,
                radius_blocks,
                config.boundary_roughness_blocks,
            );
            let terminal_taper =
                terminal_outlet_river_taper(source, segment_index, subpixel_t, radius_blocks);
            let terminal_taper = lerp(
                terminal_taper,
                1.0,
                mouth_progress * TERMINAL_MOUTH_TAPER_BLEND,
            )
            .clamp(0.0, 1.0);
            let core_strength = river_core_strength_for_roughened_distance(
                profile_distance,
                roughness_offset,
                effective.flow_hint,
                effective.water_width_blocks,
                radius_blocks,
            ) * terminal_taper;
            let shoulder_strength = river_valley_strength_for_roughened_distance(
                profile_distance,
                roughness_offset,
                effective.flow_hint,
                effective.water_width_blocks,
                effective.valley_width_blocks,
                radius_blocks,
            ) * terminal_taper;
            let valley_strength = core_strength.max(shoulder_strength);
            let hints = river_hints_from_strength(
                core_strength,
                effective.flow_hint,
                effective.bed_depth_blocks,
            );
            let (gravel_bend, cutbank_bend) = river_bend_bar_hints(
                subpixel,
                start,
                end,
                bend_strength,
                core_strength,
                shoulder_strength,
                effective.flow_hint,
            );
            closest_subpixel_distance = closest_subpixel_distance.min(subpixel_distance);
            core_sum += core_strength;
            shoulder_sum += shoulder_strength;
            profile_sum += valley_strength;
            bed_sum += hints.bed_depth_hint;
            rough_sum += hints.bank_roughness_hint;
            gravel_sum += hints.gravel_hint.max(gravel_bend);
            cutbank_sum += hints.cutbank_hint.max(cutbank_bend);
            flow_sum += effective.flow_hint;
            flow_sample_count += 1.0;
        }
        let anti_aliased_strength = (profile_sum / subpixel_count).clamp(0.0, 1.0);
        if anti_aliased_strength <= 0.0 {
            continue;
        }

        let core_strength = (core_sum / subpixel_count).clamp(0.0, 1.0);
        let shoulder_strength = (shoulder_sum / subpixel_count).clamp(0.0, 1.0);
        let bed_hint = (bed_sum / subpixel_count).clamp(0.0, 1.0);
        let rough_hint = (rough_sum / subpixel_count).clamp(0.0, 1.0);
        let gravel_hint = (gravel_sum / subpixel_count).clamp(0.0, 1.0);
        let cutbank_hint = (cutbank_sum / subpixel_count).clamp(0.0, 1.0);
        let flow_hint = if flow_sample_count > f32::EPSILON {
            (flow_sum / flow_sample_count).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let current_distance = row.distance_blocks[x];
        let same_centerline_band = spacing * 0.35;
        let current_component = river_owner_component[x];
        if current_component == source.component_id && current_distance.is_finite() {
            row.distance_blocks[x] = row.distance_blocks[x].min(closest_subpixel_distance);
            row.river_core_strength[x] =
                component_union_strength(row.river_core_strength[x], core_strength);
            row.river_shoulder_strength[x] =
                component_union_strength(row.river_shoulder_strength[x], shoulder_strength);
            row.river_valley_strength[x] =
                component_union_strength(row.river_valley_strength[x], anti_aliased_strength);
            row.river_core_depth_hint[x] = row.river_core_depth_hint[x].max(bed_hint);
            row.river_bank_roughness_hint[x] = row.river_bank_roughness_hint[x].max(rough_hint);
            row.river_gravel_hint[x] = row.river_gravel_hint[x].max(gravel_hint);
            row.river_cutbank_hint[x] = row.river_cutbank_hint[x].max(cutbank_hint);
            accumulate_river_row_hints(
                row,
                x,
                centerline_position,
                longitudinal_blocks,
                anti_aliased_strength,
            );
            row.flow_weighted_sum[x] += flow_hint * anti_aliased_strength;
            row.flow_weight_sum[x] += anti_aliased_strength;
        } else if closest_subpixel_distance + same_centerline_band < current_distance {
            row.distance_blocks[x] = closest_subpixel_distance;
            row.river_core_strength[x] = core_strength;
            row.river_shoulder_strength[x] = shoulder_strength;
            row.river_valley_strength[x] = anti_aliased_strength;
            row.river_centerline_x[x] = centerline_position.x;
            row.river_centerline_z[x] = centerline_position.z;
            row.river_longitudinal_blocks[x] = longitudinal_blocks;
            river_owner_component[x] = source.component_id;
            reset_river_row_hints(
                row,
                x,
                centerline_position,
                longitudinal_blocks,
                anti_aliased_strength,
            );
            row.river_core_depth_hint[x] = bed_hint;
            row.river_bank_roughness_hint[x] = rough_hint;
            row.river_gravel_hint[x] = gravel_hint;
            row.river_cutbank_hint[x] = cutbank_hint;
            row.flow_weighted_sum[x] = flow_hint * anti_aliased_strength;
            row.flow_weight_sum[x] = anti_aliased_strength;
        }
    }
}

fn terminal_outlet_endpoint_clips_sample(
    source: &RiverRasterWorkSource,
    segment_index: usize,
    sample: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> bool {
    if !source.is_terminal_outlet || segment_index + 2 != source.points.len() {
        return false;
    }

    projected_t_unclamped(sample, start, end) > 1.0
}

fn river_segment_distance_and_t(
    source: &RiverRasterWorkSource,
    segment_index: usize,
    sample: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> Option<(f32, f32)> {
    let raw_t = projected_t_unclamped(sample, start, end);
    let open_start = segment_index == 0 && source.open_start_cap;
    let open_end = segment_index + 2 == source.points.len() && source.open_end_cap;
    if (raw_t < 0.0 && open_start)
        || (raw_t > 1.0 && open_end)
        || terminal_outlet_endpoint_clips_sample(source, segment_index, sample, start, end)
    {
        return None;
    }

    let clamped_t = raw_t.clamp(0.0, 1.0);
    let distance = if (0.0..=1.0).contains(&raw_t) {
        distance_to_unclamped_line(sample, start, end)
    } else {
        point_segment_distance(sample, start, end)
    };
    Some((distance, clamped_t))
}

fn distance_to_unclamped_line(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    let len = (dx * dx + dz * dz).sqrt();
    if len <= f32::EPSILON {
        return ((point.x - start.x).powi(2) + (point.z - start.z).powi(2)).sqrt();
    }

    signed_side(point, start, end).abs() / len
}

fn projected_t_unclamped(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    let len2 = dx * dx + dz * dz;
    if len2 <= f32::EPSILON {
        return 0.0;
    }

    ((point.x - start.x) * dx + (point.z - start.z) * dz) / len2
}

fn terminal_outlet_river_taper(
    source: &RiverRasterWorkSource,
    segment_index: usize,
    segment_t: f32,
    configured_radius_blocks: f32,
) -> f32 {
    if !source.is_terminal_outlet {
        return 1.0;
    }

    let total_length = source.cumulative_lengths.last().copied().unwrap_or(0.0);
    let start = source
        .cumulative_lengths
        .get(segment_index)
        .copied()
        .unwrap_or(0.0);
    let end = source
        .cumulative_lengths
        .get(segment_index + 1)
        .copied()
        .unwrap_or(start);
    let along_source = lerp(start, end, segment_t.clamp(0.0, 1.0));
    let remaining = (total_length - along_source).max(0.0);
    let fade_blocks = (source.water_width_blocks * TERMINAL_RIVER_TAPER_WIDTH_SCALE)
        .max(river_water_radius_blocks(
            source.flow_hint,
            source.water_width_blocks,
            configured_radius_blocks,
        ))
        .max(1.0);

    let taper = smoothstep01((remaining / fade_blocks).clamp(0.0, 1.0));
    taper * taper
}

pub(super) fn accumulate_river_row_hints(
    row: &mut RiverRasterRow,
    x: usize,
    centerline_position: WorldPlanePoint,
    longitudinal_blocks: f32,
    weight: f32,
) {
    let weight = weight.clamp(0.0, 1.0);
    if weight <= f32::EPSILON {
        return;
    }
    row.centerline_weighted_x_sum[x] += centerline_position.x * weight;
    row.centerline_weighted_z_sum[x] += centerline_position.z * weight;
    row.centerline_weight_sum[x] += weight;
    if longitudinal_blocks.is_finite() {
        row.longitudinal_weighted_sum[x] += longitudinal_blocks * weight;
        row.longitudinal_weight_sum[x] += weight;
    }
}

pub(super) fn reset_river_row_hints(
    row: &mut RiverRasterRow,
    x: usize,
    centerline_position: WorldPlanePoint,
    longitudinal_blocks: f32,
    weight: f32,
) {
    row.centerline_weighted_x_sum[x] = 0.0;
    row.centerline_weighted_z_sum[x] = 0.0;
    row.centerline_weight_sum[x] = 0.0;
    row.longitudinal_weighted_sum[x] = 0.0;
    row.longitudinal_weight_sum[x] = 0.0;
    accumulate_river_row_hints(row, x, centerline_position, longitudinal_blocks, weight);
}

pub(super) fn component_union_strength(existing: f32, incoming: f32) -> f32 {
    let existing = existing.clamp(0.0, 1.0);
    let incoming = incoming.clamp(0.0, 1.0);
    existing.max(incoming)
}

pub(super) trait RiverRasterRadiusSource {
    fn flow_hint(&self) -> f32;
    fn water_width_blocks(&self) -> f32;
    fn valley_width_blocks(&self) -> f32;
}

impl RiverRasterRadiusSource for &RiverRasterWorkSource {
    fn flow_hint(&self) -> f32 {
        terminal_mouth_max_flow_hint(self)
    }

    fn water_width_blocks(&self) -> f32 {
        terminal_mouth_max_water_width_blocks(self)
    }

    fn valley_width_blocks(&self) -> f32 {
        terminal_mouth_max_valley_width_blocks(self)
    }
}

impl RiverRasterRadiusSource for RiverRasterSource<'_> {
    fn flow_hint(&self) -> f32 {
        let mouth = self.terminal_mouth_factor.clamp(0.0, 1.0);
        (self.flow_hint + (1.0 - self.flow_hint) * TERMINAL_MOUTH_FLOW_BOOST * mouth)
            .clamp(0.0, 1.0)
    }

    fn water_width_blocks(&self) -> f32 {
        self.water_width_blocks
            * (1.0 + TERMINAL_MOUTH_WATER_WIDTH_BOOST * self.terminal_mouth_factor.clamp(0.0, 1.0))
    }

    fn valley_width_blocks(&self) -> f32 {
        self.valley_width_blocks
            * (1.0 + TERMINAL_MOUTH_VALLEY_WIDTH_BOOST * self.terminal_mouth_factor.clamp(0.0, 1.0))
    }
}

pub(super) fn river_raster_active_radius_blocks(
    source: impl RiverRasterRadiusSource,
    configured_radius_blocks: f32,
    boundary_roughness_blocks: f32,
) -> f32 {
    let water_radius = river_water_radius_blocks(
        source.flow_hint(),
        source.water_width_blocks(),
        configured_radius_blocks,
    );
    let valley_radius = river_shoulder_radius_blocks(
        source.flow_hint(),
        source.valley_width_blocks(),
        configured_radius_blocks,
    )
    .max(water_radius + 1.0);
    let roughness_blocks = river_boundary_roughness_blocks(
        source.flow_hint(),
        source.water_width_blocks(),
        configured_radius_blocks,
        boundary_roughness_blocks,
    );

    valley_radius
        .max(water_radius + roughness_blocks)
        .clamp(1.0, configured_radius_blocks)
}

pub(super) fn rasterize_curve_sources(
    distance: &mut [f32],
    flow_hint: &mut [f32],
    width: usize,
    height: usize,
    origin: WorldPlanePoint,
    spacing: f32,
    curve: &NoisyBoundaryCurve,
    strength: f32,
) {
    for segment in curve.points.windows(2) {
        rasterize_segment_sources(
            distance, flow_hint, width, height, origin, spacing, segment[0], segment[1], strength,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rasterize_segment_sources(
    distance: &mut [f32],
    flow_hint: &mut [f32],
    width: usize,
    height: usize,
    origin: WorldPlanePoint,
    spacing: f32,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    strength: f32,
) {
    let start_x = (start.x - origin.x) / spacing;
    let start_z = (start.z - origin.z) / spacing;
    let end_x = (end.x - origin.x) / spacing;
    let end_z = (end.z - origin.z) / spacing;
    let steps = ((end_x - start_x).abs().max((end_z - start_z).abs()) * 2.0)
        .ceil()
        .max(1.0) as usize;

    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = (start_x + (end_x - start_x) * t).round() as isize;
        let z = (start_z + (end_z - start_z) * t).round() as isize;
        if x < 0 || z < 0 || x >= width as isize || z >= height as isize {
            continue;
        }
        let index = z as usize * width + x as usize;
        distance[index] = 0.0;
        flow_hint[index] = flow_hint[index].max(strength);
    }
}

pub(super) fn propagate_chamfer_distance(
    distance: &mut [f32],
    flow_hint: &mut [f32],
    width: usize,
    height: usize,
    spacing: f32,
    radius_blocks: f32,
) {
    if width == 0 || height == 0 {
        return;
    }

    let diagonal = spacing * std::f32::consts::SQRT_2;
    let limit = radius_blocks + diagonal * 2.0;
    for _ in 0..2 {
        for z in 0..height {
            for x in 0..width {
                let index = z * width + x;
                update_from_neighbor(
                    distance, flow_hint, index, x, z, -1, 0, spacing, width, height, limit,
                );
                update_from_neighbor(
                    distance, flow_hint, index, x, z, 0, -1, spacing, width, height, limit,
                );
                update_from_neighbor(
                    distance, flow_hint, index, x, z, -1, -1, diagonal, width, height, limit,
                );
                update_from_neighbor(
                    distance, flow_hint, index, x, z, 1, -1, diagonal, width, height, limit,
                );
            }
        }
        for z in (0..height).rev() {
            for x in (0..width).rev() {
                let index = z * width + x;
                update_from_neighbor(
                    distance, flow_hint, index, x, z, 1, 0, spacing, width, height, limit,
                );
                update_from_neighbor(
                    distance, flow_hint, index, x, z, 0, 1, spacing, width, height, limit,
                );
                update_from_neighbor(
                    distance, flow_hint, index, x, z, 1, 1, diagonal, width, height, limit,
                );
                update_from_neighbor(
                    distance, flow_hint, index, x, z, -1, 1, diagonal, width, height, limit,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update_from_neighbor(
    distance: &mut [f32],
    flow_hint: &mut [f32],
    index: usize,
    x: usize,
    z: usize,
    dx: isize,
    dz: isize,
    cost: f32,
    width: usize,
    height: usize,
    limit: f32,
) {
    let nx = x as isize + dx;
    let nz = z as isize + dz;
    if nx < 0 || nz < 0 || nx >= width as isize || nz >= height as isize {
        return;
    }
    let neighbor = nz as usize * width + nx as usize;
    if neighbor >= distance.len() {
        return;
    }
    let candidate = distance[neighbor] + cost;
    if candidate > limit {
        return;
    }
    if candidate + 0.001 < distance[index] {
        distance[index] = candidate;
        flow_hint[index] = flow_hint[neighbor];
    } else if (candidate - distance[index]).abs() <= 0.001 {
        flow_hint[index] = flow_hint[index].max(flow_hint[neighbor]);
    }
}
pub(super) fn macro_field_stats(
    samples: &[MacroFieldSample],
    influence_stats: MacroFieldInfluenceStats,
) -> MacroFieldTileStats {
    if samples.is_empty() {
        return MacroFieldTileStats::default();
    }

    let mut stats = MacroFieldTileStats {
        sample_count: samples.len(),
        min_combined_macro_height: f32::INFINITY,
        max_combined_macro_height: f32::NEG_INFINITY,
        min_dry_basin_height: f32::INFINITY,
        max_dry_basin_height: f32::NEG_INFINITY,
        ridge_source_curve_count: influence_stats.ridge_source_curve_count,
        river_source_curve_count: influence_stats.river_source_curve_count,
        coast_source_curve_count: influence_stats.coast_source_curve_count,
        ridge_source_pixel_count: influence_stats.ridge_source_pixel_count,
        river_source_pixel_count: influence_stats.river_source_pixel_count,
        coast_source_pixel_count: influence_stats.coast_source_pixel_count,
        ..MacroFieldTileStats::default()
    };

    let mut ridge_sum = 0.0;
    let mut dry_height_sum = 0.0;
    for sample in samples {
        stats.min_combined_macro_height = stats
            .min_combined_macro_height
            .min(sample.combined_macro_height);
        stats.max_combined_macro_height = stats
            .max_combined_macro_height
            .max(sample.combined_macro_height);
        stats.max_ridge_influence = stats.max_ridge_influence.max(sample.ridge_influence);
        ridge_sum += sample.ridge_influence;
        if sample.ridge_influence > 0.0 {
            stats.ridge_active_sample_count += 1;
        }
        stats.max_river_core_strength = stats
            .max_river_core_strength
            .max(sample.river_core_strength);
        stats.max_river_shoulder_strength = stats
            .max_river_shoulder_strength
            .max(sample.river_shoulder_strength);
        stats.max_river_valley_strength = stats
            .max_river_valley_strength
            .max(sample.river_valley_strength);
        if sample.ocean_mask > 0.5 {
            stats.ocean_sample_count += 1;
        }
        if sample.lake_mask > 0.5 {
            stats.lake_sample_count += 1;
        }
        if sample.dry_basin_mask > 0.5 {
            stats.dry_basin_sample_count += 1;
            stats.min_dry_basin_height =
                stats.min_dry_basin_height.min(sample.combined_macro_height);
            stats.max_dry_basin_height =
                stats.max_dry_basin_height.max(sample.combined_macro_height);
            dry_height_sum += sample.combined_macro_height;
        }
    }
    stats.average_ridge_influence = ridge_sum / samples.len() as f32;
    if stats.dry_basin_sample_count > 0 {
        stats.average_dry_basin_height = dry_height_sum / stats.dry_basin_sample_count as f32;
    } else {
        stats.min_dry_basin_height = 0.0;
        stats.max_dry_basin_height = 0.0;
    }

    stats
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::world::generation::boundary::{
        BoundaryAnchors, BoundaryCache, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
    };
    use crate::world::generation::graph::{
        VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId, WorldPlanePoint,
    };
    use crate::world::generation::macro_field::test_support::*;
    use crate::world::generation::macro_field::{
        DEFAULT_MACRO_FIELD_RIDGE_RADIUS_BLOCKS, MacroFieldRasterContext, MacroFieldTileConfig,
        generate_macro_field_tile, sample_macro_field_point,
    };
    use crate::world::generation::macro_map::{
        GraphMacroMap, MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
    };
    use crate::world::generation::river_plan::{
        DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS, RiverPlan,
    };

    fn set_river_shoulder_strength(
        field: &mut RasterDistanceField,
        width: usize,
        x: usize,
        z: usize,
        strength: f32,
    ) {
        let index = z * width + x;
        field.river_shoulder_strength[index] = strength;
        field.river_valley_strength[index] = strength;
        field.flow_hint[index] = 0.72;
        field.distance_blocks[index] = 1.0;
        field.river_core_depth_hint[index] = 0.45;
        field.river_bank_roughness_hint[index] = 0.35;
        field.river_gravel_hint[index] = 0.25;
        field.river_centerline_x[index] = x as f32;
        field.river_centerline_z[index] = z as f32;
        field.river_longitudinal_blocks[index] = x as f32 + z as f32;
    }

    #[test]
    fn bend_bar_hints_separate_inside_gravel_from_outside_cutbank() {
        let source = RiverRasterWorkSource {
            is_terminal_outlet: false,
            edge: VoronoiEdgeId(7),
            points: vec![
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(64.0, 0.0),
                WorldPlanePoint::new(64.0, 64.0),
            ],
            cumulative_lengths: vec![0.0, 64.0, 128.0],
            open_start_cap: false,
            open_end_cap: false,
            longitudinal_start_blocks: 0.0,
            flow_hint: 0.72,
            water_width_blocks: 48.0,
            valley_width_blocks: 160.0,
            bed_depth_blocks: 12.0,
            terminal_mouth_factor: 0.0,
            component_id: 0,
        };
        let bend = river_segment_bend_strength(&source, 0);
        let (inside_gravel, inside_cutbank) = river_bend_bar_hints(
            WorldPlanePoint::new(32.0, 12.0),
            source.points[0],
            source.points[1],
            bend,
            0.95,
            0.80,
            source.flow_hint,
        );
        let (outside_gravel, outside_cutbank) = river_bend_bar_hints(
            WorldPlanePoint::new(32.0, -12.0),
            source.points[0],
            source.points[1],
            bend,
            0.95,
            0.80,
            source.flow_hint,
        );

        assert!(
            inside_gravel > inside_cutbank,
            "inside bend should bias toward gravel bar: gravel={inside_gravel} cutbank={inside_cutbank}"
        );
        assert!(
            outside_cutbank > outside_gravel,
            "outside bend should bias toward stronger cutbank carve: gravel={outside_gravel} cutbank={outside_cutbank}"
        );
    }

    #[test]
    fn ridge_influence_is_higher_near_ridge_curve_than_far_sample() {
        let inputs = test_inputs(42);
        let Some(ridge_curve) = inputs.boundary.curves.iter().find(|curve| {
            inputs.macro_map.edge(curve.edge).is_some_and(|edge| {
                edge.guide.is_ridge_candidate
                    && edge.guide.ridgeness >= RIDGE_FIELD_SOURCE_MIN_RIDGENESS
            })
        }) else {
            return;
        };
        let context = MacroFieldRasterContext::new(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
        );
        let config = test_tile_config();
        let near = ridge_curve.points[ridge_curve.points.len() / 2];
        let far = WorldPlanePoint::new(
            near.x + config.ridge_radius_blocks * 2.2,
            near.z + config.ridge_radius_blocks * 2.2,
        );
        let near_sample = sample_macro_field_point(&context, config, near);
        let far_sample = sample_macro_field_point(&context, config, far);

        assert!(
            near_sample.ridge_influence > far_sample.ridge_influence,
            "near={} far={}",
            near_sample.ridge_influence,
            far_sample.ridge_influence
        );
    }

    #[test]
    fn rasterized_ridge_influence_tile_has_near_stronger_than_far_sample() {
        let inputs = test_inputs(42);
        let Some(ridge_curve) = inputs.boundary.curves.iter().find(|curve| {
            inputs.macro_map.edge(curve.edge).is_some_and(|edge| {
                edge.guide.is_ridge_candidate
                    && edge.guide.ridgeness >= RIDGE_FIELD_SOURCE_MIN_RIDGENESS
            })
        }) else {
            return;
        };
        let near = ridge_curve.points[ridge_curve.points.len() / 2];
        let tile = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
            centered_test_tile_config(near),
        );
        let max = tile
            .samples
            .iter()
            .map(|sample| sample.ridge_influence)
            .fold(0.0, f32::max);
        let min = tile
            .samples
            .iter()
            .map(|sample| sample.ridge_influence)
            .fold(1.0, f32::min);

        assert!(tile.stats.ridge_source_curve_count > 0);
        assert!(tile.stats.ridge_source_pixel_count > 0);
        assert!(
            max > min,
            "splat ridge field should preserve a near/far gradient: max={max} min={min}"
        );
        let active_fraction =
            tile.stats.ridge_active_sample_count as f32 / tile.stats.sample_count as f32;
        assert!(
            active_fraction < 0.85,
            "a ridge-centered diagnostic tile may show a broad belt, but it should not be fully saturated: {active_fraction}"
        );
    }

    #[test]
    fn ridge_envelope_keeps_a_connected_mountain_belt_width() {
        let radius = DEFAULT_MACRO_FIELD_RIDGE_RADIUS_BLOCKS;
        let center = ridge_envelope(0.0, radius);
        let shoulder = ridge_envelope(radius * 0.45, radius);
        let far = ridge_envelope(radius * 1.25, radius);

        assert!(center > 0.95);
        assert!(
            shoulder > 0.05,
            "ridge envelope should keep a visible shoulder around the guide instead of a pinpoint: {shoulder}"
        );
        assert_eq!(
            far, 0.0,
            "ridge envelope should not become global low-level grain"
        );
    }

    #[test]
    fn river_valley_strength_is_higher_near_selected_river_than_far_sample() {
        let inputs = test_inputs(42);
        let Some(segment) = inputs.hydrology.segments.first() else {
            return;
        };
        let curve = inputs
            .boundary
            .curve_for_edge(segment.edge)
            .expect("selected river edge should have canonical boundary curve");
        let context = MacroFieldRasterContext::new(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
        );
        let config = test_tile_config();
        let near = curve.points[curve.points.len() / 2];
        let far = WorldPlanePoint::new(
            near.x + config.river_radius_blocks * 2.4,
            near.z + config.river_radius_blocks * 2.4,
        );
        let near_sample = sample_macro_field_point(&context, config, near);
        let far_sample = sample_macro_field_point(&context, config, far);

        assert!(
            near_sample.river_valley_strength > far_sample.river_valley_strength,
            "near={} far={}",
            near_sample.river_valley_strength,
            far_sample.river_valley_strength
        );
        assert!(
            near_sample.river_flow_hint > 0.0,
            "sample on a selected river curve should expose a positive flow hint"
        );
    }

    #[test]
    fn rasterized_river_field_has_near_stronger_than_far_sample() {
        let inputs = test_inputs(42);
        let Some(segment) = inputs.hydrology.segments.first() else {
            return;
        };
        let curve = inputs
            .boundary
            .curve_for_edge(segment.edge)
            .expect("selected river edge should have canonical boundary curve");
        let near = curve.points[curve.points.len() / 2];
        let tile = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
            centered_test_tile_config(near),
        );
        let max = tile
            .samples
            .iter()
            .map(|sample| sample.river_valley_strength)
            .fold(0.0, f32::max);
        let min = tile
            .samples
            .iter()
            .map(|sample| sample.river_valley_strength)
            .fold(1.0, f32::min);

        assert!(tile.stats.river_source_curve_count > 0);
        assert!(tile.stats.river_source_pixel_count > 0);
        assert!(
            max > min,
            "anti-aliased river field should preserve a near/far gradient: max={max} min={min}"
        );
    }

    #[test]
    fn river_raster_bounds_use_planned_valley_width_not_configured_search_cap() {
        let curve = test_noisy_curve(
            91,
            vec![
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(128.0, 0.0),
            ],
        );
        let mut source = test_river_source(&curve, 0.25);
        source.water_width_blocks = 6.0;
        source.valley_width_blocks = 18.0;
        let configured_radius = 640.0;
        let rounded_sources = rounded_river_raster_sources(&[source], configured_radius);
        let active_radius =
            river_raster_active_radius_blocks(&rounded_sources[0], configured_radius, 96.0);

        assert!(
            active_radius < configured_radius * 0.1,
            "river raster bbox should follow planned valley width, not the broad search cap: {active_radius}"
        );
        assert!(
            active_radius >= source.valley_width_blocks,
            "active radius must still cover the planned broad valley"
        );
    }

    #[test]
    fn rounded_river_raster_centerline_softens_noisy_interior_kinks() {
        let points = vec![
            WorldPlanePoint::new(0.0, 0.0),
            WorldPlanePoint::new(32.0, 18.0),
            WorldPlanePoint::new(64.0, -18.0),
            WorldPlanePoint::new(96.0, 18.0),
            WorldPlanePoint::new(128.0, 0.0),
        ];
        let rounded = rounded_river_raster_points(&points, 0.75, 64.0, 160.0, 256.0);

        assert_eq!(rounded.first(), points.first());
        assert_eq!(rounded.last(), points.last());
        assert!(
            rounded.len() > points.len(),
            "raster path should insert rounded realization points instead of following each Voronoi kink verbatim"
        );
        assert!(
            rounded
                .iter()
                .any(|point| point.x > 48.0 && point.x < 80.0 && point.z.abs() < 18.0),
            "interior noisy kinks should be pulled toward a rounded guide path: {rounded:?}"
        );
    }

    #[test]
    fn macro_field_raster_fills_near_threshold_concave_river_cusp() {
        let width = 9;
        let height = 9;
        let mut field = test_river_raster_field(width, height);
        let center = 4usize;
        for (x, z) in [
            (center, center - 1),
            (center, center + 1),
            (center - 1, center),
            (center + 1, center),
            (center - 1, center - 1),
            (center + 1, center - 1),
        ] {
            set_river_raster_strength(&mut field, width, x, z, RIVER_CORE_STRENGTH_THRESHOLD);
        }
        set_river_raster_strength(
            &mut field,
            width,
            center,
            center,
            RIVER_CORE_STRENGTH_THRESHOLD * RIVER_CONCAVE_CUSP_MIN_STRENGTH_RATIO,
        );

        smooth_river_concave_cusps(&mut field, width, height);

        assert!(
            field.river_valley_strength[center * width + center] >= RIVER_CORE_STRENGTH_THRESHOLD,
            "near-threshold concave cusp should be promoted in the macro_field river raster"
        );
    }

    #[test]
    fn macro_field_raster_keeps_convex_river_corner_rounded() {
        let width = 9;
        let height = 9;
        let mut field = test_river_raster_field(width, height);
        let outside = 4usize;
        for (x, z) in [
            (outside, outside - 1),
            (outside - 1, outside),
            (outside - 1, outside - 1),
        ] {
            set_river_raster_strength(&mut field, width, x, z, RIVER_CORE_STRENGTH_THRESHOLD);
        }
        set_river_raster_strength(
            &mut field,
            width,
            outside,
            outside,
            RIVER_CORE_STRENGTH_THRESHOLD * RIVER_CONCAVE_CUSP_MIN_STRENGTH_RATIO,
        );

        smooth_river_concave_cusps(&mut field, width, height);

        assert!(
            field.river_valley_strength[outside * width + outside] < RIVER_CORE_STRENGTH_THRESHOLD,
            "convex outside corners should not be squared off by macro_field cusp cleanup"
        );
    }

    #[test]
    fn macro_field_raster_fills_broad_shoulder_concave_bay_without_promoting_core() {
        let width = 9;
        let height = 9;
        let mut field = test_river_raster_field(width, height);
        let center = 4usize;
        set_river_shoulder_strength(&mut field, width, center - 1, center, 0.58);
        set_river_shoulder_strength(&mut field, width, center + 1, center, 0.55);
        set_river_shoulder_strength(&mut field, width, center, center - 1, 0.50);
        set_river_shoulder_strength(&mut field, width, center, center, 0.08);

        close_river_shoulder_concave_bays(&mut field, width, height, 1.0);

        let index = center * width + center;
        assert!(
            field.river_shoulder_strength[index] > 0.40,
            "broad shoulder concave bay should be closed without blurring the whole river edge: {}",
            field.river_shoulder_strength[index]
        );
        assert!(
            field.river_valley_strength[index] > 0.40,
            "legacy valley diagnostic should follow the shoulder bay closing"
        );
        assert_eq!(
            field.river_core_strength[index], 0.0,
            "broad shoulder closing must not promote the water/core channel"
        );
    }

    #[test]
    fn macro_field_raster_keeps_convex_shoulder_bank_rounded() {
        let width = 9;
        let height = 9;
        let mut field = test_river_raster_field(width, height);
        let outside = 4usize;
        set_river_shoulder_strength(&mut field, width, outside, outside - 1, 0.58);
        set_river_shoulder_strength(&mut field, width, outside - 1, outside, 0.55);
        set_river_shoulder_strength(&mut field, width, outside - 1, outside - 1, 0.50);
        set_river_shoulder_strength(&mut field, width, outside, outside, 0.08);

        close_river_shoulder_concave_bays(&mut field, width, height, 1.0);

        assert!(
            field.river_shoulder_strength[outside * width + outside] < 0.20,
            "convex outside shoulder corners should stay rounded instead of being squared off"
        );
    }

    #[test]
    fn river_anti_aliased_polyline_bake_keeps_segment_continuous() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};

        let curve = NoisyBoundaryCurve {
            edge: VoronoiEdgeId(1),
            profile: BoundaryProfile::Ordinary,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                sites: [VoronoiSiteId(1), VoronoiSiteId(2)],
                start: WorldPlanePoint::new(0.0, 16.0),
                end: WorldPlanePoint::new(96.0, 16.0),
            },
            points: vec![
                WorldPlanePoint::new(0.0, 16.0),
                WorldPlanePoint::new(96.0, 16.0),
            ],
            amplitude: 0.0,
            seed: 7,
            guard: BoundaryGuard {
                min_x: -16.0,
                max_x: 112.0,
                min_z: 0.0,
                max_z: 32.0,
            },
        };
        let config = MacroFieldTileConfig::new(0.0, 0.0, 7, 3, 16.0);
        let field = rasterize_curve_anti_aliased_polyline_field(
            &[test_river_source(&curve, 0.5)],
            config,
            32.0,
        );

        for x in 0..7 {
            let index = 7 + x;
            assert!(
                field.distance_blocks[index] <= f32::EPSILON,
                "sample on continuous segment should be source distance without point-splat gaps: x={x} distance={}",
                field.distance_blocks[index]
            );
            assert!(
                field.flow_hint[index] > 0.49 && field.flow_hint[index] < 0.51,
                "flow hint should remain stable along a single baked thick polyline"
            );
            assert!(
                field.river_valley_strength[index] > 0.0,
                "anti-aliased stroke should bake positive valley strength along the whole line"
            );
        }
    }

    #[test]
    fn river_anti_aliased_polyline_flow_blends_at_connected_segments() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};

        let left = NoisyBoundaryCurve {
            edge: VoronoiEdgeId(1),
            profile: BoundaryProfile::Ordinary,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                sites: [VoronoiSiteId(1), VoronoiSiteId(2)],
                start: WorldPlanePoint::new(0.0, 16.0),
                end: WorldPlanePoint::new(64.0, 16.0),
            },
            points: vec![
                WorldPlanePoint::new(0.0, 16.0),
                WorldPlanePoint::new(64.0, 16.0),
            ],
            amplitude: 0.0,
            seed: 7,
            guard: BoundaryGuard {
                min_x: -16.0,
                max_x: 80.0,
                min_z: 0.0,
                max_z: 32.0,
            },
        };
        let right = NoisyBoundaryCurve {
            edge: VoronoiEdgeId(2),
            profile: BoundaryProfile::Ordinary,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(2), VoronoiCornerId(3)],
                sites: [VoronoiSiteId(2), VoronoiSiteId(3)],
                start: WorldPlanePoint::new(64.0, 16.0),
                end: WorldPlanePoint::new(128.0, 16.0),
            },
            points: vec![
                WorldPlanePoint::new(64.0, 16.0),
                WorldPlanePoint::new(128.0, 16.0),
            ],
            amplitude: 0.0,
            seed: 8,
            guard: BoundaryGuard {
                min_x: 48.0,
                max_x: 144.0,
                min_z: 0.0,
                max_z: 32.0,
            },
        };
        let config = MacroFieldTileConfig::new(0.0, 0.0, 9, 3, 16.0);
        let field = rasterize_curve_anti_aliased_polyline_field(
            &[
                test_river_source(&left, 0.25),
                test_river_source(&right, 0.75),
            ],
            config,
            32.0,
        );
        let joint = 9 + 4;

        assert!(
            field.flow_hint[joint] > 0.35 && field.flow_hint[joint] < 0.65,
            "connected segment joint should blend nearby display flow instead of jumping by edge: {}",
            field.flow_hint[joint]
        );
        assert!(
            (field.flow_hint[joint - 1] - field.flow_hint[joint + 1]).abs() <= 0.51,
            "flow hint should not spike abruptly around a segment joint"
        );
    }

    #[test]
    fn river_anti_aliased_polyline_join_has_no_valley_strength_gap() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};

        let curve = NoisyBoundaryCurve {
            edge: VoronoiEdgeId(11),
            profile: BoundaryProfile::Ordinary,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(1), VoronoiCornerId(3)],
                sites: [VoronoiSiteId(1), VoronoiSiteId(2)],
                start: WorldPlanePoint::new(16.0, 16.0),
                end: WorldPlanePoint::new(80.0, 80.0),
            },
            points: vec![
                WorldPlanePoint::new(16.0, 16.0),
                WorldPlanePoint::new(48.0, 16.0),
                WorldPlanePoint::new(48.0, 80.0),
            ],
            amplitude: 0.0,
            seed: 11,
            guard: BoundaryGuard {
                min_x: 0.0,
                max_x: 96.0,
                min_z: 0.0,
                max_z: 96.0,
            },
        };
        let config = MacroFieldTileConfig::new(0.0, 0.0, 7, 7, 16.0);
        let field = rasterize_curve_anti_aliased_polyline_field(
            &[test_river_source(&curve, 0.75)],
            config,
            32.0,
        );
        let joint = 1 * 7 + 3;
        let before_joint = 1 * 7 + 2;
        let after_joint = 2 * 7 + 3;

        assert!(
            field.river_valley_strength[joint] >= field.river_valley_strength[before_joint] * 0.85,
            "thick polyline join should not create a pointed valley gap at the bend"
        );
        assert!(
            field.river_valley_strength[joint] >= field.river_valley_strength[after_joint] * 0.85,
            "thick polyline join should stay continuous onto the next segment"
        );
    }

    #[test]
    fn river_raster_uses_rounded_realization_for_kinked_noisy_curve() {
        let curve = test_noisy_curve(
            211,
            vec![
                WorldPlanePoint::new(16.0, 16.0),
                WorldPlanePoint::new(40.0, 16.0),
                WorldPlanePoint::new(40.0, 40.0),
            ],
        );
        let mut source = test_river_source(&curve, 0.60);
        source.water_width_blocks = 24.0;
        source.valley_width_blocks = 96.0;
        let config = MacroFieldTileConfig::new(0.0, 0.0, 8, 8, 8.0);
        let rounded = rounded_river_raster_sources(&[source], 128.0);

        assert!(
            !rounded[0]
                .points
                .iter()
                .any(|point| squared_distance(*point, WorldPlanePoint::new(40.0, 16.0)) <= 0.001),
            "macro_field river raster should not follow every noisy-edge kink as an exact centerline vertex"
        );

        let field = rasterize_curve_anti_aliased_polyline_field(&[source], config, 128.0);
        let inside_bend = 3 * 8 + 4;
        let approach = 2 * 8 + 3;
        let exit = 4 * 8 + 5;

        assert!(
            field.river_valley_strength[inside_bend]
                >= field.river_valley_strength[approach].min(field.river_valley_strength[exit])
                    * 0.85,
            "rounded macro_field river corridor should fill the bend instead of leaving a pointed cusp: inside={} approach={} exit={}",
            field.river_valley_strength[inside_bend],
            field.river_valley_strength[approach],
            field.river_valley_strength[exit]
        );
    }

    #[test]
    fn river_connected_broad_stroke_overlap_keeps_join_without_bulging() {
        let left = test_noisy_curve(
            301,
            vec![
                WorldPlanePoint::new(16.0, 64.0),
                WorldPlanePoint::new(64.0, 64.0),
            ],
        );
        let right = test_noisy_curve(
            302,
            vec![
                WorldPlanePoint::new(64.0, 64.0),
                WorldPlanePoint::new(64.0, 112.0),
            ],
        );
        let config = MacroFieldTileConfig::new(0.0, 0.0, 9, 9, 16.0);
        let field = rasterize_curve_anti_aliased_polyline_field(
            &[
                test_river_source(&left, 0.70),
                test_river_source(&right, 0.70),
            ],
            config,
            64.0,
        );
        let overlap_inside = 5 * 9 + 3;
        let horizontal_shoulder = 4 * 9 + 3;
        let vertical_shoulder = 5 * 9 + 4;
        let weakest_shoulder = field.river_valley_strength[horizontal_shoulder]
            .min(field.river_valley_strength[vertical_shoulder]);

        assert!(
            field.river_valley_strength[overlap_inside] >= weakest_shoulder * 0.78,
            "connected broad stroke overlap should keep a readable join: overlap={} shoulder_h={} shoulder_v={}",
            field.river_valley_strength[overlap_inside],
            field.river_valley_strength[horizontal_shoulder],
            field.river_valley_strength[vertical_shoulder]
        );
        assert!(
            field.river_valley_strength[overlap_inside] <= weakest_shoulder + 0.04,
            "connected broad stroke overlap should not round/bulge the join: overlap={} shoulder_h={} shoulder_v={}",
            field.river_valley_strength[overlap_inside],
            field.river_valley_strength[horizontal_shoulder],
            field.river_valley_strength[vertical_shoulder]
        );
    }

    #[test]
    fn river_straight_segment_shoulder_is_not_a_chain_of_point_blobs() {
        let curve = test_noisy_curve(
            311,
            vec![
                WorldPlanePoint::new(16.0, 32.0),
                WorldPlanePoint::new(144.0, 32.0),
            ],
        );
        let config = MacroFieldTileConfig::new(0.0, 0.0, 11, 5, 16.0);
        let field = rasterize_curve_anti_aliased_polyline_field(
            &[test_river_source(&curve, 0.65)],
            config,
            64.0,
        );
        let shoulder_strengths = (2..=8)
            .map(|x| field.river_valley_strength[3 * 11 + x])
            .collect::<Vec<_>>();
        let min = shoulder_strengths
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min);
        let max = shoulder_strengths
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(
            max - min <= 0.08,
            "straight segment shoulder should be strip-continuous rather than point-splat blobs: min={min} max={max}"
        );
    }

    #[test]
    fn river_longitudinal_hint_uses_source_arc_length() {
        let horizontal = test_noisy_curve(
            331,
            vec![
                WorldPlanePoint::new(0.0, 64.0),
                WorldPlanePoint::new(128.0, 64.0),
            ],
        );
        let vertical = test_noisy_curve(
            332,
            vec![
                WorldPlanePoint::new(32.0, 0.0),
                WorldPlanePoint::new(32.0, 128.0),
            ],
        );
        let config = MacroFieldTileConfig::new(0.0, 0.0, 9, 9, 16.0);
        let horizontal_field = rasterize_curve_anti_aliased_polyline_field(
            &[test_river_source(&horizontal, 0.65)],
            config,
            96.0,
        );
        let vertical_field = rasterize_curve_anti_aliased_polyline_field(
            &[test_river_source(&vertical, 0.65)],
            config,
            96.0,
        );
        let horizontal_center = 4 * 9 + 4;
        let vertical_center = 4 * 9 + 2;

        assert!(
            (horizontal_field.river_longitudinal_blocks[horizontal_center] - 64.0).abs() <= 0.001,
            "horizontal river should use distance along its own source line"
        );
        assert!(
            (vertical_field.river_longitudinal_blocks[vertical_center] - 64.0).abs() <= 0.001,
            "vertical river should use distance along its own source line instead of global x/z projection"
        );
    }

    #[test]
    fn river_longitudinal_hint_keeps_chain_offset_across_edges() {
        let curve = test_noisy_curve(
            334,
            vec![
                WorldPlanePoint::new(0.0, 64.0),
                WorldPlanePoint::new(128.0, 64.0),
            ],
        );
        let config = MacroFieldTileConfig::new(0.0, 0.0, 9, 9, 16.0);
        let mut source = test_river_source(&curve, 0.65);
        source.longitudinal_start_blocks = 192.0;

        let field = rasterize_curve_anti_aliased_polyline_field(&[source], config, 96.0);
        let center = 4 * 9 + 4;

        assert!(
            (field.river_longitudinal_blocks[center] - 256.0).abs() <= 0.001,
            "river longitudinal floor bias must use chain-local distance, not restart at every Voronoi edge"
        );
    }

    #[test]
    fn connected_river_overlap_blends_longitudinal_hint() {
        let left = test_noisy_curve(
            335,
            vec![
                WorldPlanePoint::new(0.0, 64.0),
                WorldPlanePoint::new(64.0, 64.0),
            ],
        );
        let right = test_noisy_curve(
            336,
            vec![
                WorldPlanePoint::new(64.0, 64.0),
                WorldPlanePoint::new(128.0, 64.0),
            ],
        );
        let config = MacroFieldTileConfig::new(0.0, 0.0, 9, 9, 16.0);
        let left_source = test_river_source(&left, 0.65);
        let mut right_source = test_river_source(&right, 0.65);
        right_source.longitudinal_start_blocks = 512.0;

        let field =
            rasterize_curve_anti_aliased_polyline_field(&[left_source, right_source], config, 96.0);
        let joint = 4 * 9 + 4;

        assert!(
            field.river_longitudinal_blocks[joint] > 128.0
                && field.river_longitudinal_blocks[joint] < 512.0,
            "same-component overlap should blend longitudinal hints instead of keeping a hard nearest-segment jump: {}",
            field.river_longitudinal_blocks[joint]
        );
    }

    #[test]
    fn river_cross_section_keeps_same_longitudinal_hint() {
        let curve = test_noisy_curve(
            333,
            vec![
                WorldPlanePoint::new(0.0, 64.0),
                WorldPlanePoint::new(128.0, 64.0),
            ],
        );
        let config = MacroFieldTileConfig::new(0.0, 0.0, 9, 9, 16.0);
        let field = rasterize_curve_anti_aliased_polyline_field(
            &[test_river_source(&curve, 0.65)],
            config,
            96.0,
        );
        let north_bank = 3 * 9 + 4;
        let south_bank = 5 * 9 + 4;

        assert!(field.river_valley_strength[north_bank] > 0.0);
        assert!(field.river_valley_strength[south_bank] > 0.0);
        assert!(
            (field.river_longitudinal_blocks[north_bank]
                - field.river_longitudinal_blocks[south_bank])
                .abs()
                <= 0.001,
            "samples across one river cross-section should share the projected arc-length hint"
        );
    }

    #[test]
    fn river_anti_aliased_polyline_prefers_nearest_segment_for_overlaps() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};

        let weak = NoisyBoundaryCurve {
            edge: VoronoiEdgeId(101),
            profile: BoundaryProfile::Ordinary,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                sites: [VoronoiSiteId(1), VoronoiSiteId(2)],
                start: WorldPlanePoint::new(0.0, 16.0),
                end: WorldPlanePoint::new(96.0, 16.0),
            },
            points: vec![
                WorldPlanePoint::new(0.0, 16.0),
                WorldPlanePoint::new(96.0, 16.0),
            ],
            amplitude: 0.0,
            seed: 101,
            guard: BoundaryGuard {
                min_x: -16.0,
                max_x: 112.0,
                min_z: 0.0,
                max_z: 32.0,
            },
        };
        let strong = NoisyBoundaryCurve {
            edge: VoronoiEdgeId(102),
            profile: BoundaryProfile::Ordinary,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(3), VoronoiCornerId(4)],
                sites: [VoronoiSiteId(3), VoronoiSiteId(4)],
                start: WorldPlanePoint::new(0.0, 32.0),
                end: WorldPlanePoint::new(96.0, 32.0),
            },
            points: vec![
                WorldPlanePoint::new(0.0, 32.0),
                WorldPlanePoint::new(96.0, 32.0),
            ],
            amplitude: 0.0,
            seed: 102,
            guard: BoundaryGuard {
                min_x: -16.0,
                max_x: 112.0,
                min_z: 16.0,
                max_z: 48.0,
            },
        };
        let config = MacroFieldTileConfig::new(0.0, 0.0, 7, 4, 16.0);
        let field = rasterize_curve_anti_aliased_polyline_field(
            &[
                test_river_source(&weak, 0.25),
                test_river_source(&strong, 0.90),
            ],
            config,
            64.0,
        );
        let weak_center = 1 * 7 + 3;

        assert!(
            field.flow_hint[weak_center] < 0.45,
            "overlapping broad strokes should not let a farther high-flow stroke own the near-bank sample: {}",
            field.flow_hint[weak_center]
        );
        assert!(
            field.river_valley_strength[weak_center] > 0.0,
            "nearest segment priority should still keep the local small river visible"
        );
    }

    #[test]
    fn river_parallel_independent_corridors_do_not_union_into_one_owner() {
        let weak = test_noisy_curve(
            321,
            vec![
                WorldPlanePoint::new(0.0, 32.0),
                WorldPlanePoint::new(128.0, 32.0),
            ],
        );
        let strong = test_noisy_curve(
            322,
            vec![
                WorldPlanePoint::new(0.0, 64.0),
                WorldPlanePoint::new(128.0, 64.0),
            ],
        );
        let config = MacroFieldTileConfig::new(0.0, 0.0, 9, 7, 16.0);
        let field = rasterize_curve_anti_aliased_polyline_field(
            &[
                test_river_source(&weak, 0.25),
                test_river_source(&strong, 0.90),
            ],
            config,
            64.0,
        );
        let midpoint = 3 * 9 + 4;
        let weak_center = 2 * 9 + 4;
        let strong_center = 4 * 9 + 4;

        assert!(
            field.flow_hint[midpoint] < 0.45,
            "equal-distance parallel corridors should keep deterministic local ownership instead of blending separate rivers: {}",
            field.flow_hint[midpoint]
        );
        assert!(
            field.river_valley_strength[midpoint]
                <= field.river_valley_strength[weak_center]
                    .max(field.river_valley_strength[strong_center]),
            "parallel independent rivers should not union into a wider single corridor"
        );
    }

    #[test]
    fn river_anti_aliased_polyline_preserves_flow_scaled_bed_width() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};

        let curve = NoisyBoundaryCurve {
            edge: VoronoiEdgeId(21),
            profile: BoundaryProfile::Ordinary,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                sites: [VoronoiSiteId(1), VoronoiSiteId(2)],
                start: WorldPlanePoint::new(0.0, 48.0),
                end: WorldPlanePoint::new(128.0, 48.0),
            },
            points: vec![
                WorldPlanePoint::new(0.0, 48.0),
                WorldPlanePoint::new(128.0, 48.0),
            ],
            amplitude: 0.0,
            seed: 21,
            guard: BoundaryGuard {
                min_x: -16.0,
                max_x: 144.0,
                min_z: 0.0,
                max_z: 96.0,
            },
        };
        let config = MacroFieldTileConfig::new(0.0, 0.0, 9, 7, 16.0);
        let headwater = rasterize_curve_anti_aliased_polyline_field(
            &[test_river_source(&curve, flow_hint(12.0))],
            config,
            96.0,
        );
        let trunk = rasterize_curve_anti_aliased_polyline_field(
            &[test_river_source(&curve, flow_hint(1024.0))],
            config,
            96.0,
        );
        let shoulder = 2 * 9 + 4;

        assert!(
            trunk.river_valley_strength[shoulder] > headwater.river_valley_strength[shoulder],
            "downstream thick polyline should keep a wider flat/shoulder bed than headwater"
        );
    }

    #[test]
    fn terminal_mouth_progress_grows_across_last_two_edges() {
        let source = RiverRasterWorkSource {
            is_terminal_outlet: true,
            edge: VoronoiEdgeId(99),
            points: vec![
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(128.0, 0.0),
            ],
            cumulative_lengths: vec![0.0, 128.0],
            open_start_cap: false,
            open_end_cap: false,
            longitudinal_start_blocks: 0.0,
            flow_hint: 0.70,
            water_width_blocks: 64.0,
            valley_width_blocks: 180.0,
            bed_depth_blocks: 14.0,
            terminal_mouth_factor: 1.0,
            component_id: 0,
        };

        let start = terminal_mouth_progress(&source, 0, 0.0);
        let middle = terminal_mouth_progress(&source, 0, 0.5);
        let end = terminal_mouth_progress(&source, 0, 1.0);
        let start_morphology = effective_river_morphology(&source, start);
        let end_morphology = effective_river_morphology(&source, end);

        assert!(
            start > 0.40 && middle > start && end > middle,
            "terminal mouth factor should grow downstream across the outlet edge: start={start} middle={middle} end={end}"
        );
        assert!(
            end_morphology.water_width_blocks > start_morphology.water_width_blocks
                && end_morphology.valley_width_blocks > start_morphology.valley_width_blocks
                && end_morphology.bed_depth_blocks > start_morphology.bed_depth_blocks,
            "terminal morphology should widen/deepen downstream: start={start_morphology:?} end={end_morphology:?}"
        );
    }

    #[test]
    fn terminal_mouth_width_gate_limits_short_outlet_capsule_width() {
        let mut short = RiverRasterWorkSource {
            is_terminal_outlet: true,
            edge: VoronoiEdgeId(98),
            points: vec![
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(48.0, 0.0),
            ],
            cumulative_lengths: vec![0.0, 48.0],
            open_start_cap: true,
            open_end_cap: false,
            longitudinal_start_blocks: 0.0,
            flow_hint: 0.67,
            water_width_blocks: 72.0,
            valley_width_blocks: 118.0,
            bed_depth_blocks: 12.0,
            terminal_mouth_factor: 1.0,
            component_id: 0,
        };
        let short_water = terminal_mouth_max_water_width_blocks(&short);
        let short_valley = terminal_mouth_max_valley_width_blocks(&short);

        short.points[1] = WorldPlanePoint::new(240.0, 0.0);
        short.cumulative_lengths[1] = 240.0;
        let long_water = terminal_mouth_max_water_width_blocks(&short);
        let long_valley = terminal_mouth_max_valley_width_blocks(&short);

        assert!(
            short_water < 56.0 && short_valley < 72.0,
            "short terminal outlet should shrink to a throat instead of stamping a broad round capsule: water={short_water} valley={short_valley}"
        );
        assert!(
            long_water > short_water * 1.8 && long_valley > short_valley * 2.0,
            "long terminal outlets may still use the planned widening: short=({short_water},{short_valley}) long=({long_water},{long_valley})"
        );
    }

    #[test]
    fn terminal_outlet_endpoint_clip_only_applies_beyond_final_mouth() {
        let source = RiverRasterWorkSource {
            is_terminal_outlet: true,
            edge: VoronoiEdgeId(99),
            points: vec![
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(128.0, 0.0),
            ],
            cumulative_lengths: vec![0.0, 128.0],
            open_start_cap: false,
            open_end_cap: false,
            longitudinal_start_blocks: 0.0,
            flow_hint: 0.70,
            water_width_blocks: 64.0,
            valley_width_blocks: 180.0,
            bed_depth_blocks: 14.0,
            terminal_mouth_factor: 1.0,
            component_id: 0,
        };
        let start = source.points[0];
        let end = source.points[1];

        assert!(
            terminal_outlet_endpoint_clips_sample(
                &source,
                0,
                WorldPlanePoint::new(144.0, 0.0),
                start,
                end,
            ),
            "terminal outlet samples downstream of the final endpoint should not keep the circular segment cap"
        );
        assert!(
            !terminal_outlet_endpoint_clips_sample(
                &source,
                0,
                WorldPlanePoint::new(120.0, 48.0),
                start,
                end,
            ),
            "samples inside the final segment should keep the widened mouth cross-section"
        );

        let mut inland = source.clone();
        inland.is_terminal_outlet = false;
        assert!(
            !terminal_outlet_endpoint_clips_sample(
                &inland,
                0,
                WorldPlanePoint::new(144.0, 0.0),
                start,
                end,
            ),
            "ordinary river segments still use the normal rounded stroke endpoint semantics"
        );
    }

    #[test]
    fn terminal_mouth_segment_uses_open_cross_section_before_endpoint() {
        let source = RiverRasterWorkSource {
            is_terminal_outlet: true,
            edge: VoronoiEdgeId(99),
            points: vec![
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(128.0, 0.0),
            ],
            cumulative_lengths: vec![0.0, 128.0],
            open_start_cap: false,
            open_end_cap: false,
            longitudinal_start_blocks: 0.0,
            flow_hint: 0.70,
            water_width_blocks: 56.0,
            valley_width_blocks: 180.0,
            bed_depth_blocks: 14.0,
            terminal_mouth_factor: 1.0,
            component_id: 0,
        };
        let start = source.points[0];
        let end = source.points[1];
        let center =
            river_segment_distance_and_t(&source, 0, WorldPlanePoint::new(128.0, 0.0), start, end);
        let outer_endpoint =
            river_segment_distance_and_t(&source, 0, WorldPlanePoint::new(128.0, 48.0), start, end);
        let downstream_endpoint =
            river_segment_distance_and_t(&source, 0, WorldPlanePoint::new(144.0, 0.0), start, end);

        assert_eq!(
            center.map(|(distance, _)| distance),
            Some(0.0),
            "terminal mouth centerline should remain available for river/estuary handoff"
        );
        assert!(
            outer_endpoint
                .map(|(distance, _)| (distance - 48.0).abs() <= f32::EPSILON)
                .unwrap_or(false),
            "outer terminal endpoint samples should keep the open final cross-section instead of being pinched inward: {outer_endpoint:?}"
        );
        assert!(
            downstream_endpoint.is_none(),
            "terminal mouth should still clip samples downstream of the final endpoint instead of emitting a round cap"
        );
    }

    #[test]
    fn terminal_mouth_raster_does_not_emit_round_cap_beyond_endpoint() {
        let curve = test_noisy_curve(
            91,
            vec![
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(128.0, 0.0),
            ],
        );
        let mut terminal = test_river_source(&curve, 0.72);
        terminal.is_terminal_outlet = true;
        terminal.water_width_blocks = 56.0;
        terminal.valley_width_blocks = 160.0;
        terminal.bed_depth_blocks = 12.0;
        terminal.terminal_mouth_factor = 1.0;

        let config = MacroFieldTileConfig::new(0.0, -64.0, 11, 9, 16.0);
        let field = rasterize_curve_anti_aliased_polyline_field(&[terminal], config, 256.0);
        let width = config.width as usize;
        let sample_index = |x: usize, z: usize| z * width + x;
        let inside_mouth = field.river_valley_strength[sample_index(7, 4)];
        let downstream_cap = field.river_valley_strength[sample_index(9, 4)];

        assert!(
            inside_mouth > 0.0,
            "terminal mouth should still carve inside the final river segment"
        );
        assert!(
            downstream_cap <= f32::EPSILON,
            "terminal mouth should not rasterize the old circular cap beyond its endpoint: {downstream_cap}"
        );
    }

    #[test]
    fn terminal_mouth_raster_keeps_open_outer_carve_without_downstream_cap() {
        let curve = test_noisy_curve(
            92,
            vec![
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(128.0, 0.0),
            ],
        );
        let mut terminal = test_river_source(&curve, 0.72);
        terminal.is_terminal_outlet = true;
        terminal.water_width_blocks = 56.0;
        terminal.valley_width_blocks = 180.0;
        terminal.bed_depth_blocks = 14.0;
        terminal.terminal_mouth_factor = 1.0;

        let config = MacroFieldTileConfig::new(0.0, -64.0, 11, 9, 16.0);
        let field = rasterize_curve_anti_aliased_polyline_field(&[terminal], config, 256.0);
        let width = config.width as usize;
        let sample_index = |x: usize, z: usize| z * width + x;
        let upstream_outer = field.river_valley_strength[sample_index(4, 6)];
        let endpoint_outer = field.river_valley_strength[sample_index(8, 6)];
        let downstream_outer = field.river_valley_strength[sample_index(9, 6)];
        let endpoint_center = field.river_core_strength[sample_index(8, 4)];

        assert!(
            endpoint_center > 0.35,
            "terminal mouth centerline should still keep an active handoff channel: {endpoint_center}"
        );
        assert!(
            endpoint_outer >= upstream_outer * 0.72,
            "terminal mouth outer carve should stay open to the endpoint instead of forming an inward concave bank: upstream={upstream_outer} endpoint={endpoint_outer}"
        );
        assert!(
            downstream_outer <= f32::EPSILON,
            "open terminal mouth should not restore a round cap downstream of the endpoint: {downstream_outer}"
        );
    }

    #[test]
    fn connected_river_sources_open_shared_endpoint_caps() {
        let left = test_noisy_curve(
            93,
            vec![
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(128.0, 0.0),
            ],
        );
        let right = test_noisy_curve(
            94,
            vec![
                WorldPlanePoint::new(128.0, 0.0),
                WorldPlanePoint::new(256.0, 0.0),
            ],
        );
        let mut sources = rounded_river_raster_sources(
            &[
                test_river_source(&left, 0.72),
                test_river_source(&right, 0.72),
            ],
            256.0,
        );

        assign_river_raster_components(&mut sources);

        assert!(sources[0].open_end_cap);
        assert!(sources[1].open_start_cap);
        assert!(
            river_segment_distance_and_t(
                &sources[0],
                sources[0].points.len() - 2,
                WorldPlanePoint::new(144.0, 0.0),
                sources[0].points[sources[0].points.len() - 2],
                *sources[0].points.last().unwrap(),
            )
            .is_none(),
            "shared downstream endpoint should not emit a round cap into the next river edge"
        );
        assert!(
            river_segment_distance_and_t(
                &sources[1],
                0,
                WorldPlanePoint::new(112.0, 0.0),
                sources[1].points[0],
                sources[1].points[1],
            )
            .is_none(),
            "shared upstream endpoint should not emit a round cap into the previous river edge"
        );
    }

    #[test]
    fn near_connected_river_sources_open_caps_without_exact_noisy_endpoint_match() {
        let left = test_noisy_curve(
            95,
            vec![
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(128.0, 0.0),
            ],
        );
        let right = test_noisy_curve(
            96,
            vec![
                WorldPlanePoint::new(132.0, 3.0),
                WorldPlanePoint::new(256.0, 0.0),
            ],
        );
        let mut left_source = test_river_source(&left, 0.72);
        left_source.water_width_blocks = 48.0;
        let mut right_source = test_river_source(&right, 0.72);
        right_source.water_width_blocks = 48.0;
        let mut sources = rounded_river_raster_sources(&[left_source, right_source], 256.0);

        assign_river_raster_components(&mut sources);

        assert!(
            sources[0].open_end_cap && sources[1].open_start_cap,
            "near-matching endpoints on the same routed river should open their caps so broad carving does not stamp overlapping round lobes"
        );
    }

    #[test]
    fn terminal_mouth_raster_widens_last_two_edges_without_estuary() {
        let left = test_noisy_curve(
            89,
            vec![
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(128.0, 0.0),
            ],
        );
        let right = test_noisy_curve(
            90,
            vec![
                WorldPlanePoint::new(128.0, 0.0),
                WorldPlanePoint::new(256.0, 0.0),
            ],
        );
        let mut penultimate = test_river_source(&left, 0.72);
        penultimate.water_width_blocks = 56.0;
        penultimate.valley_width_blocks = 160.0;
        penultimate.bed_depth_blocks = 12.0;
        penultimate.terminal_mouth_factor = 0.45;
        let mut terminal = test_river_source(&right, 0.72);
        terminal.is_terminal_outlet = true;
        terminal.water_width_blocks = 56.0;
        terminal.valley_width_blocks = 160.0;
        terminal.bed_depth_blocks = 12.0;
        terminal.terminal_mouth_factor = 1.0;

        let config = MacroFieldTileConfig::new(0.0, -96.0, 17, 13, 16.0);
        let field =
            rasterize_curve_anti_aliased_polyline_field(&[penultimate, terminal], config, 256.0);
        let width = config.width as usize;
        let sample_index = |x: usize, z: usize| z * width + x;
        let upstream_outer = field.river_valley_strength[sample_index(2, 8)];
        let penultimate_outer = field.river_valley_strength[sample_index(7, 8)];
        let terminal_outer = field.river_valley_strength[sample_index(14, 8)];

        assert!(
            penultimate_outer > upstream_outer,
            "penultimate mouth edge should begin widening river influence: upstream={upstream_outer} penultimate={penultimate_outer}"
        );
        assert!(
            terminal_outer >= penultimate_outer * 0.72,
            "terminal endpoint should keep the widened open mouth instead of pinching into a concave broad-carve edge: upstream={upstream_outer} penultimate={penultimate_outer} terminal={terminal_outer}"
        );
    }

    #[test]
    fn estuary_fan_profile_widens_downstream() {
        let fan = EstuaryFanRef {
            segment_id: 1,
            origin: WorldPlanePoint::new(0.0, 0.0),
            direction_x: 1.0,
            direction_z: 0.0,
            start_half_width_blocks: 12.0,
            end_half_width_blocks: 72.0,
            length_blocks: 180.0,
            flow_hint: 0.82,
            bed_depth_hint: 0.55,
            water_depth_hint: 0.42,
        };

        let start_edge = estuary_fan_sample(fan, WorldPlanePoint::new(12.0, 18.0));
        let start_mouth = estuary_fan_sample(fan, WorldPlanePoint::new(12.0, 8.0));
        let downstream_same_offset = estuary_fan_sample(fan, WorldPlanePoint::new(120.0, 18.0));
        let downstream_wide_edge = estuary_fan_sample(fan, WorldPlanePoint::new(120.0, 48.0));

        assert_eq!(
            estuary_fan_half_width_blocks(fan, 0.0),
            fan.start_half_width_blocks
        );
        assert!(
            estuary_fan_half_width_blocks(fan, 0.75) > fan.start_half_width_blocks * 3.0,
            "estuary fan should spread wider downstream instead of remaining a raw line"
        );
        assert!(
            downstream_same_offset.strength > start_edge.strength,
            "the same lateral offset should be more included after the fan widens: start={} downstream={}",
            start_edge.strength,
            downstream_same_offset.strength
        );
        assert!(
            downstream_wide_edge.strength > 0.0,
            "downstream fan should retain a broad shallow shelf influence"
        );
        assert!(
            start_mouth.water_strength >= RIVER_CORE_STRENGTH_THRESHOLD,
            "estuary fan mouth should expose water eligibility across the planned mouth width: {}",
            start_mouth.water_strength
        );
        assert!(
            downstream_wide_edge.water_strength >= RIVER_CORE_STRENGTH_THRESHOLD,
            "wide downstream fan should still carry water eligibility instead of only height carve: {}",
            downstream_wide_edge.water_strength
        );
    }

    #[test]
    fn estuary_fan_inlet_overlap_carves_short_open_throat_not_round_bowl() {
        let fan = EstuaryFanRef {
            segment_id: 1,
            origin: WorldPlanePoint::new(0.0, 0.0),
            direction_x: 1.0,
            direction_z: 0.0,
            start_half_width_blocks: 48.0,
            end_half_width_blocks: 144.0,
            length_blocks: 180.0,
            flow_hint: 0.82,
            bed_depth_hint: 0.55,
            water_depth_hint: 0.42,
        };

        let upstream_center = estuary_fan_sample(fan, WorldPlanePoint::new(-12.0, 0.0));
        let upstream_edge = estuary_fan_sample(fan, WorldPlanePoint::new(-12.0, 28.0));
        let upstream_outside = estuary_fan_sample(fan, WorldPlanePoint::new(-12.0, 60.0));
        let far_upstream_center = estuary_fan_sample(fan, WorldPlanePoint::new(-72.0, 0.0));
        let downstream_center = estuary_fan_sample(fan, WorldPlanePoint::new(12.0, 0.0));

        assert!(
            upstream_center.strength > 0.0,
            "fan inlet overlap should weakly carve into the terminal mouth so the broad outline does not pinch inward"
        );
        assert!(
            upstream_edge.strength > 0.0,
            "fan inlet overlap should keep the planned mouth-width throat instead of leaving a shoulder notch"
        );
        assert_eq!(
            upstream_outside.strength, 0.0,
            "the upstream throat should not become a rounded bowl outside the mouth width"
        );
        assert_eq!(
            far_upstream_center.strength, 0.0,
            "the upstream throat should fade out before forming a long artificial fan behind the river endpoint"
        );
        assert!(
            upstream_center.water_strength > RIVER_CORE_STRENGTH_THRESHOLD,
            "upstream overlap may still provide local water handoff eligibility"
        );
        assert!(
            downstream_center.strength > upstream_center.strength,
            "actual estuary fan body should still become stronger after the terminal endpoint"
        );
    }

    #[test]
    fn estuary_fan_body_starts_at_full_mouth_width_without_concave_throat() {
        let fan = EstuaryFanRef {
            segment_id: 1,
            origin: WorldPlanePoint::new(0.0, 0.0),
            direction_x: 1.0,
            direction_z: 0.0,
            start_half_width_blocks: 48.0,
            end_half_width_blocks: 144.0,
            length_blocks: 180.0,
            flow_hint: 0.82,
            bed_depth_hint: 0.55,
            water_depth_hint: 0.42,
        };

        let near_edge = estuary_fan_sample(fan, WorldPlanePoint::new(8.0, 40.0));
        let outside_edge = estuary_fan_sample(fan, WorldPlanePoint::new(8.0, 58.0));
        let upstream_edge = estuary_fan_sample(fan, WorldPlanePoint::new(-8.0, 40.0));

        assert!(
            near_edge.strength > upstream_edge.strength,
            "fan body should start at the planned mouth width instead of pinching inward into a concave throat"
        );
        assert!(
            outside_edge.strength <= f32::EPSILON,
            "fan body still should not stamp a broad round footprint outside the planned mouth"
        );
        assert!(
            upstream_edge.strength > 0.0,
            "upstream overlap should carry a weak open throat so the river/fan seam does not leave an inward concave border"
        );
    }

    #[test]
    fn strongest_estuary_sample_preserves_water_only_handoff() {
        let water_only = EstuaryFanSample {
            strength: 0.0,
            water_strength: 0.92,
            flow_hint: 0.7,
            bed_depth_hint: 0.4,
            water_depth_hint: 0.3,
            along_blocks: -8.0,
        };

        assert_eq!(
            strongest_estuary_sample(EstuaryFanSample::default(), water_only),
            water_only,
            "water-only inlet handoff samples should not be dropped just because they no longer carve"
        );
    }

    #[test]
    fn low_flow_estuary_fan_tail_remains_connected() {
        let fan = EstuaryFanRef {
            segment_id: 2,
            origin: WorldPlanePoint::new(0.0, 0.0),
            direction_x: 1.0,
            direction_z: 0.0,
            start_half_width_blocks: 6.0,
            end_half_width_blocks: 28.0,
            length_blocks: 144.0,
            flow_hint: 0.03,
            bed_depth_hint: 0.12,
            water_depth_hint: 0.08,
        };

        let center_tail = estuary_fan_sample(fan, WorldPlanePoint::new(120.0, 0.0));
        let edge_tail = estuary_fan_sample(fan, WorldPlanePoint::new(120.0, 10.0));

        assert!(
            center_tail.strength > 0.15,
            "small river mouths should keep a visible downstream fan centerline instead of ending early: {}",
            center_tail.strength
        );
        assert!(
            edge_tail.strength > 0.0,
            "small river mouths should still spread laterally near the tail: {}",
            edge_tail.strength
        );
    }

    #[test]
    fn macro_field_coast_mask_reads_canonical_coast_guide_curve() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let land_left = test_site(
            VoronoiSiteId(1),
            -64.0,
            0.0,
            MacroSurfaceKind::CoastLand,
            0.03,
        );
        let land_right = test_site(
            VoronoiSiteId(2),
            64.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.08,
        );
        let edge = VoronoiEdgeId(101);
        let start = WorldPlanePoint::new(0.0, -128.0);
        let bend = WorldPlanePoint::new(32.0, 0.0);
        let end = WorldPlanePoint::new(0.0, 128.0);
        let macro_map = GraphMacroMap {
            sites: vec![land_left, land_right],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [land_left.id, land_right.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: true,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 1.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 0.05,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![NoisyBoundaryCurve {
                edge,
                profile: BoundaryProfile::Coast,
                anchors: BoundaryAnchors {
                    corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                    sites: [land_left.id, land_right.id],
                    start,
                    end,
                },
                points: vec![start, bend, end],
                amplitude: 32.0,
                seed: 7,
                guard: BoundaryGuard {
                    min_x: -96.0,
                    max_x: 96.0,
                    min_z: -160.0,
                    max_z: 160.0,
                },
            }],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.coast_radius_blocks = 128.0;

        let sample = sample_macro_field_point(&context, config, bend);

        assert!(
            sample.coast_mask > 0.75,
            "macro_field must use the canonical coast guide curve for coast masks: {}",
            sample.coast_mask
        );
    }
}
