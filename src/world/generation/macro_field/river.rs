use super::context::{MACRO_FIELD_CURVE_BUCKET_BLOCKS, MACRO_FIELD_SITE_BUCKET_BLOCKS};
use super::height::{boundary_roughness_offset, lerp, river_shoulder_log_growth, smoothstep01};
use crate::world::generation::graph::WorldPlanePoint;
use crate::world::generation::river_plan::{
    DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS, RiverSegmentPlan,
};

pub(super) const RIVER_BOUNDARY_ROUGHNESS_SALT: u64 = 0xA11E_2F17_5EED_CAFE;
#[derive(Debug, Clone, Copy)]
pub(super) struct NearestPolylineSegment {
    pub(super) start: WorldPlanePoint,
    pub(super) end: WorldPlanePoint,
    pub(super) distance: f32,
}
pub(super) fn flow_hint_from_plan(plan: &RiverSegmentPlan) -> f32 {
    let width_t =
        (plan.bed_width_blocks / DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS).clamp(0.0, 1.0);
    let flow_t = ((plan.discharge_q + 1.0).ln() / (1024.0_f32 + 1.0).ln()).clamp(0.0, 1.0);
    width_t.max(flow_t * 0.85)
}

#[cfg(test)]
pub(super) fn river_valley_strength_for_distance(
    distance_blocks: f32,
    flow_hint: f32,
    planned_water_width_blocks: f32,
    planned_valley_width_blocks: f32,
    configured_radius_blocks: f32,
) -> f32 {
    river_valley_strength_for_effective_distance(
        distance_blocks,
        flow_hint,
        planned_water_width_blocks,
        planned_valley_width_blocks,
        configured_radius_blocks,
    )
}

#[cfg(test)]
pub(super) fn river_valley_strength_for_sample_distance(
    distance_blocks: f32,
    position: WorldPlanePoint,
    flow_hint: f32,
    planned_water_width_blocks: f32,
    planned_valley_width_blocks: f32,
    configured_radius_blocks: f32,
    boundary_roughness_blocks: f32,
) -> f32 {
    let roughness_offset = river_boundary_roughness_offset(
        position,
        flow_hint,
        planned_water_width_blocks,
        configured_radius_blocks,
        boundary_roughness_blocks,
    );
    river_valley_strength_for_roughened_distance(
        distance_blocks,
        roughness_offset,
        flow_hint,
        planned_water_width_blocks,
        planned_valley_width_blocks,
        configured_radius_blocks,
    )
}

pub(super) fn river_valley_strength_for_roughened_distance(
    distance_blocks: f32,
    roughness_offset_blocks: f32,
    flow_hint: f32,
    planned_water_width_blocks: f32,
    planned_valley_width_blocks: f32,
    configured_radius_blocks: f32,
) -> f32 {
    let water_radius = river_water_radius_blocks(
        flow_hint,
        planned_water_width_blocks,
        configured_radius_blocks,
    );
    let effective_distance =
        if roughness_offset_blocks.abs() > f32::EPSILON && distance_blocks.is_finite() {
            let boundary_band = (water_radius * 0.42)
                .max(roughness_offset_blocks.abs() * 2.0)
                .max(1.0);
            let boundary_t = (distance_blocks - water_radius).abs() / boundary_band;
            let boundary_gate = 1.0 - smoothstep01(boundary_t);
            (distance_blocks + roughness_offset_blocks * boundary_gate).max(0.0)
        } else {
            distance_blocks
        };

    river_valley_strength_for_effective_distance(
        effective_distance,
        flow_hint,
        planned_water_width_blocks,
        planned_valley_width_blocks,
        configured_radius_blocks,
    )
}

