use rayon::prelude::*;
use std::collections::HashMap;

use super::boundary::{BoundaryCache, NoisyBoundaryCurve};
use super::graph::{VoronoiEdgeId, VoronoiGraphPatch, VoronoiSiteId, WorldPlanePoint};
use super::hydrology::GraphHydrologyGraph;
use super::macro_map::{GraphMacroMap, MacroSite, MacroSurfaceKind};

pub const DEFAULT_MACRO_FIELD_SAMPLE_SPACING_BLOCKS: f32 = 32.0;
pub const DEFAULT_MACRO_FIELD_RIDGE_RADIUS_BLOCKS: f32 = 640.0;
pub const DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS: f32 = 192.0;
pub const DEFAULT_MACRO_FIELD_COAST_RADIUS_BLOCKS: f32 = 384.0;
pub const DEFAULT_MACRO_FIELD_RIDGE_HEIGHT_SCALE: f32 = 0.42;
pub const DEFAULT_MACRO_FIELD_RIVER_CARVE_SCALE: f32 = 0.30;
pub const DEFAULT_MACRO_FIELD_COAST_FLATTEN_STRENGTH: f32 = 0.82;
pub const DEFAULT_MACRO_FIELD_LAKE_FLATTEN_STRENGTH: f32 = 0.96;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroFieldTileConfig {
    pub origin: WorldPlanePoint,
    pub width: u32,
    pub height: u32,
    pub sample_spacing_blocks: f32,
    pub ridge_radius_blocks: f32,
    pub river_radius_blocks: f32,
    pub coast_radius_blocks: f32,
    pub ridge_height_scale: f32,
    pub river_carve_scale: f32,
    pub coast_flatten_strength: f32,
    pub lake_flatten_strength: f32,
}

impl MacroFieldTileConfig {
    pub const fn new(
        origin_x: f32,
        origin_z: f32,
        width: u32,
        height: u32,
        sample_spacing_blocks: f32,
    ) -> Self {
        Self {
            origin: WorldPlanePoint::new(origin_x, origin_z),
            width,
            height,
            sample_spacing_blocks,
            ridge_radius_blocks: DEFAULT_MACRO_FIELD_RIDGE_RADIUS_BLOCKS,
            river_radius_blocks: DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS,
            coast_radius_blocks: DEFAULT_MACRO_FIELD_COAST_RADIUS_BLOCKS,
            ridge_height_scale: DEFAULT_MACRO_FIELD_RIDGE_HEIGHT_SCALE,
            river_carve_scale: DEFAULT_MACRO_FIELD_RIVER_CARVE_SCALE,
            coast_flatten_strength: DEFAULT_MACRO_FIELD_COAST_FLATTEN_STRENGTH,
            lake_flatten_strength: DEFAULT_MACRO_FIELD_LAKE_FLATTEN_STRENGTH,
        }
    }

    pub fn sample_count(self) -> usize {
        self.width as usize * self.height as usize
    }

