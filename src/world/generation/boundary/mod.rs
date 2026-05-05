use rayon::prelude::*;
use std::collections::HashMap;

use super::graph::{
    VoronoiCornerId, VoronoiEdge, VoronoiEdgeId, VoronoiGraphPatch, VoronoiSiteId, WorldPlanePoint,
};
use super::macro_map::{GraphMacroMap, MacroEdge, MacroLakeEdgeClass, MacroSite};

pub const DEFAULT_BOUNDARY_SUBDIVISION_LEVELS: u8 = 4;
pub const DEFAULT_BOUNDARY_GUARD_MARGIN_BLOCKS: f32 = 1.5;

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
            ordinary_amplitude: 0.05,
            coast_amplitude: 0.24,
            ridge_amplitude: 0.10,
            fault_amplitude: 0.08,
            lake_amplitude: 0.18,
            land_seam_amplitude: 0.07,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
    let points = noisy_midpoint_curve(
        start,
        end,
        site_a,
        site_b,
        seed,
        amplitude,
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
        amplitude,
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
    site_a: WorldPlanePoint,
    site_b: WorldPlanePoint,
    seed: u64,
    amplitude: f32,
    levels: u8,
    guard: BoundaryGuard,
) -> Vec<WorldPlanePoint> {
    let segment_count = 1_usize << levels;
    let mut points = Vec::with_capacity(segment_count + 1);
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    let length = (dx * dx + dz * dz).sqrt().max(f32::EPSILON);
    let normal = WorldPlanePoint::new(-dz / length, dx / length);
    let site_span = distance(site_a, site_b).max(1.0);
    let lateral_limit = (site_span * amplitude * 0.28).min(length * 0.34);

    for index in 0..=segment_count {
        let t = index as f32 / segment_count as f32;
        let base = lerp_point(start, end, t);
        let point = if index == 0 || index == segment_count || amplitude <= f32::EPSILON {
            base
        } else {
            let hash = splitmix64(seed ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
            let wave = (t * std::f32::consts::PI).sin().max(0.0);
            let offset = (unit_f32(hash) * 2.0 - 1.0) * lateral_limit * wave;
            let drift_hash = splitmix64(hash ^ 0xa24b_aed4_963e_3f13);
            let along = (unit_f32(drift_hash) * 2.0 - 1.0) * length * amplitude * 0.035;
            WorldPlanePoint::new(
                base.x + normal.x * offset + dx / length * along,
                base.z + normal.z * offset + dz / length * along,
            )
        };
        points.push(guard.clamp(point));
    }

    points
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
    }

    stats
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
}
