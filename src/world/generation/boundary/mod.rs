use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

use super::graph::{
    VoronoiCornerId, VoronoiEdge, VoronoiEdgeId, VoronoiGraphPatch, VoronoiSiteId, WorldPlanePoint,
};
use super::hydrology::{GraphDrainageNodeId, GraphHydrologyGraph};
use super::macro_map::{GraphMacroMap, MacroEdge, MacroLakeEdgeClass, MacroSite};

pub const DEFAULT_BOUNDARY_SUBDIVISION_LEVELS: u8 = 4;
pub const DEFAULT_BOUNDARY_GUARD_MARGIN_BLOCKS: f32 = 1.5;

const HASH_BOUNDARY: u64 = 0xb31d_0f9c_53a7_8e21;
const ROLE_SALT_COAST: u64 = 0x01c0_a57e_5eed_1001;
const ROLE_SALT_RIVER: u64 = 0x02f1_0a1d_5eed_1002;
const ROLE_SALT_RIDGE: u64 = 0x03a1_7d6e_5eed_1003;
const ROLE_SALT_FAULT: u64 = 0x04fa_0175_5eed_1004;
const ROLE_SALT_LAKE_SHORE: u64 = 0x05aa_1e5d_5eed_1005;
const ROLE_SALT_LAND_BOUNDARY: u64 = 0x06b0_0d1e_5eed_1006;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryConfig {
    pub seed: u64,
    pub generator_version: u32,
    pub subdivision_levels: u8,
    pub guard_margin_blocks: f32,
    pub coast_amplitude: f32,
    pub river_amplitude: f32,
    pub ridge_amplitude: f32,
    pub fault_amplitude: f32,
    pub lake_shore_amplitude: f32,
    pub land_boundary_amplitude: f32,
}