    pub fn sample_position(self, index: usize) -> WorldPlanePoint {
        let x = index % self.width as usize;
        let z = index / self.width as usize;
        WorldPlanePoint::new(
            self.origin.x + x as f32 * self.sample_spacing_blocks,
            self.origin.z + z as f32 * self.sample_spacing_blocks,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroFieldSample {
    pub position: WorldPlanePoint,
    pub nearest_site: Option<VoronoiSiteId>,
    pub surface_kind: Option<MacroSurfaceKind>,
    pub macro_elevation: f32,
    pub ocean_mask: f32,
    pub coast_mask: f32,
    pub lake_mask: f32,
    pub dry_basin_mask: f32,
    pub ridge_influence: f32,
    pub river_valley_strength: f32,
    pub river_distance_blocks: f32,
    pub river_flow_hint: f32,
    pub combined_macro_height: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MacroFieldTileStats {
    pub sample_count: usize,
    pub min_combined_macro_height: f32,
    pub max_combined_macro_height: f32,
    pub max_ridge_influence: f32,
    pub max_river_valley_strength: f32,
    pub ocean_sample_count: usize,
    pub lake_sample_count: usize,
    pub dry_basin_sample_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacroFieldTile {
    pub config: MacroFieldTileConfig,
    pub samples: Vec<MacroFieldSample>,
    pub stats: MacroFieldTileStats,
}

impl MacroFieldTile {
    pub fn sample(&self, x: u32, z: u32) -> Option<&MacroFieldSample> {
        if x >= self.config.width || z >= self.config.height {
            return None;
        }
        let index = z as usize * self.config.width as usize + x as usize;
        self.samples.get(index)
    }
}

pub fn generate_macro_field_tile(
    patch: &VoronoiGraphPatch,
    macro_map: &GraphMacroMap,
    hydrology: &GraphHydrologyGraph,
    boundary: &BoundaryCache,
    config: MacroFieldTileConfig,
) -> MacroFieldTile {
    validate_macro_field_config(config);
    assert_eq!(
        boundary.stats.missing_macro_edge_count, 0,
        "macro field requires complete canonical boundary coverage"
    );

    let context = MacroFieldRasterContext::new(patch, macro_map, hydrology, boundary);
    let samples = (0..config.sample_count())
        .into_par_iter()
        .map(|index| sample_macro_field_point(&context, config, config.sample_position(index)))
        .collect::<Vec<_>>();
    let stats = macro_field_stats(&samples);

    MacroFieldTile {
        config,
        samples,
        stats,
    }
}

pub fn sample_macro_field_point(
    context: &MacroFieldRasterContext<'_>,
    config: MacroFieldTileConfig,
    position: WorldPlanePoint,
) -> MacroFieldSample {
    let nearest_site = context.nearest_site(position);
    let surface_kind = nearest_site.map(|site| site.surface_kind);
    let macro_elevation = nearest_site
        .map(|site| site.signed_macro_elevation)
        .unwrap_or_default();
    let ocean_mask = surface_kind
        .is_some_and(MacroSurfaceKind::is_ocean_owned)
        .then_some(1.0)
        .unwrap_or(0.0);
    let lake_mask = surface_kind
        .is_some_and(is_lake_surface)
        .then_some(1.0)
        .unwrap_or(0.0);
    let dry_basin_mask = surface_kind
        .is_some_and(|kind| kind == MacroSurfaceKind::DryBasin)
        .then_some(1.0)
        .unwrap_or(0.0);
    let coast_mask = context
        .nearest_coast_distance(position)
        .map(|distance| envelope(distance, config.coast_radius_blocks))
        .unwrap_or_else(|| nearest_site.map(|site| site.coastness).unwrap_or_default())
        .max(nearest_site.map(|site| site.coastness).unwrap_or_default())
        .clamp(0.0, 1.0);
    let ridge_influence = context
        .ridge_curves
        .iter()
        .map(|curve| {
            envelope(
                polyline_distance(position, &curve.points),
                config.ridge_radius_blocks,
            )
        })
        .fold(0.0, f32::max);
    let (river_distance_blocks, river_flow_hint, river_valley_strength) =
        context.river_valley(position, config);
    let combined_macro_height = combine_macro_height(
        macro_elevation,
        ocean_mask,
        coast_mask,
        lake_mask,
        dry_basin_mask,
        ridge_influence,
        river_valley_strength,
        river_flow_hint,
        config,
    );

    MacroFieldSample {
        position,
        nearest_site: nearest_site.map(|site| site.id),
        surface_kind,
        macro_elevation,
        ocean_mask,
        coast_mask,
        lake_mask,
        dry_basin_mask,
        ridge_influence,
        river_valley_strength,
        river_distance_blocks,
        river_flow_hint,
        combined_macro_height,
    }
}

#[derive(Debug)]
pub struct MacroFieldRasterContext<'a> {
    sites: &'a [MacroSite],
    coast_curves: Vec<&'a NoisyBoundaryCurve>,
    ridge_curves: Vec<&'a NoisyBoundaryCurve>,
    river_curves: Vec<RiverCurveRef<'a>>,
}

impl<'a> MacroFieldRasterContext<'a> {
    pub fn new(
        patch: &'a VoronoiGraphPatch,
        macro_map: &'a GraphMacroMap,
        hydrology: &'a GraphHydrologyGraph,
        boundary: &'a BoundaryCache,
    ) -> Self {
        let macro_edges = macro_map
            .edges
            .iter()
            .map(|edge| (edge.id, edge))
            .collect::<HashMap<_, _>>();
        let boundary_curves = boundary
            .curves
            .iter()
            .map(|curve| (curve.edge, curve))
            .collect::<HashMap<_, _>>();
        let mut river_flow_by_edge = HashMap::<VoronoiEdgeId, f32>::new();
        for segment in &hydrology.segments {
            river_flow_by_edge
                .entry(segment.edge)
                .and_modify(|flow| *flow = flow.max(segment.flow_accumulation))
                .or_insert(segment.flow_accumulation);
        }

        let coast_curves = patch
            .edges
            .iter()
            .filter_map(|edge| {
                macro_edges
                    .get(&edge.id)
                    .is_some_and(|macro_edge| macro_edge.guide.is_coast)
                    .then(|| boundary_curves.get(&edge.id).copied())
                    .flatten()
            })
            .collect::<Vec<_>>();
        let ridge_curves = patch
            .edges
            .iter()
            .filter_map(|edge| {
                macro_edges
                    .get(&edge.id)
                    .is_some_and(|macro_edge| macro_edge.guide.is_ridge_candidate)
                    .then(|| boundary_curves.get(&edge.id).copied())
                    .flatten()
            })
            .collect::<Vec<_>>();
        let mut river_curves = river_flow_by_edge
            .into_iter()
            .filter_map(|(edge, flow_accumulation)| {
                boundary_curves
                    .get(&edge)
                    .copied()
                    .map(|curve| RiverCurveRef {
                        edge,
                        curve,
                        flow_accumulation,
                    })
            })
            .collect::<Vec<_>>();
        river_curves.sort_by_key(|river| river.edge.0);

        Self {
            sites: &macro_map.sites,
            coast_curves,
            ridge_curves,
            river_curves,
        }
    }

    pub fn nearest_site(&self, position: WorldPlanePoint) -> Option<&'a MacroSite> {
        self.sites.iter().min_by(|left, right| {
            squared_distance(position, left.position)
                .total_cmp(&squared_distance(position, right.position))
                .then_with(|| left.id.0.cmp(&right.id.0))
        })
    }

    fn nearest_coast_distance(&self, position: WorldPlanePoint) -> Option<f32> {
        self.coast_curves
            .iter()
            .map(|curve| polyline_distance(position, &curve.points))
            .min_by(f32::total_cmp)
    }

    fn river_valley(
        &self,
        position: WorldPlanePoint,
        config: MacroFieldTileConfig,
    ) -> (f32, f32, f32) {
        let Some((distance, flow)) = self
            .river_curves
            .iter()
            .map(|river| {
                (
                    polyline_distance(position, &river.curve.points),
                    river.flow_accumulation.max(0.0),
                )
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
        else {
            return (f32::INFINITY, 0.0, 0.0);
        };
        let flow_hint = flow_hint(flow);
        let valley = envelope(distance, config.river_radius_blocks) * (0.35 + flow_hint * 0.65);

        (distance, flow_hint, valley.clamp(0.0, 1.0))
    }
}

#[derive(Debug, Clone, Copy)]
struct RiverCurveRef<'a> {
    edge: VoronoiEdgeId,
    curve: &'a NoisyBoundaryCurve,
    flow_accumulation: f32,
}

fn validate_macro_field_config(config: MacroFieldTileConfig) {
    assert!(config.width > 0, "macro field tile width must be > 0");
    assert!(config.height > 0, "macro field tile height must be > 0");
    for (name, value) in [
        ("sample_spacing_blocks", config.sample_spacing_blocks),
        ("ridge_radius_blocks", config.ridge_radius_blocks),
        ("river_radius_blocks", config.river_radius_blocks),
        ("coast_radius_blocks", config.coast_radius_blocks),
        ("ridge_height_scale", config.ridge_height_scale),
        ("river_carve_scale", config.river_carve_scale),
        ("coast_flatten_strength", config.coast_flatten_strength),
        ("lake_flatten_strength", config.lake_flatten_strength),
    ] {
        assert!(value.is_finite(), "{name} must be finite");
    }
    assert!(
        config.sample_spacing_blocks > 0.0,
        "sample spacing must be positive"
    );
    assert!(config.ridge_radius_blocks > 0.0, "ridge radius must be > 0");
    assert!(config.river_radius_blocks > 0.0, "river radius must be > 0");
    assert!(config.coast_radius_blocks > 0.0, "coast radius must be > 0");
}

fn combine_macro_height(
    macro_elevation: f32,
    ocean_mask: f32,
    coast_mask: f32,
    lake_mask: f32,
    dry_basin_mask: f32,
    ridge_influence: f32,
    river_valley_strength: f32,
    river_flow_hint: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    let ridge_raise = ridge_influence * config.ridge_height_scale;
    let river_carve = river_valley_strength
        * config.river_carve_scale
        * (0.45 + river_flow_hint * 0.55)
        * (1.0 - ocean_mask);
    let coast_flatten = coast_mask * config.coast_flatten_strength;
    let lake_flatten = lake_mask * config.lake_flatten_strength;
    let flatten = coast_flatten.max(lake_flatten).clamp(0.0, 1.0);
    let flatten_target = if ocean_mask > 0.5 || lake_mask > 0.5 {
        -0.035
    } else {
        0.0
    };
    let mut height = macro_elevation + ridge_raise - river_carve;
    height = height + (flatten_target - height) * flatten;
    if dry_basin_mask > 0.5 {
        height = height.max(-0.02);
    }
    height.clamp(-2.0, 2.0)
}

fn macro_field_stats(samples: &[MacroFieldSample]) -> MacroFieldTileStats {
    if samples.is_empty() {
        return MacroFieldTileStats::default();
    }

    let mut stats = MacroFieldTileStats {
        sample_count: samples.len(),
        min_combined_macro_height: f32::INFINITY,
        max_combined_macro_height: f32::NEG_INFINITY,
        ..MacroFieldTileStats::default()
    };

    for sample in samples {
        stats.min_combined_macro_height = stats
            .min_combined_macro_height
            .min(sample.combined_macro_height);
        stats.max_combined_macro_height = stats
            .max_combined_macro_height
            .max(sample.combined_macro_height);
        stats.max_ridge_influence = stats.max_ridge_influence.max(sample.ridge_influence);
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
        }
    }

    stats
}

fn is_lake_surface(kind: MacroSurfaceKind) -> bool {
    matches!(
        kind,
        MacroSurfaceKind::LakeCandidate | MacroSurfaceKind::WetlandCandidate
    )
}

fn envelope(distance: f32, radius: f32) -> f32 {
    if !distance.is_finite() {
        return 0.0;
    }
    let t = (1.0 - distance / radius.max(f32::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn flow_hint(flow_accumulation: f32) -> f32 {
    (flow_accumulation.max(0.0).sqrt() / 32.0).clamp(0.0, 1.0)
}

fn polyline_distance(position: WorldPlanePoint, points: &[WorldPlanePoint]) -> f32 {
    match points {
        [] => f32::INFINITY,
        [point] => squared_distance(position, *point).sqrt(),
        _ => points
            .windows(2)
            .map(|segment| point_segment_distance(position, segment[0], segment[1]))
            .fold(f32::INFINITY, f32::min),
    }
}

fn point_segment_distance(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    let len2 = dx * dx + dz * dz;
    if len2 <= f32::EPSILON {
        return squared_distance(point, start).sqrt();
    }
    let t = (((point.x - start.x) * dx + (point.z - start.z) * dz) / len2).clamp(0.0, 1.0);
    let projected = WorldPlanePoint::new(start.x + dx * t, start.z + dz * t);
    squared_distance(point, projected).sqrt()
}

fn squared_distance(a: WorldPlanePoint, b: WorldPlanePoint) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::boundary::{BoundaryConfig, generate_noisy_boundaries};
    use crate::world::generation::graph::{
        DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, VoronoiGraphConfig,
        VoronoiGraphPatchRequest, generate_voronoi_graph_patch,
    };
    use crate::world::generation::hydrology::{HydrologyConfig, solve_hydrology};
    use crate::world::generation::macro_map::{MacroMapConfig, generate_macro_map};

    #[test]
    fn macro_field_tile_generation_is_deterministic() {
        let inputs = test_inputs(42);
        let config = test_tile_config();

        let first = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.hydrology,
            &inputs.boundary,
            config,
        );
        let second = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.hydrology,
            &inputs.boundary,
            config,
        );

        assert_eq!(first, second);
    }

    #[test]
    fn macro_field_tile_dimensions_match_channel_lengths() {
        let inputs = test_inputs(42);
        let config = test_tile_config();

        let tile = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.hydrology,
            &inputs.boundary,
            config,
        );

        assert_eq!(tile.samples.len(), config.sample_count());
        assert_eq!(tile.stats.sample_count, config.sample_count());
        assert!(tile.sample(0, 0).is_some());
        assert!(tile.sample(config.width, 0).is_none());
    }

    #[test]
    fn combined_macro_height_is_finite_and_sane() {
        let inputs = test_inputs(42);
        let tile = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.hydrology,
            &inputs.boundary,
            test_tile_config(),
        );

        assert!(tile.samples.iter().all(|sample| {
            sample.macro_elevation.is_finite()
                && sample.combined_macro_height.is_finite()
                && (-2.0..=2.0).contains(&sample.combined_macro_height)
        }));
        assert!(tile.stats.min_combined_macro_height <= tile.stats.max_combined_macro_height);
    }

    #[test]
    fn ridge_influence_is_higher_near_ridge_curve_than_far_sample() {
        let inputs = test_inputs(42);
        let Some(ridge_curve) = inputs.boundary.curves.iter().find(|curve| {
            inputs
                .macro_map
                .edge(curve.edge)
                .is_some_and(|edge| edge.guide.is_ridge_candidate)
        }) else {
            return;
        };
        let context = MacroFieldRasterContext::new(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.hydrology,
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
            &inputs.hydrology,
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
        assert!(near_sample.river_flow_hint >= far_sample.river_flow_hint);
    }

    struct TestInputs {
        patch: super::super::graph::VoronoiGraphPatch,
        macro_map: GraphMacroMap,
        hydrology: GraphHydrologyGraph,
        boundary: BoundaryCache,
    }

    fn test_inputs(seed: u64) -> TestInputs {
        let patch = generate_voronoi_graph_patch(VoronoiGraphPatchRequest::new(
            VoronoiGraphConfig {
                seed,
                generator_version: 11,
                region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
                site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
                padding_regions: 17,
            },
            0,
            0,
        ));
        let macro_map = generate_macro_map(&patch, MacroMapConfig::new(seed, 11));
        let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        let boundary = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(seed, 11));

        TestInputs {
            patch,
            macro_map,
            hydrology,
            boundary,
        }
    }

    fn test_tile_config() -> MacroFieldTileConfig {
        MacroFieldTileConfig::new(-512.0, -512.0, 24, 24, 64.0)
    }
}
