use super::types::MacroFieldTileConfig;
use crate::world::generation::graph::WorldPlanePoint;
use crate::world::generation::macro_map::{MacroSite, MacroSurfaceKind};

pub(super) const RIDGE_INFLUENCE_VISIBLE_FLOOR: f32 = 0.12;
pub(super) fn lake_boundary_lowering_factor(
    primary: MacroSite,
    distance_to_curve_blocks: f32,
    blend_radius_blocks: f32,
    roughness_offset_blocks: f32,
) -> f32 {
    let roughened_distance = (distance_to_curve_blocks + roughness_offset_blocks).max(0.0);
    let away_from_boundary =
        smoothstep01(roughened_distance / blend_radius_blocks.max(f32::EPSILON));
    if is_lake_surface(primary.surface_kind) {
        away_from_boundary
    } else {
        0.0
    }
}

pub(super) fn combine_macro_height(
    macro_elevation: f32,
    ocean_mask: f32,
    _coast_mask: f32,
    _lake_mask: f32,
    _dry_basin_mask: f32,
    lake_lowering_factor: f32,
    ridge_influence: f32,
    river_shoulder_strength: f32,
    river_flow_hint: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    combine_macro_height_with_river_longitudinal(
        macro_elevation,
        ocean_mask,
        _coast_mask,
        _lake_mask,
        _dry_basin_mask,
        lake_lowering_factor,
        ridge_influence,
        river_shoulder_strength,
        river_flow_hint,
        None,
        f32::NAN,
        config,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn combine_macro_height_with_river_longitudinal(
    macro_elevation: f32,
    ocean_mask: f32,
    _coast_mask: f32,
    _lake_mask: f32,
    _dry_basin_mask: f32,
    lake_lowering_factor: f32,
    ridge_influence: f32,
    river_shoulder_strength: f32,
    river_flow_hint: f32,
    river_centerline_macro_elevation: Option<f32>,
    river_longitudinal_blocks: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    let ridge_raise = ridge_influence * config.ridge_height_scale;
    let river_context_height = river_shoulder_context_height(
        macro_elevation,
        river_shoulder_strength,
        river_flow_hint,
        river_centerline_macro_elevation,
        river_longitudinal_blocks,
        config,
    );
    let mut height = river_context_height + ridge_raise;
    if ocean_mask > 0.5 {
        height = ocean_bathymetry_macro_height(height);
    } else if lake_lowering_factor > 0.0 {
        height = lake_bed_macro_height(height, lake_lowering_factor, config);
    }
    height.clamp(-2.0, 2.0)
}

pub(super) fn river_shoulder_context_height(
    macro_elevation: f32,
    river_shoulder_strength: f32,
    river_flow_hint: f32,
    river_centerline_macro_elevation: Option<f32>,
    river_longitudinal_blocks: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    let shoulder = river_shoulder_height_strength(river_shoulder_strength);
    if shoulder <= f32::EPSILON {
        return macro_elevation;
    }

    let flow_t = smoothstep01(river_flow_hint.clamp(0.0, 1.0));
    let shoulder_t = shoulder * lerp(0.18, 1.0, flow_t);
    let centerline_elevation = river_centerline_macro_elevation
        .filter(|height| height.is_finite())
        .unwrap_or(macro_elevation)
        .min(macro_elevation);
    let centerline_drop = (macro_elevation - centerline_elevation).max(0.0);
    let positive_relief = macro_elevation.max(0.0);
    let relief_compression = positive_relief * lerp(0.02, 0.08, flow_t);
    let lowland_bias = config.river_carve_scale * lerp(0.06, 0.55, flow_t);
    let below_sea_bias = (-macro_elevation).max(0.0) * lerp(0.0, 0.10, flow_t);
    let near_sea_t = 1.0 - smoothstep_range(0.0, 0.025, macro_elevation.max(0.0));
    let near_sea_bias = config.river_carve_scale * lerp(0.0, 0.35, flow_t) * near_sea_t;
    let centerline_pull = centerline_drop * shoulder_t * lerp(0.01, 0.06, flow_t);
    let _ = river_longitudinal_blocks;
    let broad_lowering =
        (relief_compression + lowland_bias + below_sea_bias + near_sea_bias) * shoulder_t;
    let max_context_shift = config.river_carve_scale * lerp(0.45, 2.4, flow_t)
        + centerline_drop * lerp(0.02, 0.12, flow_t);
    let lowering = (broad_lowering + centerline_pull).min(max_context_shift);

    (macro_elevation - lowering).min(macro_elevation)
}

pub(super) fn river_shoulder_height_strength(strength: f32) -> f32 {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= f32::EPSILON {
        return 0.0;
    }
    smoothstep01(strength).powf(1.35)
}

pub(super) fn ocean_bathymetry_macro_height(source_height: f32) -> f32 {
    if source_height >= 0.0 {
        return source_height;
    }

    let depth = (-source_height).max(0.0).clamp(0.0, 1.0);
    if depth <= 0.08 {
        return source_height;
    }

    let shallow_continuity = 0.08;
    let shelf = smoothstep_range(0.08, 0.18, depth) * 0.04;
    let slope = smoothstep_range(0.18, 0.52, depth) * 0.43;
    let basin = smoothstep_range(0.52, 0.92, depth) * 0.45;

    -(shallow_continuity + shelf + slope + basin).clamp(0.0, 1.0)
}

pub(super) fn lake_bed_macro_height(
    source_height: f32,
    lake_lowering_factor: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    let lake_t = lake_lowering_factor.clamp(0.0, 1.0);
    let u_shape = lake_t * lake_t * (3.0 - 2.0 * lake_t);
    let depth = 0.0025 + u_shape * 0.015;
    let target = source_height - depth;
    let preserve_relief = 1.0 - config.lake_flatten_strength.clamp(0.0, 1.0) * 0.42;

    target + (source_height - target) * preserve_relief
}

pub(super) fn roughened_distance(
    distance_blocks: f32,
    position: WorldPlanePoint,
    roughness_blocks: f32,
    salt: u64,
) -> f32 {
    if !distance_blocks.is_finite() || roughness_blocks <= 0.0 {
        return distance_blocks;
    }
    (distance_blocks + boundary_roughness_offset(position, roughness_blocks, salt)).max(0.0)
}

pub(super) fn boundary_roughness_offset(
    position: WorldPlanePoint,
    roughness_blocks: f32,
    salt: u64,
) -> f32 {
    if roughness_blocks <= 0.0 {
        return 0.0;
    }
    let broad = smooth_value_noise_2d(position, 96.0, salt);
    let medium = smooth_value_noise_2d(
        WorldPlanePoint::new(position.x + 37.0, position.z - 61.0),
        41.0,
        salt ^ 0x9E37_79B9_7F4A_7C15,
    );
    let noise = (broad * 0.58 + medium * 0.42).clamp(-1.0, 1.0);
    let shaped = noise.signum() * noise.abs().powf(0.65);
    shaped * roughness_blocks
}

pub(super) fn smooth_value_noise_2d(
    position: WorldPlanePoint,
    scale_blocks: f32,
    salt: u64,
) -> f32 {
    let scale = scale_blocks.max(1.0);
    let x = position.x / scale;
    let z = position.z / scale;
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let tx = smootherstep(x - x0 as f32);
    let tz = smootherstep(z - z0 as f32);
    let a = signed_lattice_noise(x0, z0, salt);
    let b = signed_lattice_noise(x0 + 1, z0, salt);
    let c = signed_lattice_noise(x0, z0 + 1, salt);
    let d = signed_lattice_noise(x0 + 1, z0 + 1, salt);
    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * tz
}

pub(super) fn signed_lattice_noise(x: i32, z: i32, salt: u64) -> f32 {
    let mut value = salt;
    value ^= (x as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= (z as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    let unit = ((value ^ (value >> 31)) as f64 / u64::MAX as f64) as f32;
    unit * 2.0 - 1.0
}

pub(super) fn smootherstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
pub(super) fn is_lake_surface(kind: MacroSurfaceKind) -> bool {
    matches!(
        kind,
        MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
    )
}

pub(super) fn envelope(distance: f32, radius: f32) -> f32 {
    if !distance.is_finite() {
        return 0.0;
    }
    let t = (1.0 - distance / radius.max(f32::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn smoothstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    let span = (edge1 - edge0).max(f32::EPSILON);
    smoothstep01((value - edge0) / span)
}

pub(super) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub(super) fn ridge_envelope(distance: f32, radius: f32) -> f32 {
    let raw = envelope(distance, radius);
    if raw <= RIDGE_INFLUENCE_VISIBLE_FLOOR {
        0.0
    } else {
        let t = ((raw - RIDGE_INFLUENCE_VISIBLE_FLOOR) / (1.0 - RIDGE_INFLUENCE_VISIBLE_FLOOR))
            .clamp(0.0, 1.0);
        (t * t * (3.0 - 2.0 * t)).powf(0.92)
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
    use crate::world::generation::macro_field::test_support::*;
    use crate::world::generation::macro_field::{
        MacroFieldRasterContext, MacroFieldTileConfig, generate_macro_field_tile,
        sample_macro_field_point,
    };
    use crate::world::generation::macro_map::{
        GraphMacroMap, MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
    };
    use crate::world::generation::river_plan::{
        DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS, RiverPlan,
    };

    #[test]
    fn ocean_combined_height_preserves_shelf_slope_basin_depth() {
        let config = test_tile_config();
        let shelf = combine_macro_height(-0.08, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let slope = combine_macro_height(-0.32, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let basin = combine_macro_height(-0.75, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);

        assert!(
            shelf > slope && slope > basin,
            "ocean bathymetry should deepen from shelf to slope to basin: shelf={shelf} slope={slope} basin={basin}"
        );
        assert!(
            (shelf - basin).abs() > 0.35,
            "ocean bathymetry should not collapse near sea level: shelf={shelf} basin={basin}"
        );
        assert!(
            slope < -0.12,
            "continental slope should remain visibly below shallow shelf: slope={slope}"
        );
    }

    #[test]
    fn ocean_bathymetry_keeps_coast_adjacent_depth_continuous() {
        let source = -0.004;
        let bathymetry = ocean_bathymetry_macro_height(source);

        assert!(
            (bathymetry - source).abs() < 0.002,
            "coast-adjacent ocean should not jump to a fixed shallow shelf: source={source} bathymetry={bathymetry}"
        );
    }

    #[test]
    fn ocean_bathymetry_preserves_positive_coast_adjacent_source() {
        let source = 0.018;
        let bathymetry = ocean_bathymetry_macro_height(source);

        assert_eq!(
            bathymetry, source,
            "ocean-owned positive source terrain should stay above sea level until heightfield water policy decides coverage"
        );
    }

    #[test]
    fn ocean_bathymetry_preserves_shallow_negative_source_continuity() {
        for source in [-0.012, -0.04, -0.079] {
            let bathymetry = ocean_bathymetry_macro_height(source);

            assert_eq!(
                bathymetry, source,
                "shallow ocean source should continue the signed source field without shelf snapping"
            );
        }
    }

    #[test]
    fn ocean_bathymetry_uses_narrow_shelf_before_slope() {
        let near_coast = ocean_bathymetry_macro_height(-0.03);
        let shelf_edge = ocean_bathymetry_macro_height(-0.08);
        let slope = ocean_bathymetry_macro_height(-0.42);

        assert!(
            shelf_edge < near_coast - 0.025,
            "shelf should narrow quickly after the coast: near={near_coast} shelf_edge={shelf_edge}"
        );
        assert!(
            slope < shelf_edge - 0.2,
            "continental slope should deepen soon after the narrowed shelf: shelf_edge={shelf_edge} slope={slope}"
        );
    }

    #[test]
    fn ocean_bathymetry_no_longer_uses_lake_flatten_strength() {
        let mut weak_flatten = test_tile_config();
        weak_flatten.lake_flatten_strength = 0.0;
        let mut strong_flatten = test_tile_config();
        strong_flatten.lake_flatten_strength = 1.0;

        let weak =
            combine_macro_height(-0.62, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, weak_flatten);
        let strong = combine_macro_height(
            -0.62,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            strong_flatten,
        );

        assert_eq!(weak, strong);
    }

    #[test]
    fn lake_lowering_still_uses_lake_bed_macro_height() {
        let config = test_tile_config();
        let source = 0.18;
        let factor = 0.72;
        let expected = lake_bed_macro_height(source, factor, config);
        let actual =
            combine_macro_height(source, 0.0, 0.0, 1.0, 0.0, factor, 0.0, 0.0, 0.0, config);

        assert_eq!(actual, expected);
    }

    #[test]
    fn coast_mask_does_not_change_combined_height_but_lake_and_river_still_do() {
        let config = test_tile_config();
        let base = combine_macro_height(0.42, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let coast = combine_macro_height(0.42, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let lake = combine_macro_height(0.42, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, config);
        let river = combine_macro_height(0.42, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, config);

        assert_eq!(
            base, coast,
            "coast_mask remains diagnostic/downstream policy data and must not flatten combined height"
        );
        assert!(
            lake < base,
            "lake flatten should still lower combined height: lake={lake} base={base}"
        );
        assert!(
            river < base,
            "river broad-valley context should still lower combined height: river={river} base={base}"
        );
        assert_eq!(
            config.river_carve_scale, 0.012,
            "default broad-valley context shift should stay block-scale and leave bed depth to heightfield"
        );
    }

    #[test]
    fn river_shoulder_context_reads_centerline_macro_elevation() {
        let config = test_tile_config();
        let without_centerline =
            river_shoulder_context_height(0.48, 0.92, 0.72, None, f32::NAN, config);
        let with_lower_centerline =
            river_shoulder_context_height(0.48, 0.92, 0.72, Some(0.28), f32::NAN, config);
        let with_higher_centerline =
            river_shoulder_context_height(0.48, 0.92, 0.72, Some(0.72), f32::NAN, config);

        assert!(
            with_lower_centerline < without_centerline - config.river_carve_scale,
            "river shoulder should blend toward the projected centerline elevation so contour context follows the river axis: without={without_centerline} with={with_lower_centerline}"
        );
        assert_eq!(
            with_higher_centerline, without_centerline,
            "centerline context may lower banks toward the river floor but must not raise shoulder terrain"
        );
    }

    #[test]
    fn river_shoulder_context_preserves_cross_section_source_relief() {
        let config = test_tile_config();
        let low_source =
            river_shoulder_context_height(0.028, 0.44, 0.82, Some(0.020), f32::NAN, config);
        let high_source =
            river_shoulder_context_height(0.036, 0.44, 0.82, Some(0.020), f32::NAN, config);

        assert!(
            high_source - low_source > 0.004,
            "river shoulder context should lower the valley without flattening cross-section source relief into a contour slab: low={low_source} high={high_source}"
        );
    }

    #[test]
    fn weak_river_shoulder_tail_is_continuous_but_attenuated_for_height() {
        let config = test_tile_config();
        let base = river_shoulder_context_height(0.34, 0.0, 0.8, Some(0.12), 384.0, config);
        let weak = river_shoulder_context_height(0.34, 0.18, 0.8, Some(0.12), 384.0, config);
        let active = river_shoulder_context_height(0.34, 0.74, 0.8, Some(0.12), 384.0, config);

        let weak_shift = base - weak;
        let active_shift = base - active;
        assert!(
            weak_shift > 0.0,
            "weak shoulder tails should remain continuous"
        );
        assert!(
            weak_shift < active_shift * 0.20,
            "weak broad-shoulder tails should be strongly attenuated without creating a hard contour cutoff"
        );
        assert!(
            active < base,
            "active river shoulder should still lower combined height near the selected river corridor"
        );
    }

    #[test]
    fn river_shoulder_height_does_not_step_on_longitudinal_hint_switch() {
        let config = test_tile_config();
        let left_hint =
            river_shoulder_context_height(0.036, 0.90, 0.75, Some(0.024), 116.0, config);
        let right_hint =
            river_shoulder_context_height(0.036, 0.90, 0.75, Some(0.024), 172.0, config);

        assert_eq!(
            left_hint, right_hint,
            "longitudinal hints are diagnostics/downstream hints; combined macro height must not form a vertical seam when nearest river segment ownership switches"
        );
    }

    #[test]
    fn ocean_owned_broad_river_valley_can_lower_positive_source_below_sea_level() {
        let config = test_tile_config();
        let carved = combine_macro_height(0.01, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, config);

        assert!(
            carved < 0.0,
            "selected river broad-valley context should affect ocean-owned above-sea terrain: {carved}"
        );
    }

    #[test]
    fn lake_lowering_carves_a_rounded_bed_without_flattening_to_one_height() {
        let config = test_tile_config();
        let source = 0.18;
        let shore = combine_macro_height(source, 0.0, 0.0, 1.0, 0.0, 0.25, 0.0, 0.0, 0.0, config);
        let slope = combine_macro_height(source, 0.0, 0.0, 1.0, 0.0, 0.65, 0.0, 0.0, 0.0, config);
        let center = combine_macro_height(source, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, config);
        let higher_source =
            combine_macro_height(0.24, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, config);

        assert!(source > shore && shore > slope && slope > center);
        assert!(
            higher_source > center,
            "lake bed should preserve source relief instead of collapsing all lake interiors to a flat target"
        );
    }

    #[test]
    fn dry_basin_mask_does_not_add_macro_field_lowering() {
        let config = test_tile_config();
        let base = combine_macro_height(0.18, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let dry_height = combine_macro_height(0.18, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, config);
        let water_height =
            combine_macro_height(0.18, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, config);

        assert_eq!(
            dry_height, base,
            "dry basin mask should preserve macro elevation instead of adding a floor/lowering profile"
        );
        assert!(
            dry_height > water_height,
            "dry basin should not use lake/ocean water flatten: dry={dry_height} water={water_height}"
        );
    }

    #[test]
    fn lake_lowering_factor_transitions_across_boundary() {
        let lake = test_site(
            crate::world::generation::graph::VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::LakeCandidate,
            -0.04,
        );
        let land = test_site(
            crate::world::generation::graph::VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.24,
        );
        let radius = 32.0;
        let lake_edge = lake_boundary_lowering_factor(lake, 0.0, radius, 0.0);
        let land_edge = lake_boundary_lowering_factor(land, 0.0, radius, 0.0);
        let lake_interior = lake_boundary_lowering_factor(lake, radius, radius, 0.0);
        let land_exterior = lake_boundary_lowering_factor(land, radius, radius, 0.0);

        assert_eq!(lake_edge, 0.0);
        assert_eq!(land_edge, 0.0);
        assert!(lake_interior > lake_edge);
        assert_eq!(lake_interior, 1.0);
        assert_eq!(land_exterior, 0.0);
    }

    #[test]
    fn boundary_roughness_perturbs_visible_distance_without_changing_owner_masks() {
        let position = WorldPlanePoint::new(37.0, -91.0);
        let smooth = roughened_distance(48.0, position, 0.0, 17);
        let rough = roughened_distance(48.0, position, 96.0, 17);

        assert_eq!(smooth, 48.0);
        assert_ne!(rough, smooth);
        assert!(
            (rough - smooth).abs() <= 96.0,
            "boundary roughness should stay bounded: smooth={smooth} rough={rough}"
        );
    }

    #[test]
    fn default_combined_height_does_not_apply_ridge_raise() {
        let config = MacroFieldTileConfig::new(0.0, 0.0, 1, 1, 32.0);
        let without_ridge =
            combine_macro_height(0.20, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);
        let with_ridge_influence =
            combine_macro_height(0.20, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, config);

        assert_eq!(
            config.ridge_height_scale, 0.0,
            "launch macro field keeps ridge influence diagnostic-only until broad mountain elevation is reintroduced"
        );
        assert_eq!(
            without_ridge, with_ridge_influence,
            "ridge influence should not create pinpoint combined-height maxima while ridge raise is disabled"
        );
    }

    #[test]
    fn combined_macro_height_is_lower_near_river_curve_than_far_terrain() {
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
            near_sample.combined_macro_height < far_sample.combined_macro_height,
            "river broad-valley lowering should be visible in combined macro height: near={} far={}",
            near_sample.combined_macro_height,
            far_sample.combined_macro_height
        );
    }
}