pub(super) fn river_core_strength_for_roughened_distance(
    distance_blocks: f32,
    roughness_offset_blocks: f32,
    flow_hint: f32,
    planned_water_width_blocks: f32,
    configured_radius_blocks: f32,
) -> f32 {
    let water_radius = river_water_radius_blocks(
        flow_hint,
        planned_water_width_blocks,
        configured_radius_blocks,
    );
    let effective_distance =
        if roughness_offset_blocks.abs() > f32::EPSILON && distance_blocks.is_finite() {
            let boundary_band = (water_radius * 0.42)
                .max(roughness_offset_blocks.abs() * 2.0)
                .max(1.0);
            let boundary_t = (distance_blocks - water_radius).abs() / boundary_band;
            let boundary_gate = 1.0 - smoothstep01(boundary_t);
            (distance_blocks + roughness_offset_blocks * boundary_gate).max(0.0)
        } else {
            distance_blocks
        };

    river_core_strength_for_effective_distance(
        effective_distance,
        flow_hint,
        planned_water_width_blocks,
        configured_radius_blocks,
    )
}

pub(super) fn river_boundary_roughness_offset(
    position: WorldPlanePoint,
    flow_hint: f32,
    planned_water_width_blocks: f32,
    configured_radius_blocks: f32,
    boundary_roughness_blocks: f32,
) -> f32 {
    let roughness_blocks = river_boundary_roughness_blocks(
        flow_hint,
        planned_water_width_blocks,
        configured_radius_blocks,
        boundary_roughness_blocks,
    );
    if roughness_blocks <= 0.0 {
        0.0
    } else {
        boundary_roughness_offset(position, roughness_blocks, RIVER_BOUNDARY_ROUGHNESS_SALT)
    }
}

pub(super) fn river_valley_strength_for_effective_distance(
    distance_blocks: f32,
    flow_hint: f32,
    planned_water_width_blocks: f32,
    planned_valley_width_blocks: f32,
    configured_radius_blocks: f32,
) -> f32 {
    let water_radius = river_water_radius_blocks(
        flow_hint,
        planned_water_width_blocks,
        configured_radius_blocks,
    );
    let valley_radius = river_shoulder_radius_blocks(
        flow_hint,
        planned_valley_width_blocks,
        configured_radius_blocks,
    )
    .max(water_radius + 1.0);
    if !distance_blocks.is_finite() || distance_blocks >= valley_radius {
        return 0.0;
    }
    let shoulder_cap = river_shoulder_strength_cap(flow_hint);
    if distance_blocks <= water_radius {
        let t = (distance_blocks / water_radius.max(f32::EPSILON)).clamp(0.0, 1.0);
        return lerp(shoulder_cap, shoulder_cap * 0.88, smoothstep01(t)).clamp(0.0, 1.0);
    }
    let t = ((distance_blocks - water_radius) / (valley_radius - water_radius).max(f32::EPSILON))
        .clamp(0.0, 1.0);
    let shoulder_falloff_power = lerp(1.12, 1.25, river_shoulder_log_growth(flow_hint));
    (shoulder_cap * (1.0 - smoothstep01(t)).powf(shoulder_falloff_power)).clamp(0.0, 1.0)
}

pub(super) fn river_core_strength_for_effective_distance(
    distance_blocks: f32,
    flow_hint: f32,
    planned_water_width_blocks: f32,
    configured_radius_blocks: f32,
) -> f32 {
    let water_radius = river_water_radius_blocks(
        flow_hint,
        planned_water_width_blocks,
        configured_radius_blocks,
    );
    if !distance_blocks.is_finite() || distance_blocks > water_radius {
        return 0.0;
    }
    let t = (distance_blocks / water_radius.max(f32::EPSILON)).clamp(0.0, 1.0);
    lerp(1.0, 0.89, smoothstep01(t)).clamp(0.0, 1.0)
}

