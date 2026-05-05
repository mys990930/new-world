use rayon::prelude::*;
use std::collections::HashMap;

use super::graph::{
    VoronoiCornerId, VoronoiEdge, VoronoiEdgeId, VoronoiGraphPatch, VoronoiSiteId, WorldPlanePoint,
};
use super::macro_map::{GraphMacroMap, MacroEdge, MacroLakeEdgeClass, MacroSite};

pub const DEFAULT_BOUNDARY_SUBDIVISION_LEVELS: u8 = 6;
pub const DEFAULT_BOUNDARY_GUARD_MARGIN_BLOCKS: f32 = 4.0;
pub const DEFAULT_BOUNDARY_MIN_VISIBLE_AMPLITUDE_BLOCKS: f32 = 36.0;
pub const DEFAULT_BOUNDARY_MAX_VISIBLE_AMPLITUDE_BLOCKS: f32 = 192.0;
pub const DEFAULT_BOUNDARY_MAX_EDGE_FRACTION: f32 = 0.38;
pub const DEFAULT_BOUNDARY_MAX_SITE_SPAN_FRACTION: f32 = 0.48;

const HASH_BOUNDARY: u64 = 0xb31d_0f9c_53a7_8e21;
const PROFILE_SALT_ORDINARY: u64 = 0x00ed_6e00_5eed_0000;
const PROFILE_SALT_COAST: u64 = 0x01c0_a57e_5eed_1001;
const PROFILE_SALT_RIDGE: u64 = 0x03a1_7d6e_5eed_1003;
const PROFILE_SALT_FAULT: u64 = 0x04fa_0175_5eed_1004;
const PROFILE_SALT_LAKE: u64 = 0x05aa_1e5d_5eed_1005;
const PROFILE_SALT_LAND_SEAM: u64 = 0x06b0_0d1e_5eed_1006;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryConfig {
    pub seed: u64,
    pub generator_version: u32,
    pub subdivision_levels: u8,
    pub guard_margin_blocks: f32,
    pub min_visible_amplitude_blocks: f32,
    pub max_visible_amplitude_blocks: f32,
    pub max_edge_fraction: f32,
    pub max_site_span_fraction: f32,
    pub ordinary_amplitude: f32,
    pub coast_amplitude: f32,
    pub ridge_amplitude: f32,
    pub fault_amplitude: f32,
    pub lake_amplitude: f32,
    pub land_seam_amplitude: f32,
}

