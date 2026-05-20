use rayon::prelude::*;
use std::collections::HashMap;

use super::context::MacroFieldRasterContext;
use super::height::{envelope, lerp, ridge_envelope, roughened_distance, smoothstep01};
use super::river::{
    nearest_point_on_segment, point_segment_distance, projected_t_on_segment,
    river_boundary_roughness_blocks, river_boundary_roughness_offset,
    river_core_strength_for_roughened_distance, river_hints_from_strength,
    river_shoulder_radius_blocks, river_valley_strength_for_roughened_distance,
    river_water_radius_blocks, squared_distance,
};
use super::types::{MacroFieldSample, MacroFieldTileConfig, MacroFieldTileStats};
use crate::world::generation::boundary::NoisyBoundaryCurve;
use crate::world::generation::graph::{VoronoiEdgeId, WorldPlanePoint};

pub(super) const RIDGE_FIELD_SOURCE_MIN_RIDGENESS: f32 = 0.44;
pub(super) const RIVER_CORE_STRENGTH_THRESHOLD: f32 = 0.88;
pub(super) const RIVER_CONCAVE_CUSP_MIN_STRENGTH_RATIO: f32 = 0.72;
pub(super) const RIVER_CONCAVE_CUSP_MIN_NEIGHBORS: usize = 5;
pub(super) const RIVER_CONCAVE_CUSP_MAX_PASSES: usize = 2;
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
    pub(super) river_bed_depth_hint: f32,
    pub(super) river_bank_roughness_hint: f32,
    pub(super) river_gravel_hint: f32,
    pub(super) river_cutbank_hint: f32,
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
    pub(super) river_bed_depth_hint: Vec<f32>,
    pub(super) river_bank_roughness_hint: Vec<f32>,
    pub(super) river_gravel_hint: Vec<f32>,
    pub(super) river_cutbank_hint: Vec<f32>,
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
            river_bed_depth_hint: self.river_bed_depth_hint[index].clamp(0.0, 1.0),
            river_bank_roughness_hint: self.river_bank_roughness_hint[index].clamp(0.0, 1.0),
            river_gravel_hint: self.river_gravel_hint[index].clamp(0.0, 1.0),
            river_cutbank_hint: self.river_cutbank_hint[index].clamp(0.0, 1.0),
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
            edge: river.edge,
            points: &river.points,
            longitudinal_start_blocks: river.longitudinal_start_blocks,
            flow_hint: river.flow_hint,
            water_width_blocks: river.water_width_blocks,
            valley_width_blocks: river.valley_width_blocks,
            bed_depth_blocks: river.bed_depth_blocks,
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
        river_bed_depth_hint: river.river_bed_depth_hint,
        river_bank_roughness_hint: river.river_bank_roughness_hint,
        river_gravel_hint: river.river_gravel_hint,
        river_cutbank_hint: river.river_cutbank_hint,
        stats,
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
    pub(super) river_bed_depth_hint: Vec<f32>,
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
    pub(super) river_bed_depth_hint: Vec<f32>,
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
            river_bed_depth_hint: vec![0.0; width],
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
        river.river_bed_depth_hint[index] =
            river.river_bed_depth_hint[index].max(river.river_bed_depth_hint[neighbor]);
        river.river_bank_roughness_hint[index] =
            river.river_bank_roughness_hint[index].max(river.river_bank_roughness_hint[neighbor]);
        river.river_gravel_hint[index] =
            river.river_gravel_hint[index].max(river.river_gravel_hint[neighbor]);
        river.river_cutbank_hint[index] =
            river.river_cutbank_hint[index].max(river.river_cutbank_hint[neighbor]);
        river.river_centerline_x[index] = river.river_centerline_x[neighbor];
        river.river_centerline_z[index] = river.river_centerline_z[neighbor];
        river.river_longitudinal_blocks[index] = river.river_longitudinal_blocks[neighbor];
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RiverRasterSource<'a> {
    pub(super) edge: VoronoiEdgeId,
    pub(super) points: &'a [WorldPlanePoint],
    pub(super) longitudinal_start_blocks: f32,
    pub(super) flow_hint: f32,
    pub(super) water_width_blocks: f32,
    pub(super) valley_width_blocks: f32,
    pub(super) bed_depth_blocks: f32,
    pub(super) component_id: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct RiverRasterWorkSource {
    pub(super) edge: VoronoiEdgeId,
    pub(super) points: Vec<WorldPlanePoint>,
    pub(super) cumulative_lengths: Vec<f32>,
    pub(super) longitudinal_start_blocks: f32,
    pub(super) flow_hint: f32,
    pub(super) water_width_blocks: f32,
    pub(super) valley_width_blocks: f32,
    pub(super) bed_depth_blocks: f32,
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
                edge: source.edge,
                cumulative_lengths: cumulative_polyline_lengths(&points),
                points,
                longitudinal_start_blocks: source.longitudinal_start_blocks,
                flow_hint: source.flow_hint,
                water_width_blocks: source.water_width_blocks,
                valley_width_blocks: source.valley_width_blocks,
                bed_depth_blocks: source.bed_depth_blocks,
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
            if river_sources_touch(&sources[left], &sources[right]) {
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

pub(super) fn river_sources_touch(
    left: &RiverRasterWorkSource,
    right: &RiverRasterWorkSource,
) -> bool {
    let Some(left_start) = left.points.first() else {
        return false;
    };
    let Some(left_end) = left.points.last() else {
        return false;
    };
    let Some(right_start) = right.points.first() else {
        return false;
    };
    let Some(right_end) = right.points.last() else {
        return false;
    };
    const ENDPOINT_EPSILON_BLOCKS: f32 = 0.01;
    let threshold = ENDPOINT_EPSILON_BLOCKS * ENDPOINT_EPSILON_BLOCKS;
    [
        (*left_start, *right_start),
        (*left_start, *right_end),
        (*left_end, *right_start),
        (*left_end, *right_end),
    ]
    .into_iter()
    .any(|(a, b)| squared_distance(a, b) <= threshold)
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
            river_bed_depth_hint: vec![0.0; sample_count],
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
        river_bed_depth_hint: vec![0.0; sample_count],
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
            river_bed_depth_hint: vec![0.0; sample_count],
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
    let mut river_bed_depth_hint = Vec::with_capacity(sample_count);
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
        river_bed_depth_hint.extend(row.river_bed_depth_hint);
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
    RasterDistanceField {
        distance_blocks,
        river_core_strength,
        river_shoulder_strength,
        river_valley_strength,
        river_centerline_x,
        river_centerline_z,
        river_longitudinal_blocks,
        flow_hint,
        river_bed_depth_hint,
        river_bank_roughness_hint,
        river_gravel_hint,
        river_cutbank_hint,
        source_pixel_count,
    }
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
    let strength = source.flow_hint;
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
        let distance = point_segment_distance(position, start, end);
        if distance > active_radius_blocks + aa_margin {
            continue;
        }
        let centerline_position = nearest_point_on_segment(position, start, end);
        let centerline_t = projected_t_on_segment(position, start, end);
        let longitudinal_blocks = segment_longitudinal_blocks(source, segment_index, centerline_t);
        let mut profile_sum = 0.0;
        let mut core_sum = 0.0;
        let mut shoulder_sum = 0.0;
        let mut closest_subpixel_distance = distance;
        let mut bed_sum = 0.0;
        let mut rough_sum = 0.0;
        let mut gravel_sum = 0.0;
        let mut cutbank_sum = 0.0;
        let roughness_offset = river_boundary_roughness_offset(
            position,
            source.flow_hint,
            source.water_width_blocks,
            radius_blocks,
            config.boundary_roughness_blocks,
        );
        for (offset_x, offset_z) in subpixel_offsets {
            let subpixel = WorldPlanePoint::new(
                position.x + offset_x * spacing,
                position.z + offset_z * spacing,
            );
            let subpixel_distance = point_segment_distance(subpixel, start, end);
            let core_strength = river_core_strength_for_roughened_distance(
                subpixel_distance,
                roughness_offset,
                source.flow_hint,
                source.water_width_blocks,
                radius_blocks,
            );
            let shoulder_strength = river_valley_strength_for_roughened_distance(
                subpixel_distance,
                roughness_offset,
                source.flow_hint,
                source.water_width_blocks,
                source.valley_width_blocks,
                radius_blocks,
            );
            let valley_strength = core_strength.max(shoulder_strength);
            let hints =
                river_hints_from_strength(core_strength, source.flow_hint, source.bed_depth_blocks);
            closest_subpixel_distance = closest_subpixel_distance.min(subpixel_distance);
            core_sum += core_strength;
            shoulder_sum += shoulder_strength;
            profile_sum += valley_strength;
            bed_sum += hints.bed_depth_hint;
            rough_sum += hints.bank_roughness_hint;
            gravel_sum += hints.gravel_hint;
            cutbank_sum += hints.cutbank_hint;
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
            row.river_bed_depth_hint[x] = row.river_bed_depth_hint[x].max(bed_hint);
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
            row.flow_weighted_sum[x] += strength * anti_aliased_strength;
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
            row.river_bed_depth_hint[x] = bed_hint;
            row.river_bank_roughness_hint[x] = rough_hint;
            row.river_gravel_hint[x] = gravel_hint;
            row.river_cutbank_hint[x] = cutbank_hint;
            row.flow_weighted_sum[x] = strength * anti_aliased_strength;
            row.flow_weight_sum[x] = anti_aliased_strength;
        }
    }
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
        self.flow_hint
    }

    fn water_width_blocks(&self) -> f32 {
        self.water_width_blocks
    }

    fn valley_width_blocks(&self) -> f32 {
        self.valley_width_blocks
    }
}

impl RiverRasterRadiusSource for RiverRasterSource<'_> {
    fn flow_hint(&self) -> f32 {
        self.flow_hint
    }

    fn water_width_blocks(&self) -> f32 {
        self.water_width_blocks
    }

    fn valley_width_blocks(&self) -> f32 {
        self.valley_width_blocks
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