pub(super) fn river_boundary_roughness_blocks(
    flow_hint: f32,
    planned_water_width_blocks: f32,
    configured_radius_blocks: f32,
    boundary_roughness_blocks: f32,
) -> f32 {
    if boundary_roughness_blocks <= 0.0 {
        return 0.0;
    }
    let water_radius = river_water_radius_blocks(
        flow_hint,
        planned_water_width_blocks,
        configured_radius_blocks,
    );
    let flow_t = smoothstep01(flow_hint.clamp(0.0, 1.0));
    let radius_scale = lerp(0.10, 0.22, flow_t);
    (water_radius * radius_scale)
        .min(boundary_roughness_blocks * 0.18)
        .min(24.0)
        .clamp(0.0, water_radius * 0.33)
}

pub(super) fn river_shoulder_radius_blocks(
    flow_hint: f32,
    planned_width_blocks: f32,
    configured_radius_blocks: f32,
) -> f32 {
    let flow_t = smoothstep01(flow_hint.clamp(0.0, 1.0));
    let fallback = lerp(3.0, configured_radius_blocks, flow_t);
    let planned = planned_width_blocks
        .max(fallback.min(18.0))
        .clamp(1.0, configured_radius_blocks);
    let upstream_radius = planned.min(18.0).max(3.0);
    let downstream_cap = (planned * 0.5)
        .max(upstream_radius)
        .min(configured_radius_blocks * 0.5);
    lerp(
        upstream_radius,
        downstream_cap,
        river_shoulder_log_growth(flow_hint),
    )
    .min(planned)
    .clamp(1.0, configured_radius_blocks)
}

pub(super) fn river_shoulder_strength_cap(flow_hint: f32) -> f32 {
    lerp(0.86, 0.50, river_shoulder_log_growth(flow_hint)).clamp(0.0, 1.0)
}

pub(super) fn river_water_radius_blocks(
    flow_hint: f32,
    planned_water_width_blocks: f32,
    configured_radius_blocks: f32,
) -> f32 {
    let flow_t = smoothstep01(flow_hint.clamp(0.0, 1.0));
    let fallback = lerp(
        1.0,
        DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS * 0.5,
        flow_t,
    );
    (planned_water_width_blocks * 0.5)
        .max(fallback.min(4.0))
        .clamp(0.5, configured_radius_blocks)
}