impl BoundaryConfig {
    pub const fn new(seed: u64, generator_version: u32) -> Self {
        Self {
            seed,
            generator_version,
            subdivision_levels: DEFAULT_BOUNDARY_SUBDIVISION_LEVELS,
            guard_margin_blocks: DEFAULT_BOUNDARY_GUARD_MARGIN_BLOCKS,
            min_visible_amplitude_blocks: DEFAULT_BOUNDARY_MIN_VISIBLE_AMPLITUDE_BLOCKS,
            max_visible_amplitude_blocks: DEFAULT_BOUNDARY_MAX_VISIBLE_AMPLITUDE_BLOCKS,
            max_edge_fraction: DEFAULT_BOUNDARY_MAX_EDGE_FRACTION,
            max_site_span_fraction: DEFAULT_BOUNDARY_MAX_SITE_SPAN_FRACTION,
            ordinary_amplitude: 0.20,
            coast_amplitude: 0.44,
            ridge_amplitude: 0.34,
            fault_amplitude: 0.24,
            lake_amplitude: 0.36,
            land_seam_amplitude: 0.24,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BoundaryProfile {
    Ordinary,
    Coast,
    Ridge,
    Fault,
    Lake,
    LandSeam,
}

impl BoundaryProfile {
    pub const fn salt(self) -> u64 {
        match self {
            Self::Ordinary => PROFILE_SALT_ORDINARY,
            Self::Coast => PROFILE_SALT_COAST,
            Self::Ridge => PROFILE_SALT_RIDGE,
            Self::Fault => PROFILE_SALT_FAULT,
            Self::Lake => PROFILE_SALT_LAKE,
            Self::LandSeam => PROFILE_SALT_LAND_SEAM,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryAnchors {
    pub corners: [VoronoiCornerId; 2],
    pub sites: [VoronoiSiteId; 2],
    pub start: WorldPlanePoint,
    pub end: WorldPlanePoint,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryGuard {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
}

impl BoundaryGuard {
    pub fn contains(self, point: WorldPlanePoint) -> bool {
        point.x >= self.min_x
            && point.x <= self.max_x
            && point.z >= self.min_z
            && point.z <= self.max_z
    }

    fn clamp(self, point: WorldPlanePoint) -> WorldPlanePoint {
        WorldPlanePoint::new(
            point.x.clamp(self.min_x, self.max_x),
            point.z.clamp(self.min_z, self.max_z),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoisyBoundaryCurve {
    pub edge: VoronoiEdgeId,
    pub profile: BoundaryProfile,
    pub anchors: BoundaryAnchors,
    pub points: Vec<WorldPlanePoint>,
    pub amplitude: f32,
    pub seed: u64,
    pub guard: BoundaryGuard,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BoundaryStats {
    pub total_curve_count: usize,
    pub ordinary_curve_count: usize,
    pub coast_curve_count: usize,
    pub ridge_curve_count: usize,
    pub fault_curve_count: usize,
    pub lake_curve_count: usize,
    pub land_seam_curve_count: usize,
    pub guard_violation_count: usize,
    pub missing_macro_edge_count: usize,
    pub average_amplitude_blocks: f32,
    pub max_amplitude_blocks: f32,
    pub average_perpendicular_displacement_blocks: f32,
    pub max_perpendicular_displacement_blocks: f32,
    pub nearly_straight_curve_count: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoundaryCache {
    pub curves: Vec<NoisyBoundaryCurve>,
    pub stats: BoundaryStats,
}

impl BoundaryCache {
    pub fn curve_for_edge(&self, edge: VoronoiEdgeId) -> Option<&NoisyBoundaryCurve> {
        self.curves.iter().find(|curve| curve.edge == edge)
    }
}

pub fn generate_noisy_boundaries(
    patch: &VoronoiGraphPatch,
    macro_map: &GraphMacroMap,
    config: BoundaryConfig,
) -> BoundaryCache {
    validate_boundary_config(config);

    let corner_positions = patch
        .corners
        .iter()
        .map(|corner| (corner.id, corner.position))
        .collect::<HashMap<_, _>>();
    let site_positions = patch
        .sites
        .iter()
        .map(|site| (site.id, site.position))
        .collect::<HashMap<_, _>>();
    let macro_edges = macro_map
        .edges
        .iter()
        .map(|edge| (edge.id, *edge))
        .collect::<HashMap<_, _>>();
    let macro_sites = macro_map
        .sites
        .iter()
        .map(|site| (site.id, *site))
        .collect::<HashMap<_, _>>();

    let mut curves = patch
        .edges
        .par_iter()
        .filter_map(|edge| {
            let macro_edge = macro_edges.get(&edge.id).copied()?;
            build_curve_for_edge(
                *edge,
                macro_edge,
                &macro_sites,
                &corner_positions,
                &site_positions,
                config,
            )
        })
        .collect::<Vec<_>>();

    curves.sort_by_key(|curve| curve.edge.0);
    let stats = boundary_stats(&curves, patch.edges.len());

    BoundaryCache { curves, stats }
}

fn validate_boundary_config(config: BoundaryConfig) {
    assert!(
        config.subdivision_levels <= 8,
        "subdivision levels must stay small enough for preview/runtime cache"
    );
    assert!(
        config.guard_margin_blocks.is_finite() && config.guard_margin_blocks >= 0.0,
        "guard margin must be finite and >= 0"
    );
    assert!(
        config.min_visible_amplitude_blocks.is_finite()
            && config.min_visible_amplitude_blocks >= 0.0,
        "min visible amplitude must be finite and >= 0"
    );
    assert!(
        config.max_visible_amplitude_blocks.is_finite()
            && config.max_visible_amplitude_blocks >= config.min_visible_amplitude_blocks,
        "max visible amplitude must be finite and >= min visible amplitude"
    );
    assert!(
        config.max_edge_fraction.is_finite() && (0.0..=0.5).contains(&config.max_edge_fraction),
        "max edge fraction must be finite in 0..=0.5"
    );
    assert!(
        config.max_site_span_fraction.is_finite()
            && (0.0..=0.5).contains(&config.max_site_span_fraction),
        "max site span fraction must be finite in 0..=0.5"
    );
    for value in [
        config.ordinary_amplitude,
        config.coast_amplitude,
        config.ridge_amplitude,
        config.fault_amplitude,
        config.lake_amplitude,
        config.land_seam_amplitude,
    ] {
        assert!(
            value.is_finite() && (0.0..=0.5).contains(&value),
            "boundary amplitude must be finite in 0..=0.5"
        );
    }
}

fn build_curve_for_edge(
    edge: VoronoiEdge,
    macro_edge: MacroEdge,
    macro_sites: &HashMap<VoronoiSiteId, MacroSite>,
    corner_positions: &HashMap<VoronoiCornerId, WorldPlanePoint>,
    site_positions: &HashMap<VoronoiSiteId, WorldPlanePoint>,
    config: BoundaryConfig,
) -> Option<NoisyBoundaryCurve> {
    let start = *corner_positions.get(&edge.corners[0])?;
    let end = *corner_positions.get(&edge.corners[1])?;
    let site_a = *site_positions.get(&edge.sites[0])?;
    let site_b = *site_positions.get(&edge.sites[1])?;
    let profile = boundary_profile(macro_edge, macro_sites);
    let seed = curve_seed(config, edge.id, profile);
    let amplitude = amplitude_for_profile(profile, config);
    let guard = boundary_guard(start, end, site_a, site_b, config.guard_margin_blocks);
    let amplitude_blocks = visible_amplitude_blocks(start, end, site_a, site_b, amplitude, config);
    let points = noisy_midpoint_curve(
        start,
        end,
        site_a,
        site_b,
        seed,
        amplitude_blocks,
        config.subdivision_levels,
        guard,
    );

    Some(NoisyBoundaryCurve {
        edge: edge.id,
        profile,
        anchors: BoundaryAnchors {
            corners: macro_edge.corners,
            sites: macro_edge.sites,
            start,
            end,
        },
        points,
        amplitude: amplitude_blocks,
        seed,
        guard,
    })
}

fn boundary_profile(
    edge: MacroEdge,
    macro_sites: &HashMap<VoronoiSiteId, MacroSite>,
) -> BoundaryProfile {
    if edge.guide.is_coast {
        return BoundaryProfile::Coast;
    }
    if edge.lake_class != MacroLakeEdgeClass::NonLake {
        return BoundaryProfile::Lake;
    }
    if edge.guide.is_ridge_candidate {
        return BoundaryProfile::Ridge;
    }
    if edge.guide.is_fault_candidate {
        return BoundaryProfile::Fault;
    }
    if is_land_seam(edge, macro_sites) {
        return BoundaryProfile::LandSeam;
    }
    BoundaryProfile::Ordinary
}

fn is_land_seam(edge: MacroEdge, macro_sites: &HashMap<VoronoiSiteId, MacroSite>) -> bool {
    let Some(a) = macro_sites.get(&edge.sites[0]).copied() else {
        return false;
    };
    let Some(b) = macro_sites.get(&edge.sites[1]).copied() else {
        return false;
    };
    a.surface_kind != b.surface_kind
        && a.surface_kind.is_land_owned()
        && b.surface_kind.is_land_owned()
        && !a.surface_kind.is_coast()
        && !b.surface_kind.is_coast()
}

fn noisy_midpoint_curve(
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    _site_a: WorldPlanePoint,
    _site_b: WorldPlanePoint,
    seed: u64,
    amplitude_blocks: f32,
    levels: u8,
    guard: BoundaryGuard,
) -> Vec<WorldPlanePoint> {
    let segment_count = 1_usize << levels;
    let mut points = Vec::with_capacity(segment_count + 1);
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
    let normal = WorldPlanePoint::new(-dz / length, dx / length);
    let lateral_limit = amplitude_blocks.max(0.0);
    let displacements = natural_displacement_series(seed, lateral_limit, segment_count);

    for index in 0..=segment_count {
        let t = index as f32 / segment_count as f32;
        let base = lerp_point(start, end, t);
        let point = if index == 0 || index == segment_count || lateral_limit <= f32::EPSILON {
            base
        } else {
            let offset = displacements[index];
            let first =
                WorldPlanePoint::new(base.x + normal.x * offset, base.z + normal.z * offset);
            let first_clamped = guard.clamp(first);
            let first_distance = perpendicular_distance_to_line(first_clamped, start, end);
            let minimum_useful_distance = (offset.abs() * 0.25).min(1.0);
            if first_distance >= minimum_useful_distance {
                first_clamped
            } else {
                WorldPlanePoint::new(base.x - normal.x * offset, base.z - normal.z * offset)
            }
        };
        points.push(guard.clamp(point));
    }

    points
}

fn natural_displacement_series(seed: u64, lateral_limit: f32, segment_count: usize) -> Vec<f32> {
    let mut values = (0..=segment_count)
        .map(|index| {
            let t = index as f32 / segment_count as f32;
            if index == 0 || index == segment_count || lateral_limit <= f32::EPSILON {
                return 0.0;
            }

            let envelope = endpoint_falloff(t);
            let low_phase = unit_f32(seed) * std::f32::consts::TAU;
            let mid_phase =
                unit_f32(splitmix64(seed ^ 0x8412_91c3_5a77_9021)) * std::f32::consts::TAU;
            let high_phase =
                unit_f32(splitmix64(seed ^ 0x2f2d_091d_a871_1943)) * std::f32::consts::TAU;
            let low_wave = (t * std::f32::consts::TAU * 1.15 + low_phase).sin() * 0.52;
            let mid_wave = (t * std::f32::consts::TAU * 2.65 + mid_phase).sin() * 0.31;
            let high_wave = (t * std::f32::consts::TAU * 5.20 + high_phase).sin() * 0.10;
            let coarse = smooth_value_noise(seed ^ 0xc01d_cafe_7a11_0001, t, 5) * 0.30;
            let fine = smooth_value_noise(seed ^ 0xf1b0_5eed_91ce_0002, t, 9) * 0.16;
            let signed = (low_wave + mid_wave + high_wave + coarse + fine).clamp(-1.0, 1.0);

            signed * lateral_limit * envelope
        })
        .collect::<Vec<_>>();

    for _ in 0..2 {
        values = smooth_displacements(&values);
    }

    values
}

fn endpoint_falloff(t: f32) -> f32 {
    let sine = (t * std::f32::consts::PI).sin().max(0.0);
    smoothstep(sine).powf(0.72)
}

fn smooth_value_noise(seed: u64, t: f32, knot_count: usize) -> f32 {
    debug_assert!(knot_count >= 2);
    let scaled = t.clamp(0.0, 1.0) * (knot_count - 1) as f32;
    let left = scaled.floor() as usize;
    let right = (left + 1).min(knot_count - 1);
    let local_t = smoothstep(scaled - left as f32);
    let a = signed_knot(seed, left);
    let b = signed_knot(seed, right);
    a + (b - a) * local_t
}

fn signed_knot(seed: u64, index: usize) -> f32 {
    unit_f32(splitmix64(
        seed ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15),
    )) * 2.0
        - 1.0
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn smooth_displacements(values: &[f32]) -> Vec<f32> {
    if values.len() <= 2 {
        return values.to_vec();
    }

    let mut smoothed = Vec::with_capacity(values.len());
    smoothed.push(0.0);
    for index in 1..values.len() - 1 {
        smoothed.push(values[index - 1] * 0.25 + values[index] * 0.50 + values[index + 1] * 0.25);
    }
    smoothed.push(0.0);
    smoothed
}

fn visible_amplitude_blocks(
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    site_a: WorldPlanePoint,
    site_b: WorldPlanePoint,
    amplitude: f32,
    config: BoundaryConfig,
) -> f32 {
    let edge_length = distance(start, end).max(1.0);
    let site_span = distance(site_a, site_b).max(1.0);
    let local_scale = edge_length.min(site_span);
    let desired = (local_scale * amplitude).max(config.min_visible_amplitude_blocks);
    let edge_limit = edge_length * config.max_edge_fraction;
    let site_limit = site_span * config.max_site_span_fraction;

    desired
        .min(edge_limit)
        .min(site_limit)
        .min(config.max_visible_amplitude_blocks)
        .max(0.0)
}

fn boundary_guard(
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    site_a: WorldPlanePoint,
    site_b: WorldPlanePoint,
    margin: f32,
) -> BoundaryGuard {
    let min_x = start.x.min(end.x).min(site_a.x).min(site_b.x) - margin;
    let max_x = start.x.max(end.x).max(site_a.x).max(site_b.x) + margin;
    let min_z = start.z.min(end.z).min(site_a.z).min(site_b.z) - margin;
    let max_z = start.z.max(end.z).max(site_a.z).max(site_b.z) + margin;
    BoundaryGuard {
        min_x,
        max_x,
        min_z,
        max_z,
    }
}

fn curve_seed(config: BoundaryConfig, edge: VoronoiEdgeId, profile: BoundaryProfile) -> u64 {
    let mut state = splitmix64(config.seed ^ HASH_BOUNDARY);
    state = splitmix64(state ^ u64::from(config.generator_version));
    state = splitmix64(state ^ edge.0);
    splitmix64(state ^ profile.salt())
}

fn amplitude_for_profile(profile: BoundaryProfile, config: BoundaryConfig) -> f32 {
    match profile {
        BoundaryProfile::Ordinary => config.ordinary_amplitude,
        BoundaryProfile::Coast => config.coast_amplitude,
        BoundaryProfile::Ridge => config.ridge_amplitude,
        BoundaryProfile::Fault => config.fault_amplitude,
        BoundaryProfile::Lake => config.lake_amplitude,
        BoundaryProfile::LandSeam => config.land_seam_amplitude,
    }
}

fn boundary_stats(curves: &[NoisyBoundaryCurve], graph_edge_count: usize) -> BoundaryStats {
    let mut stats = BoundaryStats {
        total_curve_count: curves.len(),
        missing_macro_edge_count: graph_edge_count.saturating_sub(curves.len()),
        ..BoundaryStats::default()
    };
    let mut amplitude_sum = 0.0;
    let mut displacement_sum = 0.0;

    for curve in curves {
        match curve.profile {
            BoundaryProfile::Ordinary => stats.ordinary_curve_count += 1,
            BoundaryProfile::Coast => stats.coast_curve_count += 1,
            BoundaryProfile::Ridge => stats.ridge_curve_count += 1,
            BoundaryProfile::Fault => stats.fault_curve_count += 1,
            BoundaryProfile::Lake => stats.lake_curve_count += 1,
            BoundaryProfile::LandSeam => stats.land_seam_curve_count += 1,
        }
        if curve
            .points
            .iter()
            .any(|point| !curve.guard.contains(*point))
        {
            stats.guard_violation_count += 1;
        }
        amplitude_sum += curve.amplitude;
        stats.max_amplitude_blocks = stats.max_amplitude_blocks.max(curve.amplitude);
        let displacement = max_perpendicular_displacement(curve);
        displacement_sum += displacement;
        stats.max_perpendicular_displacement_blocks = stats
            .max_perpendicular_displacement_blocks
            .max(displacement);
        if curve_chord_length(curve) >= 8.0 && displacement < 1.0 {
            stats.nearly_straight_curve_count += 1;
        }
    }

    if !curves.is_empty() {
        let count = curves.len() as f32;
        stats.average_amplitude_blocks = amplitude_sum / count;
        stats.average_perpendicular_displacement_blocks = displacement_sum / count;
    }

    stats
}

fn max_perpendicular_displacement(curve: &NoisyBoundaryCurve) -> f32 {
    curve
        .points
        .iter()
        .skip(1)
        .take(curve.points.len().saturating_sub(2))
        .map(|point| perpendicular_distance_to_line(*point, curve.anchors.start, curve.anchors.end))
        .fold(0.0, f32::max)
}

fn curve_chord_length(curve: &NoisyBoundaryCurve) -> f32 {
    distance(curve.anchors.start, curve.anchors.end)
}

fn perpendicular_distance_to_line(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    let length = (dx * dx + dz * dz).sqrt();
    if length <= f32::EPSILON {
        return distance(point, start);
    }
    ((point.x - start.x) * dz - (point.z - start.z) * dx).abs() / length
}

fn lerp_point(a: WorldPlanePoint, b: WorldPlanePoint, t: f32) -> WorldPlanePoint {
    WorldPlanePoint::new(a.x + (b.x - a.x) * t, a.z + (b.z - a.z) * t)
}

fn distance(a: WorldPlanePoint, b: WorldPlanePoint) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn unit_f32(value: u64) -> f32 {
    const SCALE: f64 = 1.0 / ((1u64 << 53) as f64);
    ((value >> 11) as f64 * SCALE) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::graph::{
        DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, VoronoiGraphConfig,
        VoronoiGraphPatchRequest, generate_voronoi_graph_patch,
    };
    use crate::world::generation::macro_map::{MacroMapConfig, generate_macro_map};

    #[test]
    fn boundary_generation_creates_one_curve_for_every_macro_edge() {
        let (patch, macro_map) = test_inputs(42, 0, 0);
        let boundary = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(42, 11));

        assert_eq!(boundary.curves.len(), macro_map.edges.len());
        assert_eq!(boundary.stats.total_curve_count, macro_map.edges.len());
        assert_eq!(boundary.stats.missing_macro_edge_count, 0);
        assert!(boundary.stats.ordinary_curve_count > 0);
    }

    #[test]
    fn boundary_generation_is_deterministic_for_same_input() {
        let (patch, macro_map) = test_inputs(42, 0, 0);
        let config = BoundaryConfig::new(42, 11);

        let first = generate_noisy_boundaries(&patch, &macro_map, config);
        let second = generate_noisy_boundaries(&patch, &macro_map, config);

        assert_eq!(first, second);
    }

    #[test]
    fn boundary_generation_changes_with_seed() {
        let (patch, macro_map) = test_inputs(42, 0, 0);
        let first = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(42, 11));
        let second = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(43, 11));

        assert_ne!(first.curves, second.curves);
    }

    #[test]
    fn noisy_points_stay_inside_edge_guard() {
        let (patch, macro_map) = test_inputs(42, 0, 0);
        let boundary = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(42, 11));

        assert_eq!(boundary.stats.guard_violation_count, 0);
        assert!(boundary.curves.iter().all(|curve| {
            curve
                .points
                .iter()
                .all(|point| curve.guard.contains(*point))
        }));
    }

    #[test]
    fn noisy_curves_have_visible_perpendicular_displacement() {
        let (patch, macro_map) = test_inputs(42, 0, 0);
        let boundary = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(42, 11));

        assert_eq!(boundary.stats.nearly_straight_curve_count, 0);
        assert!(
            boundary.stats.average_perpendicular_displacement_blocks >= 8.0,
            "default noisy edges should visibly bend in world space, got avg displacement {:.2}",
            boundary.stats.average_perpendicular_displacement_blocks
        );
        assert!(
            boundary
                .curves
                .iter()
                .filter(|curve| curve_chord_length(curve) >= 8.0)
                .all(|curve| max_perpendicular_displacement(curve) >= 1.0),
            "every canonical boundary curve should have non-collinear interior points"
        );
        assert!(
            boundary.curves.iter().all(|curve| curve.amplitude > 0.0),
            "every Voronoi edge should receive a nonzero canonical noisy amplitude"
        );
    }

    #[test]
    fn default_amplitude_is_visible_at_4k_preview_scale() {
        let (patch, macro_map) = test_inputs(42, 0, 0);
        let boundary = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(42, 11));
        let blocks_per_4k_pixel = 32_768.0 / 3_840.0;

        assert!(
            boundary.stats.average_amplitude_blocks / blocks_per_4k_pixel >= 2.5,
            "average boundary amplitude should be visible at 4K preview scale: avg {:.2} blocks",
            boundary.stats.average_amplitude_blocks
        );
        assert!(
            boundary.stats.max_perpendicular_displacement_blocks / blocks_per_4k_pixel >= 3.0,
            "some boundary displacement should be unmistakable at 4K preview scale"
        );
    }

    #[test]
    fn noisy_curves_use_smooth_correlated_displacement() {
        let (patch, macro_map) = test_inputs(42, 0, 0);
        let boundary = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(42, 11));
        let mut checked = 0;

        for curve in boundary
            .curves
            .iter()
            .filter(|curve| curve_chord_length(curve) >= 32.0 && curve.amplitude >= 8.0)
        {
            let roughness = average_normal_second_difference(curve);
            assert!(
                roughness <= curve.amplitude * 0.22,
                "curve {:?} should avoid sawtooth jitter: roughness {:.2}, amplitude {:.2}",
                curve.edge,
                roughness,
                curve.amplitude
            );
            checked += 1;
        }

        assert!(checked > 0);
    }

    #[test]
    fn selected_hydrology_does_not_create_extra_boundary_curves() {
        let (patch, macro_map) = test_inputs(42, 0, 0);
        let boundary = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(42, 11));
        let unique_edges = boundary
            .curves
            .iter()
            .map(|curve| curve.edge)
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(unique_edges.len(), boundary.curves.len());
    }

    #[test]
    fn adjacent_patch_overlap_keeps_internal_boundary_curves_stable() {
        let (left_patch, left_macro) = test_inputs(77, 0, 0);
        let (right_patch, right_macro) = test_inputs(77, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, 0);
        let config = BoundaryConfig::new(77, 11);
        let left = generate_noisy_boundaries(&left_patch, &left_macro, config);
        let right = generate_noisy_boundaries(&right_patch, &right_macro, config);
        let left_sites = sites_in_region(&left_patch, 1, 0);
        let right_sites = sites_in_region(&right_patch, 1, 0);
        let left_edges = internal_edges(&left_patch, &left_sites);
        let right_edges = internal_edges(&right_patch, &right_sites);
        let shared_edges = left_edges
            .intersection(&right_edges)
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let left_curves = curves_by_edge(&left, &shared_edges);
        let right_curves = curves_by_edge(&right, &shared_edges);

        assert!(!left_curves.is_empty());
        assert_eq!(left_curves, right_curves);
    }

    fn test_inputs(seed: u64, center_x: i32, center_z: i32) -> (VoronoiGraphPatch, GraphMacroMap) {
        let patch = generate_voronoi_graph_patch(VoronoiGraphPatchRequest::new(
            VoronoiGraphConfig {
                seed,
                generator_version: 11,
                region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
                site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
                padding_regions: 17,
            },
            center_x,
            center_z,
        ));
        let macro_map = generate_macro_map(&patch, MacroMapConfig::new(seed, 11));
        (patch, macro_map)
    }

    fn sites_in_region(
        patch: &VoronoiGraphPatch,
        x: i32,
        z: i32,
    ) -> std::collections::HashSet<VoronoiSiteId> {
        patch
            .sites
            .iter()
            .filter(|site| site.owner_region.x == x && site.owner_region.z == z)
            .map(|site| site.id)
            .collect()
    }

    fn internal_edges(
        patch: &VoronoiGraphPatch,
        sites: &std::collections::HashSet<VoronoiSiteId>,
    ) -> std::collections::HashSet<VoronoiEdgeId> {
        patch
            .edges
            .iter()
            .filter(|edge| sites.contains(&edge.sites[0]) && sites.contains(&edge.sites[1]))
            .map(|edge| edge.id)
            .collect()
    }

    fn curves_by_edge(
        cache: &BoundaryCache,
        edges: &std::collections::HashSet<VoronoiEdgeId>,
    ) -> HashMap<VoronoiEdgeId, Vec<WorldPlanePoint>> {
        cache
            .curves
            .iter()
            .filter(|curve| edges.contains(&curve.edge))
            .map(|curve| (curve.edge, curve.points.clone()))
            .collect()
    }

    fn average_normal_second_difference(curve: &NoisyBoundaryCurve) -> f32 {
        if curve.points.len() < 5 {
            return 0.0;
        }
        let dx = curve.anchors.end.x - curve.anchors.start.x;
        let dz = curve.anchors.end.z - curve.anchors.start.z;
        let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
        let normal = WorldPlanePoint::new(-dz / length, dx / length);
        let offsets = curve
            .points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let t = index as f32 / (curve.points.len() - 1) as f32;
                let base = lerp_point(curve.anchors.start, curve.anchors.end, t);
                (point.x - base.x) * normal.x + (point.z - base.z) * normal.z
            })
            .collect::<Vec<_>>();
        let sum = offsets
            .windows(3)
            .map(|window| (window[2] - 2.0 * window[1] + window[0]).abs())
            .sum::<f32>();
        sum / (offsets.len() - 2) as f32
    }
}