impl BoundaryConfig {
    pub const fn new(seed: u64, generator_version: u32) -> Self {
        Self {
            seed,
            generator_version,
            subdivision_levels: DEFAULT_BOUNDARY_SUBDIVISION_LEVELS,
            guard_margin_blocks: DEFAULT_BOUNDARY_GUARD_MARGIN_BLOCKS,
            coast_amplitude: 0.24,
            river_amplitude: 0.16,
            ridge_amplitude: 0.10,
            fault_amplitude: 0.08,
            lake_shore_amplitude: 0.18,
            land_boundary_amplitude: 0.07,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BoundaryRole {
    Coast,
    River,
    Ridge,
    Fault,
    LakeShore,
    LandBoundary,
}

impl BoundaryRole {
    pub const fn salt(self) -> u64 {
        match self {
            Self::Coast => ROLE_SALT_COAST,
            Self::River => ROLE_SALT_RIVER,
            Self::Ridge => ROLE_SALT_RIDGE,
            Self::Fault => ROLE_SALT_FAULT,
            Self::LakeShore => ROLE_SALT_LAKE_SHORE,
            Self::LandBoundary => ROLE_SALT_LAND_BOUNDARY,
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
    pub role: BoundaryRole,
    pub anchors: BoundaryAnchors,
    pub points: Vec<WorldPlanePoint>,
    pub width_hint_blocks: f32,
    pub amplitude: f32,
    pub seed: u64,
    pub guard: BoundaryGuard,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BoundaryStats {
    pub coast_curve_count: usize,
    pub river_curve_count: usize,
    pub ridge_curve_count: usize,
    pub fault_curve_count: usize,
    pub lake_shore_curve_count: usize,
    pub land_boundary_curve_count: usize,
    pub guard_violation_count: usize,
    pub river_lake_edge_curve_count: usize,
    pub river_endpoint_mismatch_count: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoundaryCache {
    pub curves: Vec<NoisyBoundaryCurve>,
    pub stats: BoundaryStats,
}

impl BoundaryCache {
    pub fn curves_for_edge(
        &self,
        edge: VoronoiEdgeId,
    ) -> impl Iterator<Item = &NoisyBoundaryCurve> {
        self.curves.iter().filter(move |curve| curve.edge == edge)
    }
}

pub fn generate_noisy_boundaries(
    patch: &VoronoiGraphPatch,
    macro_map: &GraphMacroMap,
    hydrology: &GraphHydrologyGraph,
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
    let graph_edges = patch
        .edges
        .iter()
        .map(|edge| (edge.id, *edge))
        .collect::<HashMap<_, _>>();
    let river_edges = hydrology
        .segments
        .iter()
        .map(|segment| segment.edge)
        .collect::<HashSet<_>>();
    let river_flow_by_edge = hydrology.segments.iter().fold(
        HashMap::<VoronoiEdgeId, f32>::new(),
        |mut flows, segment| {
            let current = flows.entry(segment.edge).or_default();
            *current = (*current).max(segment.flow_accumulation);
            flows
        },
    );
    let node_corners = hydrology
        .nodes
        .iter()
        .map(|node| (node.id, node.corner))
        .collect::<HashMap<_, _>>();

    let mut curves = patch
        .edges
        .par_iter()
        .flat_map_iter(|edge| {
            let Some(macro_edge) = macro_edges.get(&edge.id).copied() else {
                return Vec::new().into_iter();
            };
            let roles = classify_boundary_roles(edge, macro_edge, &macro_sites, &river_edges);
            roles
                .into_iter()
                .filter_map(|role| {
                    build_curve_for_edge(
                        *edge,
                        role,
                        macro_edge,
                        &corner_positions,
                        &site_positions,
                        config,
                        river_flow_by_edge.get(&edge.id).copied(),
                    )
                })
                .collect::<Vec<_>>()
                .into_iter()
        })
        .collect::<Vec<_>>();

    curves.sort_by_key(|curve| (curve.role, curve.edge.0));
    let stats = boundary_stats(
        &curves,
        &macro_edges,
        &graph_edges,
        hydrology,
        &node_corners,
    );

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
        config.coast_amplitude,
        config.river_amplitude,
        config.ridge_amplitude,
        config.fault_amplitude,
        config.lake_shore_amplitude,
        config.land_boundary_amplitude,
    ] {
        assert!(
            value.is_finite() && (0.0..=0.5).contains(&value),
            "boundary amplitude must be finite in 0..=0.5"
        );
    }
}

fn classify_boundary_roles(
    graph_edge: &VoronoiEdge,
    macro_edge: MacroEdge,
    macro_sites: &HashMap<VoronoiSiteId, MacroSite>,
    river_edges: &HashSet<VoronoiEdgeId>,
) -> Vec<BoundaryRole> {
    let mut roles = Vec::new();
    if macro_edge.guide.is_coast {
        roles.push(BoundaryRole::Coast);
    }
    if macro_edge.lake_class != MacroLakeEdgeClass::NonLake {
        roles.push(BoundaryRole::LakeShore);
    }
    if macro_edge.guide.is_ridge_candidate {
        roles.push(BoundaryRole::Ridge);
    }
    if macro_edge.guide.is_fault_candidate {
        roles.push(BoundaryRole::Fault);
    }
    if river_edges.contains(&graph_edge.id) && !macro_edge.lake_class.excludes_selected_river() {
        roles.push(BoundaryRole::River);
    }
    if roles.is_empty() && is_low_amplitude_land_boundary(macro_edge, macro_sites) {
        // Low-amplitude ownership seam used by later field/material stages. It is intentionally
        // subtle in preview and can be ignored by heightfield if no visible transition is needed.
        roles.push(BoundaryRole::LandBoundary);
    }
    roles
}

fn is_low_amplitude_land_boundary(
    edge: MacroEdge,
    macro_sites: &HashMap<VoronoiSiteId, MacroSite>,
) -> bool {
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

fn build_curve_for_edge(
    edge: VoronoiEdge,
    role: BoundaryRole,
    macro_edge: MacroEdge,
    corner_positions: &HashMap<VoronoiCornerId, WorldPlanePoint>,
    site_positions: &HashMap<VoronoiSiteId, WorldPlanePoint>,
    config: BoundaryConfig,
    river_flow: Option<f32>,
) -> Option<NoisyBoundaryCurve> {
    let start = *corner_positions.get(&edge.corners[0])?;
    let end = *corner_positions.get(&edge.corners[1])?;
    let site_a = *site_positions.get(&edge.sites[0])?;
    let site_b = *site_positions.get(&edge.sites[1])?;
    let seed = curve_seed(config, edge.id, role);
    let amplitude = amplitude_for_role(role, config);
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
        role,
        anchors: BoundaryAnchors {
            corners: macro_edge.corners,
            sites: macro_edge.sites,
            start,
            end,
        },
        points,
        width_hint_blocks: width_hint_for_role(role, macro_edge, river_flow),
        amplitude,
        seed,
        guard,
    })
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
            let wave = ((t * std::f32::consts::PI).sin()).max(0.0);
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

fn curve_seed(config: BoundaryConfig, edge: VoronoiEdgeId, role: BoundaryRole) -> u64 {
    let mut state = splitmix64(config.seed ^ HASH_BOUNDARY);
    state = splitmix64(state ^ u64::from(config.generator_version));
    state = splitmix64(state ^ edge.0);
    splitmix64(state ^ role.salt())
}

fn amplitude_for_role(role: BoundaryRole, config: BoundaryConfig) -> f32 {
    match role {
        BoundaryRole::Coast => config.coast_amplitude,
        BoundaryRole::River => config.river_amplitude,
        BoundaryRole::Ridge => config.ridge_amplitude,
        BoundaryRole::Fault => config.fault_amplitude,
        BoundaryRole::LakeShore => config.lake_shore_amplitude,
        BoundaryRole::LandBoundary => config.land_boundary_amplitude,
    }
}

fn width_hint_for_role(role: BoundaryRole, edge: MacroEdge, river_flow: Option<f32>) -> f32 {
    match role {
        BoundaryRole::Coast => 10.0 + edge.guide.coastness * 18.0,
        BoundaryRole::River => 2.0 + river_flow.unwrap_or(0.0).sqrt() * 0.45,
        BoundaryRole::Ridge => 8.0 + edge.guide.ridgeness * 16.0,
        BoundaryRole::Fault => 6.0 + edge.guide.signed_elevation_gradient.abs() * 20.0,
        BoundaryRole::LakeShore => 5.0,
        BoundaryRole::LandBoundary => 2.0,
    }
}

fn boundary_stats(
    curves: &[NoisyBoundaryCurve],
    macro_edges: &HashMap<VoronoiEdgeId, MacroEdge>,
    graph_edges: &HashMap<VoronoiEdgeId, VoronoiEdge>,
    hydrology: &GraphHydrologyGraph,
    node_corners: &HashMap<GraphDrainageNodeId, VoronoiCornerId>,
) -> BoundaryStats {
    let mut stats = BoundaryStats::default();
    let river_edges = hydrology
        .segments
        .iter()
        .map(|segment| segment.edge)
        .collect::<HashSet<_>>();
    let river_curve_edges = curves
        .iter()
        .filter(|curve| curve.role == BoundaryRole::River)
        .map(|curve| curve.edge)
        .collect::<HashSet<_>>();

    for curve in curves {
        match curve.role {
            BoundaryRole::Coast => stats.coast_curve_count += 1,
            BoundaryRole::River => stats.river_curve_count += 1,
            BoundaryRole::Ridge => stats.ridge_curve_count += 1,
            BoundaryRole::Fault => stats.fault_curve_count += 1,
            BoundaryRole::LakeShore => stats.lake_shore_curve_count += 1,
            BoundaryRole::LandBoundary => stats.land_boundary_curve_count += 1,
        }
        if curve
            .points
            .iter()
            .any(|point| !curve.guard.contains(*point))
        {
            stats.guard_violation_count += 1;
        }
        if curve.role == BoundaryRole::River
            && macro_edges
                .get(&curve.edge)
                .is_some_and(|edge| edge.lake_class.excludes_selected_river())
        {
            stats.river_lake_edge_curve_count += 1;
        }
    }

    for segment in &hydrology.segments {
        if !river_curve_edges.contains(&segment.edge) {
            continue;
        }
        let Some(edge) = graph_edges.get(&segment.edge) else {
            stats.river_endpoint_mismatch_count += 1;
            continue;
        };
        let Some(from_corner) = node_corners.get(&segment.from).copied() else {
            stats.river_endpoint_mismatch_count += 1;
            continue;
        };
        let Some(to_corner) = node_corners.get(&segment.to).copied() else {
            stats.river_endpoint_mismatch_count += 1;
            continue;
        };
        if !edge.corners.contains(&from_corner) || !edge.corners.contains(&to_corner) {
            stats.river_endpoint_mismatch_count += 1;
        }
    }

    stats.river_endpoint_mismatch_count += river_edges
        .difference(&river_curve_edges)
        .filter(|edge| {
            macro_edges
                .get(edge)
                .is_none_or(|edge| !edge.lake_class.excludes_selected_river())
        })
        .count();
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
    use crate::world::generation::hydrology::{HydrologyConfig, solve_hydrology};
    use crate::world::generation::macro_map::{MacroMapConfig, generate_macro_map};

    #[test]
    fn boundary_generation_is_deterministic_for_same_input() {
        let (patch, macro_map, hydrology) = test_inputs(42, 0, 0);
        let config = BoundaryConfig::new(42, 11);

        let first = generate_noisy_boundaries(&patch, &macro_map, &hydrology, config);
        let second = generate_noisy_boundaries(&patch, &macro_map, &hydrology, config);

        assert_eq!(first, second);
    }

    #[test]
    fn boundary_generation_changes_with_seed_or_role() {
        let (patch, macro_map, hydrology) = test_inputs(42, 0, 0);
        let first =
            generate_noisy_boundaries(&patch, &macro_map, &hydrology, BoundaryConfig::new(42, 11));
        let second =
            generate_noisy_boundaries(&patch, &macro_map, &hydrology, BoundaryConfig::new(43, 11));

        assert_ne!(first.curves, second.curves);

        let edge_with_multiple_roles = first
            .curves
            .iter()
            .find_map(|curve| {
                first
                    .curves_for_edge(curve.edge)
                    .find(|other| other.role != curve.role)
                    .map(|other| (curve, other))
            })
            .expect("test patch should expose at least one multi-role edge");
        assert_ne!(
            edge_with_multiple_roles.0.points,
            edge_with_multiple_roles.1.points
        );
    }

    #[test]
    fn noisy_points_stay_inside_edge_guard() {
        let (patch, macro_map, hydrology) = test_inputs(42, 0, 0);
        let boundary =
            generate_noisy_boundaries(&patch, &macro_map, &hydrology, BoundaryConfig::new(42, 11));

        assert_eq!(boundary.stats.guard_violation_count, 0);
        assert!(boundary.curves.iter().all(|curve| {
            curve
                .points
                .iter()
                .all(|point| curve.guard.contains(*point))
        }));
    }

    #[test]
    fn river_curves_only_use_selected_non_lake_edges() {
        let (patch, macro_map, hydrology) = test_inputs(42, 0, 0);
        let boundary =
            generate_noisy_boundaries(&patch, &macro_map, &hydrology, BoundaryConfig::new(42, 11));
        let selected_edges = hydrology
            .segments
            .iter()
            .map(|segment| segment.edge)
            .collect::<HashSet<_>>();
        let macro_edges = macro_map
            .edges
            .iter()
            .map(|edge| (edge.id, *edge))
            .collect::<HashMap<_, _>>();

        assert_eq!(boundary.stats.river_lake_edge_curve_count, 0);
        assert_eq!(boundary.stats.river_endpoint_mismatch_count, 0);
        assert!(
            boundary
                .curves
                .iter()
                .any(|curve| curve.role == BoundaryRole::River)
        );
        for curve in boundary
            .curves
            .iter()
            .filter(|curve| curve.role == BoundaryRole::River)
        {
            assert!(selected_edges.contains(&curve.edge));
            assert_eq!(
                macro_edges[&curve.edge].lake_class,
                MacroLakeEdgeClass::NonLake
            );
        }
    }

    #[test]
    fn adjacent_patch_overlap_keeps_internal_boundary_curves_stable() {
        let (left_patch, left_macro, left_hydro) = test_inputs(77, 0, 0);
        let (right_patch, right_macro, right_hydro) =
            test_inputs(77, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, 0);
        let config = BoundaryConfig::new(77, 11);
        let left = generate_noisy_boundaries(&left_patch, &left_macro, &left_hydro, config);
        let right = generate_noisy_boundaries(&right_patch, &right_macro, &right_hydro, config);
        let left_sites = sites_in_region(&left_patch, 1, 0);
        let right_sites = sites_in_region(&right_patch, 1, 0);
        let left_edges = internal_edges(&left_patch, &left_sites);
        let right_edges = internal_edges(&right_patch, &right_sites);
        let shared_edges = left_edges
            .intersection(&right_edges)
            .copied()
            .collect::<HashSet<_>>();
        let left_curves = curves_by_key(&left, &shared_edges);
        let right_curves = curves_by_key(&right, &shared_edges);

        assert!(!left_curves.is_empty());
        assert_eq!(left_curves, right_curves);
    }

    fn test_inputs(
        seed: u64,
        center_x: i32,
        center_z: i32,
    ) -> (VoronoiGraphPatch, GraphMacroMap, GraphHydrologyGraph) {
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
        let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        (patch, macro_map, hydrology)
    }

    fn sites_in_region(patch: &VoronoiGraphPatch, x: i32, z: i32) -> HashSet<VoronoiSiteId> {
        patch
            .sites
            .iter()
            .filter(|site| site.owner_region.x == x && site.owner_region.z == z)
            .map(|site| site.id)
            .collect()
    }

    fn internal_edges(
        patch: &VoronoiGraphPatch,
        sites: &HashSet<VoronoiSiteId>,
    ) -> HashSet<VoronoiEdgeId> {
        patch
            .edges
            .iter()
            .filter(|edge| sites.contains(&edge.sites[0]) && sites.contains(&edge.sites[1]))
            .map(|edge| edge.id)
            .collect()
    }

    fn curves_by_key(
        cache: &BoundaryCache,
        edges: &HashSet<VoronoiEdgeId>,
    ) -> HashMap<(BoundaryRole, VoronoiEdgeId), Vec<WorldPlanePoint>> {
        cache
            .curves
            .iter()
            .filter(|curve| edges.contains(&curve.edge))
            .map(|curve| ((curve.role, curve.edge), curve.points.clone()))
            .collect()
    }
}