pub(super) fn river_depth_factor(flow_hint: f32) -> f32 {
    lerp(0.10, 0.62, smoothstep01(flow_hint.clamp(0.0, 1.0))).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct RiverInfluenceSample {
    pub(super) distance_blocks: f32,
    pub(super) flow_hint: f32,
    pub(super) core_strength: f32,
    pub(super) shoulder_strength: f32,
    pub(super) valley_strength: f32,
    pub(super) bed_depth_hint: f32,
    pub(super) bank_roughness_hint: f32,
    pub(super) gravel_hint: f32,
    pub(super) cutbank_hint: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct RiverInfluenceHints {
    pub(super) bed_depth_hint: f32,
    pub(super) bank_roughness_hint: f32,
    pub(super) gravel_hint: f32,
    pub(super) cutbank_hint: f32,
}

pub(super) fn river_hints_from_strength(
    valley_strength: f32,
    flow_hint: f32,
    planned_bed_depth_blocks: f32,
) -> RiverInfluenceHints {
    let valley = valley_strength.clamp(0.0, 1.0);
    let flow = flow_hint.clamp(0.0, 1.0);
    let planned_depth_hint = (planned_bed_depth_blocks.max(0.0) / 36.0).clamp(0.0, 1.0);
    let flow_depth_floor = river_depth_factor(flow) * 0.50;
    let depth_hint = planned_depth_hint.max(flow_depth_floor.min(planned_depth_hint + 0.035));
    RiverInfluenceHints {
        bed_depth_hint: (valley * depth_hint).clamp(0.0, 1.0),
        bank_roughness_hint: (valley * (1.0 - flow * 0.45)).clamp(0.0, 1.0),
        gravel_hint: (valley * (0.65 - flow * 0.25)).clamp(0.0, 1.0),
        cutbank_hint: 0.0,
    }
}

pub(super) fn river_influence_sample(
    position: WorldPlanePoint,
    points: &[WorldPlanePoint],
    flow_hint: f32,
    planned_water_width_blocks: f32,
    planned_valley_width_blocks: f32,
    planned_bed_depth_blocks: f32,
    configured_radius_blocks: f32,
    boundary_roughness_blocks: f32,
) -> RiverInfluenceSample {
    let flow_hint = flow_hint.clamp(0.0, 1.0);
    let distance = polyline_distance(position, points);
    let roughness_offset = river_boundary_roughness_offset(
        position,
        flow_hint,
        planned_water_width_blocks,
        configured_radius_blocks,
        boundary_roughness_blocks,
    );
    let core_strength = river_core_strength_for_roughened_distance(
        distance,
        roughness_offset,
        flow_hint,
        planned_water_width_blocks,
        configured_radius_blocks,
    );
    let shoulder_strength = river_valley_strength_for_roughened_distance(
        distance,
        roughness_offset,
        flow_hint,
        planned_water_width_blocks,
        planned_valley_width_blocks,
        configured_radius_blocks,
    );
    let valley_strength = core_strength.max(shoulder_strength);
    let hints = river_hints_from_strength(core_strength, flow_hint, planned_bed_depth_blocks);

    RiverInfluenceSample {
        distance_blocks: distance,
        flow_hint,
        core_strength: core_strength.clamp(0.0, 1.0),
        shoulder_strength: shoulder_strength.clamp(0.0, 1.0),
        valley_strength: valley_strength.clamp(0.0, 1.0),
        bed_depth_hint: hints.bed_depth_hint,
        bank_roughness_hint: hints.bank_roughness_hint,
        gravel_hint: hints.gravel_hint,
        cutbank_hint: hints.cutbank_hint,
    }
}

pub(super) fn polyline_distance(position: WorldPlanePoint, points: &[WorldPlanePoint]) -> f32 {
    let distance_squared = match points {
        [] => f32::INFINITY,
        [point] => squared_distance(position, *point),
        _ => points
            .windows(2)
            .map(|segment| point_segment_distance_squared(position, segment[0], segment[1]))
            .fold(f32::INFINITY, f32::min),
    };
    distance_squared.sqrt()
}

pub(super) fn nearest_polyline_segment(
    position: WorldPlanePoint,
    points: &[WorldPlanePoint],
) -> Option<NearestPolylineSegment> {
    match points {
        [] | [_] => None,
        _ => points
            .windows(2)
            .map(|segment| {
                let distance_squared =
                    point_segment_distance_squared(position, segment[0], segment[1]);
                (segment, distance_squared)
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(segment, distance_squared)| NearestPolylineSegment {
                start: segment[0],
                end: segment[1],
                distance: distance_squared.sqrt(),
            }),
    }
}

pub(super) fn point_segment_distance(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    point_segment_distance_squared(point, start, end).sqrt()
}

pub(super) fn nearest_point_on_segment(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> WorldPlanePoint {
    let t = projected_t_on_segment(point, start, end);
    WorldPlanePoint::new(
        start.x + (end.x - start.x) * t,
        start.z + (end.z - start.z) * t,
    )
}

pub(super) fn projected_t_on_segment(
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
    (((point.x - start.x) * dx + (point.z - start.z) * dz) / len2).clamp(0.0, 1.0)
}

pub(super) fn point_segment_distance_squared(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    let projected = nearest_point_on_segment(point, start, end);
    squared_distance(point, projected)
}

pub(super) fn squared_distance(a: WorldPlanePoint, b: WorldPlanePoint) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}

pub(super) fn curve_bucket(point: WorldPlanePoint) -> (i32, i32) {
    (
        (point.x / MACRO_FIELD_CURVE_BUCKET_BLOCKS).floor() as i32,
        (point.z / MACRO_FIELD_CURVE_BUCKET_BLOCKS).floor() as i32,
    )
}

pub(super) fn curve_bucket_search_radius(radius: f32) -> i32 {
    ((radius + MACRO_FIELD_CURVE_BUCKET_BLOCKS * 0.5) / MACRO_FIELD_CURVE_BUCKET_BLOCKS).ceil()
        as i32
}

pub(super) fn site_bucket(point: WorldPlanePoint) -> (i32, i32) {
    (
        (point.x / MACRO_FIELD_SITE_BUCKET_BLOCKS).floor() as i32,
        (point.z / MACRO_FIELD_SITE_BUCKET_BLOCKS).floor() as i32,
    )
}

pub(super) fn signed_side(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    (point.x - start.x) * dz - (point.z - start.z) * dx
}

pub(super) fn usable_side(preferred: f32, fallback: f32) -> f32 {
    if preferred.abs() > f32::EPSILON {
        preferred
    } else if fallback.abs() > f32::EPSILON {
        -fallback
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::world::generation::boundary::{
        BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
    };
    use crate::world::generation::graph::{
        VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId, WorldPlanePoint,
    };
    use crate::world::generation::macro_field::height::combine_macro_height;
    use crate::world::generation::macro_field::test_support::*;
    use crate::world::generation::macro_field::{
        DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS, MacroFieldRasterContext, MacroFieldTileConfig,
        generate_macro_field_tile, sample_macro_field_point,
    };
    use crate::world::generation::macro_map::{
        GraphMacroMap, MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
    };
    use crate::world::generation::river_plan::{
        DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS, RiverPlan,
    };

    #[test]
    fn upstream_broad_valley_context_is_narrow_but_keeps_river_bed_hint() {
        let config = test_tile_config();
        let base = combine_macro_height(0.36, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let upstream = combine_macro_height(0.36, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.02, config);
        let downstream = combine_macro_height(0.36, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, config);
        let low_flow = 0.02;
        let inside_narrow_valley =
            river_valley_strength_for_distance(3.0, low_flow, 4.0, 6.0, config.river_radius_blocks);
        let outside_narrow_valley =
            river_valley_strength_for_distance(9.0, low_flow, 4.0, 6.0, config.river_radius_blocks);
        let hints = river_hints_from_strength(1.0, 0.02, 1.6);

        assert!(
            base - upstream >= config.river_carve_scale * 0.05,
            "headwater valley context should stay meaningful while width is narrowed: base={base} upstream={upstream}"
        );
        assert!(
            inside_narrow_valley > 0.67 && outside_narrow_valley == 0.0,
            "low-flow valley context should keep a visible immediate shoulder without widening the narrow reach: inside={inside_narrow_valley} outside={outside_narrow_valley}"
        );
        let near_shoulder =
            river_valley_strength_for_distance(5.0, low_flow, 4.0, 6.0, config.river_radius_blocks);
        assert!(
            near_shoulder > 0.10,
            "low-flow shoulder should not collapse immediately outside the core bed: {near_shoulder}"
        );
        assert!(
            downstream < upstream - config.river_carve_scale * 0.35,
            "downstream broad-valley context should still scale up with Q without restoring a uniform boundary-shaped floor: upstream={upstream} downstream={downstream}"
        );
        assert!(
            hints.bed_depth_hint > 0.05,
            "narrower broad land carve must preserve a deeper selected river bed/water depth hint: {}",
            hints.bed_depth_hint
        );
    }

    #[test]
    fn river_core_depth_hint_is_stronger_without_using_shoulder_strength() {
        let low_flow_core = river_hints_from_strength(1.0, 0.02, 1.6);
        let low_flow_shoulder_only = river_hints_from_strength(0.0, 0.02, 1.6);
        let trunk_core = river_hints_from_strength(1.0, 1.0, 24.0);

        assert!(
            low_flow_core.bed_depth_hint > 0.05,
            "low-flow core bed hint should stay visible: {}",
            low_flow_core.bed_depth_hint
        );
        assert_eq!(
            low_flow_shoulder_only.bed_depth_hint, 0.0,
            "bed depth hint must stay tied to river/core strength, not broad shoulder context"
        );
        assert!(
            trunk_core.bed_depth_hint > 0.66,
            "planned downstream bed depth should be scaled deeper overall: {}",
            trunk_core.bed_depth_hint
        );
    }

    #[test]
    fn river_width_and_depth_increase_with_flow() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let headwater_flow = flow_hint(12.0);
        let trunk_flow = flow_hint(1024.0);

        assert!(
            river_shoulder_radius_blocks(headwater_flow, 4.0, radius)
                < river_shoulder_radius_blocks(trunk_flow, 180.0, radius),
            "river shoulder radius should grow with selected/display flow"
        );
        assert!(
            river_depth_factor(headwater_flow) < river_depth_factor(trunk_flow),
            "river carve depth should grow with selected/display flow"
        );
        assert!(
            river_shoulder_radius_blocks(headwater_flow, 4.0, radius) < radius * 0.10,
            "headwater rivers should be much narrower than the maximum downstream radius"
        );
    }

    #[test]
    fn river_boundary_roughness_scales_with_planned_water_width() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let headwater = river_boundary_roughness_blocks(0.02, 4.0, radius, 96.0);
        let trunk = river_boundary_roughness_blocks(
            flow_hint(1024.0),
            DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS,
            radius,
            96.0,
        );

        assert!(
            headwater <= 0.8,
            "headwater water-boundary roughness should stay sub-block scale: {headwater}"
        );
        assert!(
            trunk > headwater * 8.0 && trunk <= 24.0,
            "downstream river roughness should be visible but bounded: headwater={headwater} trunk={trunk}"
        );
    }

    #[test]
    fn river_boundary_roughness_perturbs_water_core_threshold_only_near_bank() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let flow = flow_hint(1024.0);
        let water_width = DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS;
        let valley_width = 520.0;
        let water_radius = super::river_water_radius_blocks(flow, water_width, radius);
        let roughness = river_boundary_roughness_blocks(flow, water_width, radius, 96.0);
        let threshold_distance = water_radius + roughness * 0.35;
        let base = river_core_strength_for_effective_distance(
            threshold_distance,
            flow,
            water_width,
            radius,
        );
        let mut min_rough = f32::INFINITY;
        let mut max_rough = f32::NEG_INFINITY;
        for x in 0..128 {
            let position = WorldPlanePoint::new(x as f32 * 19.0, 37.0);
            let roughness_offset =
                super::river_boundary_roughness_offset(position, flow, water_width, radius, 96.0);
            let rough = super::river_core_strength_for_roughened_distance(
                threshold_distance,
                roughness_offset,
                flow,
                water_width,
                radius,
            );
            min_rough = min_rough.min(rough);
            max_rough = max_rough.max(rough);
            assert_eq!(
                rough,
                {
                    let roughness_offset = super::river_boundary_roughness_offset(
                        position,
                        flow,
                        water_width,
                        radius,
                        96.0,
                    );
                    super::river_core_strength_for_roughened_distance(
                        threshold_distance,
                        roughness_offset,
                        flow,
                        water_width,
                        radius,
                    )
                },
                "river boundary roughness must be deterministic at a world position"
            );
        }
        let far_distance = water_radius + roughness * 4.0;
        let far_base = river_valley_strength_for_distance(
            far_distance,
            flow,
            water_width,
            valley_width,
            radius,
        );
        let far_rough = super::river_valley_strength_for_sample_distance(
            far_distance,
            WorldPlanePoint::new(913.0, -211.0),
            flow,
            water_width,
            valley_width,
            radius,
            96.0,
        );

        assert!(
            min_rough <= base && max_rough > base,
            "roughness should pull some samples across the smooth core boundary: base={base} min={min_rough} max={max_rough}"
        );
        assert!(
            max_rough >= 0.88 && min_rough < 0.88,
            "roughened boundary should cross the heightfield river-water threshold: min={min_rough} max={max_rough}"
        );
        assert_eq!(
            far_base, far_rough,
            "river boundary roughness should fade before broad valley shoulder topology changes"
        );
    }

    #[test]
    fn downstream_river_high_core_uses_absolute_plan_water_width() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let flow = flow_hint(1024.0);
        let water_width = DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS;
        let inside = river_core_strength_for_effective_distance(99.0, flow, water_width, radius);
        let outside = river_core_strength_for_effective_distance(106.0, flow, water_width, radius);

        assert!(
            inside >= 0.88,
            "downstream high-core width should reach about the 200-block absolute target: {inside}"
        );
        assert!(
            outside < 0.88,
            "samples beyond the absolute water-width target should fall into the broad valley shoulder: {outside}"
        );
    }

    #[test]
    fn river_core_and_shoulder_profiles_are_distinct() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let flow = flow_hint(1024.0);
        let water_width = DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS;
        let valley_width = 520.0;
        let water_radius = super::river_water_radius_blocks(flow, water_width, radius);
        let shoulder_distance = water_radius + 64.0;

        let core = super::river_core_strength_for_effective_distance(
            shoulder_distance,
            flow,
            water_width,
            radius,
        );
        let shoulder = river_valley_strength_for_distance(
            shoulder_distance,
            flow,
            water_width,
            valley_width,
            radius,
        );

        assert_eq!(
            core, 0.0,
            "samples outside the planned water/bed radius must not be river core"
        );
        assert!(
            shoulder > 0.0,
            "the same sample can remain part of the broad valley shoulder: {shoulder}"
        );
    }

    #[test]
    fn downstream_shoulder_radius_and_strength_cap_to_half_planned_valley() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let flow = flow_hint(1024.0);
        let water_width = DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS;
        let planned_valley_width = 520.0;
        let shoulder_radius = river_shoulder_radius_blocks(flow, planned_valley_width, radius);
        let core = river_core_strength_for_effective_distance(99.0, flow, water_width, radius);
        let shoulder_at_bank = river_valley_strength_for_distance(
            101.0,
            flow,
            water_width,
            planned_valley_width,
            radius,
        );
        let outside_old_planned = river_valley_strength_for_distance(
            planned_valley_width * 0.55,
            flow,
            water_width,
            planned_valley_width,
            radius,
        );

        assert!(
            shoulder_radius <= planned_valley_width * 0.52,
            "downstream broad-valley shoulder radius should cap near half the planned scale: {shoulder_radius}"
        );
        assert!(
            core >= 0.88,
            "water/core corridor must keep the absolute downstream target: {core}"
        );
        assert!(
            shoulder_at_bank <= 0.52,
            "non-core shoulder strength should be capped below core strength: {shoulder_at_bank}"
        );
        assert_eq!(
            outside_old_planned, 0.0,
            "old downstream shoulder tail should no longer influence beyond the capped radius"
        );
    }

    #[test]
    fn river_valley_uses_flow_scaled_width() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let distance = 180.0;
        let headwater =
            river_valley_strength_for_distance(distance, flow_hint(12.0), 4.0, 12.0, radius);
        let trunk = river_valley_strength_for_distance(
            distance,
            flow_hint(1024.0),
            DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS,
            520.0,
            radius,
        );

        assert!(
            trunk > headwater,
            "a downstream trunk should still carve at distances where a headwater has faded: headwater={headwater} trunk={trunk}"
        );
        assert!(
            headwater <= 0.02,
            "headwater carve should fade quickly instead of using a fixed wide corridor: {headwater}"
        );
    }
}
