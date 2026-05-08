use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

use super::biome::{GraphBiomeCell, GraphBiomeContext, GraphBiomeKind};
use super::boundary::{BoundaryCache, NoisyBoundaryCurve};
use super::graph::{VoronoiEdgeId, VoronoiGraphPatch, VoronoiSiteId, WorldPlanePoint};
use super::hydrology::{GraphDrainageNodeId, GraphHydrologyGraph, GraphRiverSegment};
use super::macro_map::{GraphMacroMap, MacroSite, MacroSurfaceKind};

const MACRO_FIELD_CURVE_BUCKET_BLOCKS: f32 = 256.0;

pub const DEFAULT_MACRO_FIELD_SAMPLE_SPACING_BLOCKS: f32 = 32.0;
pub const DEFAULT_MACRO_FIELD_RIDGE_RADIUS_BLOCKS: f32 = 256.0;
pub const DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS: f32 = 160.0;
pub const DEFAULT_MACRO_FIELD_COAST_RADIUS_BLOCKS: f32 = 384.0;
pub const DEFAULT_MACRO_FIELD_RIDGE_HEIGHT_SCALE: f32 = 0.0;
pub const DEFAULT_MACRO_FIELD_RIVER_CARVE_SCALE: f32 = 0.34;
pub const DEFAULT_MACRO_FIELD_COAST_FLATTEN_STRENGTH: f32 = 0.82;
pub const DEFAULT_MACRO_FIELD_LAKE_FLATTEN_STRENGTH: f32 = 0.96;
pub const DEFAULT_MACRO_FIELD_BOUNDARY_BLEND_RADIUS_BLOCKS: f32 = 96.0;
pub const MACRO_FIELD_CONTOUR_NORMALIZED_MIN: f32 = -0.5;
pub const MACRO_FIELD_CONTOUR_NORMALIZED_MAX: f32 = 1.0;
pub const MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS: f32 = -1024.0;
pub const MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS: f32 = 2048.0;
pub const DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS: f32 = 32.0;
pub const DEFAULT_MACRO_FIELD_CONTOUR_MAJOR_EVERY: u32 = 5;

const RIDGE_INFLUENCE_VISIBLE_FLOOR: f32 = 0.12;
const RIDGE_FIELD_SOURCE_MIN_RIDGENESS: f32 = 0.44;
const RIVER_MIN_WIDTH_BLOCKS: f32 = 28.0;
const RIVER_MAX_WIDTH_BLOCKS: f32 = 176.0;
const RIVER_MIN_FLAT_BED_BLOCKS: f32 = 3.5;
const RIVER_MAX_FLAT_BED_BLOCKS: f32 = 56.0;
const RIVER_HEADWATER_DEPTH_FACTOR: f32 = 0.12;
const RIVER_TRUNK_DEPTH_FACTOR: f32 = 0.68;
const DRY_BASIN_MIN_HEIGHT: f32 = 0.018;
const DRY_BASIN_FLOOR_LOWERING: f32 = 0.055;
const DRY_BASIN_RIM_RAISE: f32 = 0.18;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroFieldTileConfig {
    pub origin: WorldPlanePoint,
    pub width: u32,
    pub height: u32,
    pub sample_spacing_blocks: f32,
    pub ridge_radius_blocks: f32,
    pub river_radius_blocks: f32,
    pub coast_radius_blocks: f32,
    pub boundary_blend_radius_blocks: f32,
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
            boundary_blend_radius_blocks: DEFAULT_MACRO_FIELD_BOUNDARY_BLEND_RADIUS_BLOCKS,
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
    pub biome_context: Option<GraphBiomeContext>,
    pub biome: Option<GraphBiomeKind>,
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
    pub average_ridge_influence: f32,
    pub ridge_active_sample_count: usize,
    pub max_river_valley_strength: f32,
    pub ocean_sample_count: usize,
    pub lake_sample_count: usize,
    pub dry_basin_sample_count: usize,
    pub min_dry_basin_height: f32,
    pub max_dry_basin_height: f32,
    pub average_dry_basin_height: f32,
    pub ridge_source_curve_count: usize,
    pub river_source_curve_count: usize,
    pub coast_source_curve_count: usize,
    pub ridge_source_pixel_count: usize,
    pub river_source_pixel_count: usize,
    pub coast_source_pixel_count: usize,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MacroFieldContourSegment {
    pub start: WorldPlanePoint,
    pub end: WorldPlanePoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacroFieldContourLevel {
    pub height_blocks: f32,
    pub is_major: bool,
    pub segments: Vec<MacroFieldContourSegment>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MacroFieldContourSet {
    pub step_blocks: f32,
    pub major_every: u32,
    pub min_level_blocks: f32,
    pub max_level_blocks: f32,
    pub total_segment_count: usize,
    pub levels: Vec<MacroFieldContourLevel>,
}

pub fn combined_macro_height_to_blocks(value: f32) -> f32 {
    if value >= 0.0 {
        let t = (value / MACRO_FIELD_CONTOUR_NORMALIZED_MAX.max(f32::EPSILON)).clamp(0.0, 1.0);
        t * MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS
    } else {
        let t = (value / MACRO_FIELD_CONTOUR_NORMALIZED_MIN.min(-f32::EPSILON)).clamp(0.0, 1.0);
        t * MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS
    }
}

pub fn extract_macro_field_contours(
    tile: &MacroFieldTile,
    step_blocks: f32,
    major_every: u32,
) -> MacroFieldContourSet {
    assert!(
        step_blocks.is_finite() && step_blocks > 0.0,
        "contour step must be finite and positive"
    );
    let major_every = major_every.max(1);
    let width = tile.config.width as usize;
    let height = tile.config.height as usize;
    if width < 2 || height < 2 || tile.samples.len() != width * height {
        return MacroFieldContourSet {
            step_blocks,
            major_every,
            min_level_blocks: 0.0,
            max_level_blocks: 0.0,
            total_segment_count: 0,
            levels: Vec::new(),
        };
    }

    let mut min_height = f32::INFINITY;
    let mut max_height = f32::NEG_INFINITY;
    let heights = tile
        .samples
        .iter()
        .map(|sample| {
            let height = combined_macro_height_to_blocks(sample.combined_macro_height);
            min_height = min_height.min(height);
            max_height = max_height.max(height);
            height
        })
        .collect::<Vec<_>>();
    if !min_height.is_finite()
        || !max_height.is_finite()
        || (max_height - min_height).abs() <= f32::EPSILON
    {
        return MacroFieldContourSet {
            step_blocks,
            major_every,
            min_level_blocks: min_height,
            max_level_blocks: max_height,
            total_segment_count: 0,
            levels: Vec::new(),
        };
    }

    let first_level = (min_height / step_blocks).ceil() as i32;
    let last_level = (max_height / step_blocks).floor() as i32;
    let mut levels = Vec::new();
    let mut total_segment_count = 0;
    for level_index in first_level..=last_level {
        let height_blocks = level_index as f32 * step_blocks;
        let mut segments = Vec::new();
        for z in 0..height - 1 {
            for x in 0..width - 1 {
                append_contour_cell_segments(
                    &mut segments,
                    &heights,
                    tile.config,
                    width,
                    x,
                    z,
                    height_blocks,
                );
            }
        }
        total_segment_count += segments.len();
        levels.push(MacroFieldContourLevel {
            height_blocks,
            is_major: level_index.rem_euclid(major_every as i32) == 0,
            segments,
        });
    }

    MacroFieldContourSet {
        step_blocks,
        major_every,
        min_level_blocks: first_level as f32 * step_blocks,
        max_level_blocks: last_level as f32 * step_blocks,
        total_segment_count,
        levels,
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
    let influence_fields = rasterize_influence_fields(&context, config);
    let samples = (0..config.sample_count())
        .into_par_iter()
        .map(|index| {
            sample_macro_field_point_with_influence(
                &context,
                config,
                config.sample_position(index),
                influence_fields.sample(index, config),
            )
        })
        .collect::<Vec<_>>();
    let stats = macro_field_stats(&samples, influence_fields.stats);

    MacroFieldTile {
        config,
        samples,
        stats,
    }
}

fn append_contour_cell_segments(
    output: &mut Vec<MacroFieldContourSegment>,
    heights: &[f32],
    config: MacroFieldTileConfig,
    width: usize,
    x: usize,
    z: usize,
    level: f32,
) {
    let i00 = z * width + x;
    let i10 = z * width + x + 1;
    let i11 = (z + 1) * width + x + 1;
    let i01 = (z + 1) * width + x;
    let p00 = config.sample_position(i00);
    let p10 = config.sample_position(i10);
    let p11 = config.sample_position(i11);
    let p01 = config.sample_position(i01);
    let mut points = Vec::with_capacity(4);

    if let Some(point) = contour_edge_intersection(p00, heights[i00], p10, heights[i10], level) {
        points.push(point);
    }
    if let Some(point) = contour_edge_intersection(p10, heights[i10], p11, heights[i11], level) {
        points.push(point);
    }
    if let Some(point) = contour_edge_intersection(p11, heights[i11], p01, heights[i01], level) {
        points.push(point);
    }
    if let Some(point) = contour_edge_intersection(p01, heights[i01], p00, heights[i00], level) {
        points.push(point);
    }

    match points.len() {
        2 => output.push(MacroFieldContourSegment {
            start: points[0],
            end: points[1],
        }),
        4 => {
            output.push(MacroFieldContourSegment {
                start: points[0],
                end: points[1],
            });
            output.push(MacroFieldContourSegment {
                start: points[2],
                end: points[3],
            });
        }
        _ => {}
    }
}

fn contour_edge_intersection(
    start: WorldPlanePoint,
    start_height: f32,
    end: WorldPlanePoint,
    end_height: f32,
    level: f32,
) -> Option<WorldPlanePoint> {
    if !start_height.is_finite() || !end_height.is_finite() {
        return None;
    }
    let delta = end_height - start_height;
    if delta.abs() <= f32::EPSILON {
        return None;
    }
    let t = (level - start_height) / delta;
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    Some(WorldPlanePoint::new(
        start.x + (end.x - start.x) * t,
        start.z + (end.z - start.z) * t,
    ))
}

pub fn sample_macro_field_point(
    context: &MacroFieldRasterContext<'_>,
    config: MacroFieldTileConfig,
    position: WorldPlanePoint,
) -> MacroFieldSample {
    let owner_sample = context.owner_sample(position, config);
    let nearest_site = owner_sample.primary;
    let surface_kind = nearest_site.map(|site| site.surface_kind);
    let biome_cell = nearest_site.and_then(|site| context.biome_for_site(site.id));
    let macro_elevation = owner_sample.macro_elevation;
    let site_coastness = nearest_site.map(|site| site.coastness).unwrap_or_default();
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
        .nearest_coast_distance(position, config.coast_radius_blocks)
        .map(|distance| envelope(distance, config.coast_radius_blocks))
        .unwrap_or(site_coastness)
        .max(site_coastness)
        .max(owner_sample.coast_boundary_blend * 0.35)
        .clamp(0.0, 1.0);
    let ridge_influence = context
        .ridge_grid
        .candidate_indices(position, config.ridge_radius_blocks)
        .into_iter()
        .filter_map(|index| context.ridge_curves.get(index))
        .map(|curve| {
            ridge_envelope(
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
        owner_sample.dry_basin_rim_blend,
        ridge_influence,
        river_valley_strength,
        river_flow_hint,
        config,
    );

    MacroFieldSample {
        position,
        nearest_site: nearest_site.map(|site| site.id),
        surface_kind,
        biome_context: biome_cell.map(|biome| biome.context),
        biome: biome_cell.map(|biome| biome.biome),
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

fn sample_macro_field_point_with_influence(
    context: &MacroFieldRasterContext<'_>,
    config: MacroFieldTileConfig,
    position: WorldPlanePoint,
    influence: MacroFieldInfluenceSample,
) -> MacroFieldSample {
    let owner_sample = context.owner_sample(position, config);
    let nearest_site = owner_sample.primary;
    let surface_kind = nearest_site.map(|site| site.surface_kind);
    let biome_cell = nearest_site.and_then(|site| context.biome_for_site(site.id));
    let macro_elevation = owner_sample.macro_elevation;
    let site_coastness = nearest_site.map(|site| site.coastness).unwrap_or_default();
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
    let coast_mask = influence
        .coast_influence
        .max(site_coastness)
        .max(owner_sample.coast_boundary_blend * 0.35)
        .clamp(0.0, 1.0);
    let ridge_influence = influence.ridge_influence;
    let river_distance_blocks = influence.river_distance_blocks;
    let river_flow_hint = influence.river_flow_hint;
    let river_valley_strength = influence.river_valley_strength;
    let combined_macro_height = combine_macro_height(
        macro_elevation,
        ocean_mask,
        coast_mask,
        lake_mask,
        dry_basin_mask,
        owner_sample.dry_basin_rim_blend,
        ridge_influence,
        river_valley_strength,
        river_flow_hint,
        config,
    );

    MacroFieldSample {
        position,
        nearest_site: nearest_site.map(|site| site.id),
        surface_kind,
        biome_context: biome_cell.map(|biome| biome.context),
        biome: biome_cell.map(|biome| biome.biome),
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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct MacroFieldInfluenceStats {
    ridge_source_curve_count: usize,
    river_source_curve_count: usize,
    coast_source_curve_count: usize,
    ridge_source_pixel_count: usize,
    river_source_pixel_count: usize,
    coast_source_pixel_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MacroFieldInfluenceSample {
    ridge_influence: f32,
    coast_influence: f32,
    river_valley_strength: f32,
    river_distance_blocks: f32,
    river_flow_hint: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct MacroFieldInfluenceFields {
    ridge_distance_blocks: Vec<f32>,
    coast_distance_blocks: Vec<f32>,
    river_distance_blocks: Vec<f32>,
    river_valley_strength: Vec<f32>,
    river_flow_hint: Vec<f32>,
    stats: MacroFieldInfluenceStats,
}

impl MacroFieldInfluenceFields {
    fn sample(&self, index: usize, config: MacroFieldTileConfig) -> MacroFieldInfluenceSample {
        let ridge_distance = self.ridge_distance_blocks[index];
        let coast_distance = self.coast_distance_blocks[index];
        let river_distance = self.river_distance_blocks[index];
        let river_flow_hint = self.river_flow_hint[index];
        let river_valley_strength = self.river_valley_strength[index];

        MacroFieldInfluenceSample {
            ridge_influence: ridge_envelope(ridge_distance, config.ridge_radius_blocks),
            coast_influence: envelope(coast_distance, config.coast_radius_blocks),
            river_valley_strength: river_valley_strength.clamp(0.0, 1.0),
            river_distance_blocks: river_distance,
            river_flow_hint,
        }
    }
}

fn rasterize_influence_fields(
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
    let ridge = rasterize_curve_distance_field(&ridge_sources, config, config.ridge_radius_blocks);
    let coast = rasterize_curve_distance_field(&coast_sources, config, config.coast_radius_blocks);
    let river = rasterize_river_chain_anti_aliased_polyline_field(
        &context.river_chains,
        config,
        config.river_radius_blocks,
    );
    let stats = MacroFieldInfluenceStats {
        ridge_source_curve_count: ridge_sources.len(),
        river_source_curve_count: context.river_chains.len(),
        coast_source_curve_count: coast_sources.len(),
        ridge_source_pixel_count: ridge.source_pixel_count,
        river_source_pixel_count: river.source_pixel_count,
        coast_source_pixel_count: coast.source_pixel_count,
    };

    MacroFieldInfluenceFields {
        ridge_distance_blocks: ridge.distance_blocks,
        coast_distance_blocks: coast.distance_blocks,
        river_distance_blocks: river.distance_blocks,
        river_valley_strength: river.river_valley_strength,
        river_flow_hint: river.flow_hint,
        stats,
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RasterDistanceField {
    distance_blocks: Vec<f32>,
    river_valley_strength: Vec<f32>,
    flow_hint: Vec<f32>,
    source_pixel_count: usize,
}

fn rasterize_curve_distance_field(
    sources: &[(&NoisyBoundaryCurve, f32)],
    config: MacroFieldTileConfig,
    radius_blocks: f32,
) -> RasterDistanceField {
    let sample_count = config.sample_count();
    if sources.is_empty() {
        return RasterDistanceField {
            distance_blocks: vec![f32::INFINITY; sample_count],
            river_valley_strength: vec![0.0; sample_count],
            flow_hint: vec![0.0; sample_count],
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
        river_valley_strength: vec![0.0; sample_count],
        flow_hint: cropped_flow,
        source_pixel_count,
    }
}

#[cfg(test)]
fn rasterize_curve_anti_aliased_polyline_field(
    sources: &[(&NoisyBoundaryCurve, f32)],
    config: MacroFieldTileConfig,
    radius_blocks: f32,
) -> RasterDistanceField {
    let chains = sources
        .iter()
        .map(|(curve, strength)| RiverChainSource {
            points: curve.points.clone(),
            flow_hints: vec![*strength; curve.points.len()],
        })
        .collect::<Vec<_>>();
    rasterize_river_chain_anti_aliased_polyline_field(&chains, config, radius_blocks)
}

fn rasterize_river_chain_anti_aliased_polyline_field(
    sources: &[RiverChainSource],
    config: MacroFieldTileConfig,
    radius_blocks: f32,
) -> RasterDistanceField {
    let sample_count = config.sample_count();
    if sources.is_empty() {
        return RasterDistanceField {
            distance_blocks: vec![f32::INFINITY; sample_count],
            river_valley_strength: vec![0.0; sample_count],
            flow_hint: vec![0.0; sample_count],
            source_pixel_count: 0,
        };
    }

    let width = config.width as usize;
    let height = config.height as usize;
    let mut distance_blocks = vec![f32::INFINITY; sample_count];
    let mut river_valley_strength = vec![0.0; sample_count];
    let mut flow_weighted_sum = vec![0.0; sample_count];
    let mut flow_weight_sum = vec![0.0; sample_count];

    for source in sources {
        for index in 0..source.points.len().saturating_sub(1) {
            rasterize_segment_anti_aliased_stroke(
                &mut distance_blocks,
                &mut river_valley_strength,
                &mut flow_weighted_sum,
                &mut flow_weight_sum,
                width,
                height,
                config,
                source.points[index],
                source.points[index + 1],
                radius_blocks,
                source.flow_hints[index],
                source.flow_hints[index + 1],
            );
        }
    }

    let source_pixel_count = river_valley_strength
        .iter()
        .filter(|strength| **strength > 0.001)
        .count();
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

    RasterDistanceField {
        distance_blocks,
        river_valley_strength,
        flow_hint,
        source_pixel_count,
    }
}

#[allow(clippy::too_many_arguments)]
fn rasterize_segment_anti_aliased_stroke(
    distance_blocks: &mut [f32],
    river_valley_strength: &mut [f32],
    flow_weighted_sum: &mut [f32],
    flow_weight_sum: &mut [f32],
    width: usize,
    height: usize,
    config: MacroFieldTileConfig,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    radius_blocks: f32,
    start_flow_hint: f32,
    end_flow_hint: f32,
) {
    let spacing = config.sample_spacing_blocks;
    let aa_margin = spacing * 0.75;
    let min_x = ((start.x.min(end.x) - radius_blocks - aa_margin - config.origin.x) / spacing)
        .floor()
        .max(0.0) as usize;
    let max_x = ((start.x.max(end.x) + radius_blocks + aa_margin - config.origin.x) / spacing)
        .ceil()
        .min((width.saturating_sub(1)) as f32) as usize;
    let min_z = ((start.z.min(end.z) - radius_blocks - aa_margin - config.origin.z) / spacing)
        .floor()
        .max(0.0) as usize;
    let max_z = ((start.z.max(end.z) + radius_blocks + aa_margin - config.origin.z) / spacing)
        .ceil()
        .min((height.saturating_sub(1)) as f32) as usize;
    if min_x > max_x || min_z > max_z {
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
    for z in min_z..=max_z {
        for x in min_x..=max_x {
            let index = z * width + x;
            let position = config.sample_position(index);
            let distance = point_segment_distance(position, start, end);
            if distance > radius_blocks + aa_margin {
                continue;
            }
            let mut profile_sum = 0.0;
            let mut flow_profile_sum = 0.0;
            let mut closest_subpixel_distance = distance;
            for (offset_x, offset_z) in subpixel_offsets {
                let subpixel = WorldPlanePoint::new(
                    position.x + offset_x * spacing,
                    position.z + offset_z * spacing,
                );
                let (subpixel_distance, segment_t) =
                    point_segment_distance_and_t(subpixel, start, end);
                let subpixel_flow = lerp(start_flow_hint, end_flow_hint, segment_t).clamp(0.0, 1.0);
                closest_subpixel_distance = closest_subpixel_distance.min(subpixel_distance);
                profile_sum += river_valley_strength_for_distance(
                    subpixel_distance,
                    subpixel_flow,
                    radius_blocks,
                );
                flow_profile_sum += subpixel_flow;
            }
            let anti_aliased_strength = (profile_sum / subpixel_count).clamp(0.0, 1.0);
            if anti_aliased_strength <= 0.0 {
                continue;
            }
            let blended_flow = (flow_profile_sum / subpixel_count).clamp(0.0, 1.0);

            distance_blocks[index] = distance_blocks[index].min(closest_subpixel_distance);
            river_valley_strength[index] = river_valley_strength[index].max(anti_aliased_strength);
            flow_weighted_sum[index] += blended_flow * anti_aliased_strength;
            flow_weight_sum[index] += anti_aliased_strength;
        }
    }
}

fn rasterize_curve_sources(
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
fn rasterize_segment_sources(
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

fn propagate_chamfer_distance(
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
fn update_from_neighbor(
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

#[derive(Debug)]
pub struct MacroFieldRasterContext<'a> {
    sites: &'a [MacroSite],
    site_by_id: HashMap<VoronoiSiteId, MacroSite>,
    biome_by_site_id: HashMap<VoronoiSiteId, GraphBiomeCell>,
    site_grid: SiteIndexGrid,
    boundary_edges: Vec<BoundaryEdgeRef<'a>>,
    boundary_grid: CurveIndexGrid,
    coast_curves: Vec<&'a NoisyBoundaryCurve>,
    coast_grid: CurveIndexGrid,
    ridge_curves: Vec<&'a NoisyBoundaryCurve>,
    ridge_grid: CurveIndexGrid,
    river_chains: Vec<RiverChainSource>,
}

impl<'a> MacroFieldRasterContext<'a> {
    pub fn new(
        _patch: &'a VoronoiGraphPatch,
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
        let site_by_id = macro_map
            .sites
            .iter()
            .map(|site| (site.id, *site))
            .collect::<HashMap<_, _>>();
        let biome_by_site_id = macro_map
            .biomes
            .iter()
            .map(|biome| (biome.site, *biome))
            .collect::<HashMap<_, _>>();
        let site_grid = SiteIndexGrid::from_sites(&macro_map.sites);
        let coast_curves = macro_map
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
        let ridge_curves = macro_map
            .edges
            .iter()
            .filter_map(|edge| {
                macro_edges
                    .get(&edge.id)
                    .is_some_and(|macro_edge| {
                        macro_edge.guide.is_ridge_candidate
                            && macro_edge.guide.ridgeness >= RIDGE_FIELD_SOURCE_MIN_RIDGENESS
                    })
                    .then(|| boundary_curves.get(&edge.id).copied())
                    .flatten()
            })
            .collect::<Vec<_>>();
        let river_chains = build_river_chain_sources(hydrology, &boundary_curves);
        let mut boundary_edges = macro_map
            .edges
            .iter()
            .filter_map(|edge| {
                let left = site_by_id.get(&edge.sites[0]).copied()?;
                let right = site_by_id.get(&edge.sites[1]).copied()?;
                let curve = boundary_curves.get(&edge.id).copied()?;
                Some(BoundaryEdgeRef { curve, left, right })
            })
            .collect::<Vec<_>>();
        boundary_edges.sort_by_key(|edge| edge.curve.edge.0);
        let boundary_grid = CurveIndexGrid::from_boundary_edges(&boundary_edges);
        let coast_grid = CurveIndexGrid::from_curves(&coast_curves);
        let ridge_grid = CurveIndexGrid::from_curves(&ridge_curves);

        Self {
            sites: &macro_map.sites,
            site_by_id,
            biome_by_site_id,
            site_grid,
            boundary_edges,
            boundary_grid,
            coast_curves,
            coast_grid,
            ridge_curves,
            ridge_grid,
            river_chains,
        }
    }

    pub fn nearest_site(&self, position: WorldPlanePoint) -> Option<&'a MacroSite> {
        let candidates = self.site_grid.candidate_indices(position);
        let iter: Box<dyn Iterator<Item = usize> + '_> = if candidates.is_empty() {
            Box::new(0..self.sites.len())
        } else {
            Box::new(candidates.into_iter())
        };
        iter.filter_map(|index| self.sites.get(index))
            .min_by(|left, right| {
                squared_distance(position, left.position)
                    .total_cmp(&squared_distance(position, right.position))
                    .then_with(|| left.id.0.cmp(&right.id.0))
            })
    }

    pub fn biome_for_site(&self, site: VoronoiSiteId) -> Option<GraphBiomeCell> {
        self.biome_by_site_id.get(&site).copied()
    }

    fn owner_sample(&self, position: WorldPlanePoint, config: MacroFieldTileConfig) -> OwnerSample {
        let nearest = self.nearest_site(position);
        let Some(boundary) = self.nearest_boundary(position, config.boundary_blend_radius_blocks)
        else {
            return OwnerSample::from_site(nearest);
        };
        if boundary.distance > config.boundary_blend_radius_blocks {
            return OwnerSample::from_site(nearest);
        }

        let primary = boundary.primary_site();
        let secondary = boundary.secondary_site();
        let is_coast_pair = is_explicit_coast_pair(primary, secondary);
        let is_dry_basin_pair = primary.surface_kind == MacroSurfaceKind::DryBasin
            && secondary.surface_kind != MacroSurfaceKind::DryBasin;
        let blend = envelope(boundary.distance, config.boundary_blend_radius_blocks);
        let mixed_elevation = if is_coast_pair {
            coast_boundary_elevation_profile(
                primary,
                boundary.distance,
                config.boundary_blend_radius_blocks,
            )
        } else {
            primary.signed_macro_elevation * (1.0 - blend * 0.35)
                + secondary.signed_macro_elevation * (blend * 0.35)
        };

        OwnerSample {
            primary: self
                .site_by_id
                .get(&primary.id)
                .copied()
                .or(nearest.copied()),
            macro_elevation: mixed_elevation,
            coast_boundary_blend: if is_coast_pair { blend } else { 0.0 },
            dry_basin_rim_blend: if is_dry_basin_pair { blend } else { 0.0 },
        }
    }

    fn nearest_boundary(
        &self,
        position: WorldPlanePoint,
        radius: f32,
    ) -> Option<BoundarySideSample> {
        self.boundary_grid
            .candidate_indices(position, radius)
            .into_iter()
            .filter_map(|index| self.boundary_edges[index].side_sample(position))
            .min_by(|left, right| left.distance.total_cmp(&right.distance))
    }

    fn nearest_coast_distance(&self, position: WorldPlanePoint, radius: f32) -> Option<f32> {
        self.coast_grid
            .candidate_indices(position, radius)
            .into_iter()
            .filter_map(|index| self.coast_curves.get(index))
            .map(|curve| polyline_distance(position, &curve.points))
            .min_by(f32::total_cmp)
    }

    fn river_valley(
        &self,
        position: WorldPlanePoint,
        config: MacroFieldTileConfig,
    ) -> (f32, f32, f32) {
        let Some((distance, flow)) = self
            .river_chains
            .iter()
            .filter_map(|chain| nearest_river_chain_sample(position, chain))
            .min_by(|left, right| left.0.total_cmp(&right.0))
        else {
            return (f32::INFINITY, 0.0, 0.0);
        };
        let valley = river_valley_strength_for_distance(distance, flow, config.river_radius_blocks);

        (distance, flow, valley.clamp(0.0, 1.0))
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RiverChainSource {
    points: Vec<WorldPlanePoint>,
    flow_hints: Vec<f32>,
}

fn build_river_chain_sources<'a>(
    hydrology: &'a GraphHydrologyGraph,
    boundary_curves: &HashMap<VoronoiEdgeId, &'a NoisyBoundaryCurve>,
) -> Vec<RiverChainSource> {
    let node_positions = hydrology
        .nodes
        .iter()
        .map(|node| (node.id, node.position))
        .collect::<HashMap<_, _>>();
    let selected_segments = hydrology
        .segments
        .iter()
        .filter(|segment| boundary_curves.contains_key(&segment.edge))
        .collect::<Vec<_>>();
    let mut outgoing_by_from = HashMap::<GraphDrainageNodeId, Vec<&GraphRiverSegment>>::new();
    let mut incoming_count_by_to = HashMap::<GraphDrainageNodeId, usize>::new();
    for segment in &selected_segments {
        outgoing_by_from
            .entry(segment.from)
            .or_default()
            .push(*segment);
        *incoming_count_by_to.entry(segment.to).or_default() += 1;
    }
    for outgoing in outgoing_by_from.values_mut() {
        outgoing.sort_by(|left, right| {
            right
                .flow_accumulation
                .total_cmp(&left.flow_accumulation)
                .then_with(|| left.id.0.cmp(&right.id.0))
        });
    }

    let mut chain_starts = selected_segments
        .iter()
        .copied()
        .filter(|segment| {
            let incoming = incoming_count_by_to
                .get(&segment.from)
                .copied()
                .unwrap_or(0);
            let outgoing = outgoing_by_from.get(&segment.from).map_or(0, Vec::len);
            incoming != 1 || outgoing != 1
        })
        .collect::<Vec<_>>();
    chain_starts.sort_by(|left, right| {
        left.from
            .0
            .cmp(&right.from.0)
            .then_with(|| left.to.0.cmp(&right.to.0))
            .then_with(|| left.id.0.cmp(&right.id.0))
    });

    let mut visited = HashSet::<u64>::new();
    let mut sources = Vec::new();
    for start in chain_starts {
        if visited.contains(&start.id.0) {
            continue;
        }
        if let Some(source) = build_river_chain_source(
            start,
            &outgoing_by_from,
            &incoming_count_by_to,
            &node_positions,
            boundary_curves,
            &mut visited,
        ) {
            sources.push(source);
        }
    }

    let mut remaining = selected_segments;
    remaining.sort_by_key(|segment| segment.id.0);
    for start in remaining {
        if visited.contains(&start.id.0) {
            continue;
        }
        if let Some(source) = build_river_chain_source(
            start,
            &outgoing_by_from,
            &incoming_count_by_to,
            &node_positions,
            boundary_curves,
            &mut visited,
        ) {
            sources.push(source);
        }
    }

    sources.sort_by(|left, right| {
        left.points
            .first()
            .map(|point| point.x)
            .unwrap_or_default()
            .total_cmp(
                &right
                    .points
                    .first()
                    .map(|point| point.x)
                    .unwrap_or_default(),
            )
            .then_with(|| {
                left.points
                    .first()
                    .map(|point| point.z)
                    .unwrap_or_default()
                    .total_cmp(
                        &right
                            .points
                            .first()
                            .map(|point| point.z)
                            .unwrap_or_default(),
                    )
            })
    });
    sources
}

fn build_river_chain_source<'a>(
    start: &'a GraphRiverSegment,
    outgoing_by_from: &HashMap<GraphDrainageNodeId, Vec<&'a GraphRiverSegment>>,
    incoming_count_by_to: &HashMap<GraphDrainageNodeId, usize>,
    node_positions: &HashMap<GraphDrainageNodeId, WorldPlanePoint>,
    boundary_curves: &HashMap<VoronoiEdgeId, &'a NoisyBoundaryCurve>,
    visited: &mut HashSet<u64>,
) -> Option<RiverChainSource> {
    let mut points = Vec::<WorldPlanePoint>::new();
    let mut flow_hints = Vec::<f32>::new();
    let mut current = start;

    loop {
        if !visited.insert(current.id.0) {
            break;
        }
        let curve = boundary_curves.get(&current.edge)?;
        let segment_points = oriented_river_segment_points(curve, current, node_positions);
        append_river_segment_points(
            &mut points,
            &mut flow_hints,
            &segment_points,
            flow_hint(current.flow_accumulation),
        );

        let incoming_at_target = incoming_count_by_to.get(&current.to).copied().unwrap_or(0);
        let Some(outgoing) = outgoing_by_from.get(&current.to) else {
            break;
        };
        if incoming_at_target != 1 || outgoing.len() != 1 {
            break;
        }
        current = outgoing[0];
        if visited.contains(&current.id.0) {
            break;
        }
    }

    smooth_river_chain_source(points, flow_hints)
}

fn oriented_river_segment_points(
    curve: &NoisyBoundaryCurve,
    segment: &GraphRiverSegment,
    node_positions: &HashMap<GraphDrainageNodeId, WorldPlanePoint>,
) -> Vec<WorldPlanePoint> {
    let mut points = curve.points.clone();
    let Some(from) = node_positions.get(&segment.from).copied() else {
        return points;
    };
    let Some(to) = node_positions.get(&segment.to).copied() else {
        return points;
    };
    let forward_score =
        squared_distance(from, curve.anchors.start) + squared_distance(to, curve.anchors.end);
    let reverse_score =
        squared_distance(from, curve.anchors.end) + squared_distance(to, curve.anchors.start);
    if reverse_score + 0.001 < forward_score {
        points.reverse();
    }
    points
}

fn append_river_segment_points(
    points: &mut Vec<WorldPlanePoint>,
    flow_hints: &mut Vec<f32>,
    segment_points: &[WorldPlanePoint],
    segment_flow_hint: f32,
) {
    if segment_points.is_empty() {
        return;
    }
    let mut start_index = 0;
    if let Some(last) = points.last().copied() {
        if squared_distance(last, segment_points[0]) <= 0.01 {
            if let Some(last_flow) = flow_hints.last_mut() {
                *last_flow = (*last_flow + segment_flow_hint) * 0.5;
            }
            start_index = 1;
        }
    }
    for point in &segment_points[start_index..] {
        points.push(*point);
        flow_hints.push(segment_flow_hint.clamp(0.0, 1.0));
    }
}

fn smooth_river_chain_source(
    points: Vec<WorldPlanePoint>,
    flow_hints: Vec<f32>,
) -> Option<RiverChainSource> {
    if points.len() < 2 || points.len() != flow_hints.len() {
        return None;
    }
    let flow_hints = smooth_river_flow_hints(&flow_hints);
    if points.len() < 3 {
        return Some(RiverChainSource { points, flow_hints });
    }

    let mut smoothed_points = Vec::with_capacity(points.len() * 2);
    let mut smoothed_flows = Vec::with_capacity(flow_hints.len() * 2);
    smoothed_points.push(points[0]);
    smoothed_flows.push(flow_hints[0]);
    for index in 0..points.len() - 1 {
        let left = points[index];
        let right = points[index + 1];
        let left_flow = flow_hints[index];
        let right_flow = flow_hints[index + 1];
        smoothed_points.push(lerp_point(left, right, 0.25));
        smoothed_flows.push(lerp(left_flow, right_flow, 0.25));
        smoothed_points.push(lerp_point(left, right, 0.75));
        smoothed_flows.push(lerp(left_flow, right_flow, 0.75));
    }
    smoothed_points.push(*points.last().expect("checked non-empty points"));
    smoothed_flows.push(*flow_hints.last().expect("checked non-empty flows"));

    Some(RiverChainSource {
        points: smoothed_points,
        flow_hints: smooth_river_flow_hints(&smoothed_flows),
    })
}

fn smooth_river_flow_hints(flow_hints: &[f32]) -> Vec<f32> {
    if flow_hints.len() <= 2 {
        return flow_hints.iter().map(|flow| flow.clamp(0.0, 1.0)).collect();
    }
    let mut smoothed = flow_hints
        .iter()
        .map(|flow| flow.clamp(0.0, 1.0))
        .collect::<Vec<_>>();
    for _ in 0..2 {
        let previous = smoothed.clone();
        for index in 1..previous.len() - 1 {
            smoothed[index] =
                previous[index - 1] * 0.25 + previous[index] * 0.5 + previous[index + 1] * 0.25;
        }
    }
    for index in 1..smoothed.len() {
        smoothed[index] = smoothed[index]
            .max(smoothed[index - 1] * 0.88)
            .clamp(0.0, 1.0);
    }
    smoothed
}

fn nearest_river_chain_sample(
    position: WorldPlanePoint,
    chain: &RiverChainSource,
) -> Option<(f32, f32)> {
    if chain.points.len() < 2 || chain.points.len() != chain.flow_hints.len() {
        return None;
    }
    (0..chain.points.len() - 1)
        .map(|index| {
            let (distance, t) = point_segment_distance_and_t(
                position,
                chain.points[index],
                chain.points[index + 1],
            );
            let flow = lerp(chain.flow_hints[index], chain.flow_hints[index + 1], t);
            (distance, flow.clamp(0.0, 1.0))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
}

#[derive(Debug, Clone, Copy)]
struct BoundaryEdgeRef<'a> {
    curve: &'a NoisyBoundaryCurve,
    left: MacroSite,
    right: MacroSite,
}

impl BoundaryEdgeRef<'_> {
    fn side_sample(self, position: WorldPlanePoint) -> Option<BoundarySideSample> {
        let nearest = nearest_polyline_segment(position, &self.curve.points)?;
        let side = signed_side(position, nearest.start, nearest.end);
        let left_side = signed_side(self.left.position, nearest.start, nearest.end);
        let right_side = signed_side(self.right.position, nearest.start, nearest.end);
        Some(BoundarySideSample {
            distance: nearest.distance,
            side,
            left_side,
            right_side,
            left: self.left,
            right: self.right,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct BoundarySideSample {
    distance: f32,
    side: f32,
    left_side: f32,
    right_side: f32,
    left: MacroSite,
    right: MacroSite,
}

impl BoundarySideSample {
    fn primary_site(self) -> MacroSite {
        if self.matches_left_side() {
            self.left
        } else {
            self.right
        }
    }

    fn secondary_site(self) -> MacroSite {
        if self.matches_left_side() {
            self.right
        } else {
            self.left
        }
    }

    fn matches_left_side(self) -> bool {
        let left_side = usable_side(self.left_side, self.right_side);
        let sample_side = if self.side.abs() <= f32::EPSILON {
            left_side
        } else {
            self.side
        };
        sample_side.signum() == left_side.signum()
    }
}

#[derive(Debug, Clone, Copy)]
struct OwnerSample {
    primary: Option<MacroSite>,
    macro_elevation: f32,
    coast_boundary_blend: f32,
    dry_basin_rim_blend: f32,
}

impl OwnerSample {
    fn from_site(site: Option<&MacroSite>) -> Self {
        Self {
            primary: site.copied(),
            macro_elevation: site
                .map(|site| site.signed_macro_elevation)
                .unwrap_or_default(),
            coast_boundary_blend: 0.0,
            dry_basin_rim_blend: 0.0,
        }
    }
}

fn is_explicit_coast_pair(a: MacroSite, b: MacroSite) -> bool {
    a.surface_kind.is_ocean_owned() != b.surface_kind.is_ocean_owned()
}

fn coast_boundary_elevation_profile(
    primary: MacroSite,
    distance_to_curve_blocks: f32,
    blend_radius_blocks: f32,
) -> f32 {
    let recovery = smoothstep01(distance_to_curve_blocks / blend_radius_blocks.max(f32::EPSILON));
    if primary.surface_kind.is_ocean_owned() {
        let shallow_water = -0.012;
        shallow_water * (1.0 - recovery) + primary.signed_macro_elevation.min(-0.01) * recovery
    } else {
        primary.signed_macro_elevation.max(0.0) * recovery
    }
}

#[derive(Debug, Clone, Copy)]
struct NearestPolylineSegment {
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    distance: f32,
}

#[derive(Debug, Clone, Default)]
struct CurveIndexGrid {
    buckets: HashMap<(i32, i32), Vec<usize>>,
}

impl CurveIndexGrid {
    fn from_curves(curves: &[&NoisyBoundaryCurve]) -> Self {
        let mut grid = Self::default();
        for (index, curve) in curves.iter().enumerate() {
            grid.insert_curve(index, &curve.points);
        }
        grid
    }

    fn from_boundary_edges(edges: &[BoundaryEdgeRef<'_>]) -> Self {
        let mut grid = Self::default();
        for (index, edge) in edges.iter().enumerate() {
            grid.insert_curve(index, &edge.curve.points);
        }
        grid
    }

    fn insert_curve(&mut self, index: usize, points: &[WorldPlanePoint]) {
        for point in points {
            self.buckets
                .entry(curve_bucket(*point))
                .or_default()
                .push(index);
        }
    }

    fn candidate_indices(&self, position: WorldPlanePoint, radius: f32) -> Vec<usize> {
        if self.buckets.is_empty() {
            return Vec::new();
        }
        let center = curve_bucket(position);
        let search = (radius.max(MACRO_FIELD_CURVE_BUCKET_BLOCKS) / MACRO_FIELD_CURVE_BUCKET_BLOCKS)
            .ceil() as i32
            + 1;
        let mut indices = Vec::new();
        for z in center.1 - search..=center.1 + search {
            for x in center.0 - search..=center.0 + search {
                if let Some(bucket) = self.buckets.get(&(x, z)) {
                    indices.extend(bucket.iter().copied());
                }
            }
        }
        indices.sort_unstable();
        indices.dedup();
        indices
    }
}

#[derive(Debug, Clone, Default)]
struct SiteIndexGrid {
    buckets: HashMap<(i32, i32), Vec<usize>>,
}

impl SiteIndexGrid {
    fn from_sites(sites: &[MacroSite]) -> Self {
        let mut grid = Self::default();
        for (index, site) in sites.iter().enumerate() {
            grid.buckets
                .entry(curve_bucket(site.position))
                .or_default()
                .push(index);
        }
        grid
    }

    fn candidate_indices(&self, position: WorldPlanePoint) -> Vec<usize> {
        let center = curve_bucket(position);
        for search in 1..=4 {
            let mut indices = Vec::new();
            for z in center.1 - search..=center.1 + search {
                for x in center.0 - search..=center.0 + search {
                    if let Some(bucket) = self.buckets.get(&(x, z)) {
                        indices.extend(bucket.iter().copied());
                    }
                }
            }
            if !indices.is_empty() {
                indices.sort_unstable();
                indices.dedup();
                return indices;
            }
        }
        Vec::new()
    }
}

fn validate_macro_field_config(config: MacroFieldTileConfig) {
    assert!(config.width > 0, "macro field tile width must be > 0");
    assert!(config.height > 0, "macro field tile height must be > 0");
    for (name, value) in [
        ("sample_spacing_blocks", config.sample_spacing_blocks),
        ("ridge_radius_blocks", config.ridge_radius_blocks),
        ("river_radius_blocks", config.river_radius_blocks),
        ("coast_radius_blocks", config.coast_radius_blocks),
        (
            "boundary_blend_radius_blocks",
            config.boundary_blend_radius_blocks,
        ),
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
    assert!(
        config.boundary_blend_radius_blocks > 0.0,
        "boundary blend radius must be > 0"
    );
}

fn combine_macro_height(
    macro_elevation: f32,
    ocean_mask: f32,
    coast_mask: f32,
    lake_mask: f32,
    dry_basin_mask: f32,
    dry_basin_rim_blend: f32,
    ridge_influence: f32,
    river_valley_strength: f32,
    river_flow_hint: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    let ridge_raise = ridge_influence * config.ridge_height_scale;
    let river_carve = river_valley_strength
        * config.river_carve_scale
        * (0.86 + river_flow_hint * 0.10)
        * (1.0 - ocean_mask);
    let dry_basin = dry_basin_mask > 0.5;
    let coast_flatten = if dry_basin {
        0.0
    } else {
        coast_mask * config.coast_flatten_strength
    };
    let lake_flatten = lake_mask * config.lake_flatten_strength;
    let flatten = coast_flatten.max(lake_flatten).clamp(0.0, 1.0);
    let flatten_target = if ocean_mask > 0.5 || lake_mask > 0.5 {
        -0.035
    } else {
        0.0
    };
    let mut height = macro_elevation + ridge_raise - river_carve;
    height = height + (flatten_target - height) * flatten;
    if dry_basin {
        height = dry_basin_height_profile(height, dry_basin_rim_blend);
    }
    height.clamp(-2.0, 2.0)
}

fn dry_basin_height_profile(height: f32, rim_blend: f32) -> f32 {
    let lowered_floor = (height - DRY_BASIN_FLOOR_LOWERING).max(DRY_BASIN_MIN_HEIGHT);
    let preserved_variation = height.max(0.0) * 0.42;
    let interior = lowered_floor.max(DRY_BASIN_MIN_HEIGHT + preserved_variation);
    let rim = smoothstep01(rim_blend) * DRY_BASIN_RIM_RAISE;
    interior + rim
}

fn macro_field_stats(
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

fn smoothstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn ridge_envelope(distance: f32, radius: f32) -> f32 {
    let raw = envelope(distance, radius);
    if raw <= RIDGE_INFLUENCE_VISIBLE_FLOOR {
        0.0
    } else {
        let t = ((raw - RIDGE_INFLUENCE_VISIBLE_FLOOR) / (1.0 - RIDGE_INFLUENCE_VISIBLE_FLOOR))
            .clamp(0.0, 1.0);
        (t * t * (3.0 - 2.0 * t)).powf(0.92)
    }
}

fn flow_hint(flow_accumulation: f32) -> f32 {
    (flow_accumulation.max(0.0).sqrt() / 32.0).clamp(0.0, 1.0)
}

fn river_width_blocks(flow_hint: f32, configured_radius_blocks: f32) -> f32 {
    let t = flow_hint.clamp(0.0, 1.0).powf(1.35);
    let width = RIVER_MIN_WIDTH_BLOCKS + (RIVER_MAX_WIDTH_BLOCKS - RIVER_MIN_WIDTH_BLOCKS) * t;
    width.min(configured_radius_blocks.max(RIVER_MIN_WIDTH_BLOCKS))
}

fn river_depth_factor(flow_hint: f32) -> f32 {
    RIVER_HEADWATER_DEPTH_FACTOR
        + (RIVER_TRUNK_DEPTH_FACTOR - RIVER_HEADWATER_DEPTH_FACTOR)
            * flow_hint.clamp(0.0, 1.0).powf(1.05)
}

fn river_flat_bed_radius_blocks(flow_hint: f32, configured_radius_blocks: f32) -> f32 {
    let t = flow_hint.clamp(0.0, 1.0).powf(1.05);
    let flat =
        RIVER_MIN_FLAT_BED_BLOCKS + (RIVER_MAX_FLAT_BED_BLOCKS - RIVER_MIN_FLAT_BED_BLOCKS) * t;
    flat.min(river_width_blocks(flow_hint, configured_radius_blocks) * 0.42)
}

fn river_valley_strength_for_distance(
    distance_blocks: f32,
    flow_hint: f32,
    configured_radius_blocks: f32,
) -> f32 {
    let width = river_width_blocks(flow_hint, configured_radius_blocks);
    let flat_bed = river_flat_bed_radius_blocks(flow_hint, configured_radius_blocks);
    let depth = river_depth_factor(flow_hint);
    if !distance_blocks.is_finite() || distance_blocks >= width {
        return 0.0;
    }
    if distance_blocks <= flat_bed {
        return depth;
    }
    let shoulder_t =
        ((distance_blocks - flat_bed) / (width - flat_bed).max(f32::EPSILON)).clamp(0.0, 1.0);
    let shoulder = 1.0 - smoothstep01(shoulder_t);
    depth * shoulder.powf(1.35)
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

fn nearest_polyline_segment(
    position: WorldPlanePoint,
    points: &[WorldPlanePoint],
) -> Option<NearestPolylineSegment> {
    match points {
        [] | [_] => None,
        _ => points
            .windows(2)
            .map(|segment| NearestPolylineSegment {
                start: segment[0],
                end: segment[1],
                distance: point_segment_distance(position, segment[0], segment[1]),
            })
            .min_by(|left, right| left.distance.total_cmp(&right.distance)),
    }
}

fn point_segment_distance(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    point_segment_distance_and_t(point, start, end).0
}

fn point_segment_distance_and_t(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> (f32, f32) {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    let len2 = dx * dx + dz * dz;
    if len2 <= f32::EPSILON {
        return (squared_distance(point, start).sqrt(), 0.0);
    }
    let t = (((point.x - start.x) * dx + (point.z - start.z) * dz) / len2).clamp(0.0, 1.0);
    let projected = WorldPlanePoint::new(start.x + dx * t, start.z + dz * t);
    (squared_distance(point, projected).sqrt(), t)
}

fn lerp(left: f32, right: f32, t: f32) -> f32 {
    left + (right - left) * t.clamp(0.0, 1.0)
}

fn lerp_point(left: WorldPlanePoint, right: WorldPlanePoint, t: f32) -> WorldPlanePoint {
    WorldPlanePoint::new(lerp(left.x, right.x, t), lerp(left.z, right.z, t))
}

fn squared_distance(a: WorldPlanePoint, b: WorldPlanePoint) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}

fn curve_bucket(point: WorldPlanePoint) -> (i32, i32) {
    (
        (point.x / MACRO_FIELD_CURVE_BUCKET_BLOCKS).floor() as i32,
        (point.z / MACRO_FIELD_CURVE_BUCKET_BLOCKS).floor() as i32,
    )
}

fn signed_side(point: WorldPlanePoint, start: WorldPlanePoint, end: WorldPlanePoint) -> f32 {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    (point.x - start.x) * dz - (point.z - start.z) * dx
}

fn usable_side(preferred: f32, fallback: f32) -> f32 {
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
    fn dry_basin_height_is_shallow_land_floor_not_water_flatten() {
        let config = test_tile_config();
        let dry_height = combine_macro_height(0.18, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, config);
        let water_height =
            combine_macro_height(0.18, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, config);

        assert!(
            dry_height >= DRY_BASIN_MIN_HEIGHT,
            "dry basin should remain an above-sea-level land floor: {dry_height}"
        );
        assert!(
            dry_height > water_height,
            "dry basin should not use lake/ocean water flatten: dry={dry_height} water={water_height}"
        );
    }

    #[test]
    fn dry_basin_profile_preserves_variation_and_rim_rise() {
        let low_floor = dry_basin_height_profile(0.08, 0.0);
        let high_floor = dry_basin_height_profile(0.28, 0.0);
        let rim = dry_basin_height_profile(0.08, 1.0);

        assert!(
            high_floor > low_floor + 0.05,
            "dry basin profile should not collapse interior macro variation into one plateau: low={low_floor} high={high_floor}"
        );
        assert!(
            rim > low_floor + 0.12,
            "dry basin boundary should rise toward a rim rather than stay flat: rim={rim} floor={low_floor}"
        );
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
    fn rasterized_ridge_influence_tile_has_near_stronger_than_far_sample() {
        let inputs = test_inputs(42);
        let Some(ridge_curve) = inputs.boundary.curves.iter().find(|curve| {
            inputs
                .macro_map
                .edge(curve.edge)
                .is_some_and(|edge| edge.guide.is_ridge_candidate)
        }) else {
            return;
        };
        let near = ridge_curve.points[ridge_curve.points.len() / 2];
        let tile = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.hydrology,
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
            &inputs.hydrology,
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
        let field = rasterize_curve_anti_aliased_polyline_field(&[(&curve, 0.5)], config, 32.0);

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
            &[(&left, 0.25), (&right, 0.75)],
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
            (field.flow_hint[joint - 1] - field.flow_hint[joint + 1]).abs() < 0.5,
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
        let field = rasterize_curve_anti_aliased_polyline_field(&[(&curve, 0.75)], config, 32.0);
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
    fn river_anti_aliased_polyline_preserves_flow_scaled_flat_bed_width() {
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
        let headwater =
            rasterize_curve_anti_aliased_polyline_field(&[(&curve, flow_hint(12.0))], config, 96.0);
        let trunk = rasterize_curve_anti_aliased_polyline_field(
            &[(&curve, flow_hint(1024.0))],
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
    fn river_chain_builder_splits_at_confluence_and_branch_nodes() {
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::hydrology::{
            GraphDrainageNode, GraphDrainageNodeId, GraphDrainageNodeKind, GraphHydrologyGraph,
            GraphHydrologyRole, GraphRiverSegment, GraphRiverSegmentId, WatershedId,
        };

        let watershed = WatershedId(1);
        let nodes = [
            (GraphDrainageNodeId(1), WorldPlanePoint::new(0.0, 0.0)),
            (GraphDrainageNodeId(2), WorldPlanePoint::new(64.0, 0.0)),
            (GraphDrainageNodeId(3), WorldPlanePoint::new(64.0, 64.0)),
            (GraphDrainageNodeId(4), WorldPlanePoint::new(128.0, 32.0)),
            (GraphDrainageNodeId(5), WorldPlanePoint::new(192.0, 0.0)),
            (GraphDrainageNodeId(6), WorldPlanePoint::new(192.0, 64.0)),
        ];
        let hydrology = GraphHydrologyGraph {
            nodes: nodes
                .iter()
                .map(|(id, position)| GraphDrainageNode {
                    id: *id,
                    kind: GraphDrainageNodeKind::Confluence,
                    corner: VoronoiCornerId(id.0),
                    position: *position,
                    watershed,
                })
                .collect(),
            segments: vec![
                test_river_segment(1, 1, 1, 2, 64.0),
                test_river_segment(2, 2, 3, 2, 72.0),
                test_river_segment(3, 3, 2, 4, 144.0),
                test_river_segment(4, 4, 4, 5, 160.0),
                test_river_segment(5, 5, 4, 6, 150.0),
            ],
            ..GraphHydrologyGraph::default()
        };
        let curves = vec![
            test_noisy_curve(1, 1, 2, nodes[0].1, nodes[1].1),
            test_noisy_curve(2, 3, 2, nodes[2].1, nodes[1].1),
            test_noisy_curve(3, 2, 4, nodes[1].1, nodes[3].1),
            test_noisy_curve(4, 4, 5, nodes[3].1, nodes[4].1),
            test_noisy_curve(5, 4, 6, nodes[3].1, nodes[5].1),
        ];
        let curve_map = curves
            .iter()
            .map(|curve| (curve.edge, curve))
            .collect::<HashMap<_, _>>();

        let chains = build_river_chain_sources(&hydrology, &curve_map);

        assert_eq!(
            chains.len(),
            5,
            "confluence and branch nodes should split render chains instead of sewing unrelated paths"
        );
        assert!(
            chains.iter().all(|chain| chain.points.len() >= 2),
            "each selected branch should still provide a renderable centerline"
        );
        assert!(
            chains
                .iter()
                .any(|chain| chain.points.first() == Some(&nodes[1].1)
                    && chain.points.last() == Some(&nodes[3].1)),
            "downstream segment after confluence should begin as its own chain"
        );

        fn test_river_segment(
            id: u64,
            edge: u64,
            from: u64,
            to: u64,
            flow_accumulation: f32,
        ) -> GraphRiverSegment {
            GraphRiverSegment {
                id: GraphRiverSegmentId(id),
                edge: VoronoiEdgeId(edge),
                from: GraphDrainageNodeId(from),
                to: GraphDrainageNodeId(to),
                watershed: WatershedId(1),
                role: GraphHydrologyRole::Trunk,
                raw_flow_accumulation: flow_accumulation,
                flow_accumulation,
                downstream_progress: id as f32,
            }
        }

        fn test_noisy_curve(
            edge: u64,
            start_corner: u64,
            end_corner: u64,
            start: WorldPlanePoint,
            end: WorldPlanePoint,
        ) -> NoisyBoundaryCurve {
            use crate::world::generation::boundary::{
                BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
            };

            NoisyBoundaryCurve {
                edge: VoronoiEdgeId(edge),
                profile: BoundaryProfile::Ordinary,
                anchors: BoundaryAnchors {
                    corners: [VoronoiCornerId(start_corner), VoronoiCornerId(end_corner)],
                    sites: [VoronoiSiteId(edge * 2), VoronoiSiteId(edge * 2 + 1)],
                    start,
                    end,
                },
                points: vec![start, end],
                amplitude: 0.0,
                seed: edge,
                guard: BoundaryGuard {
                    min_x: start.x.min(end.x) - 16.0,
                    max_x: start.x.max(end.x) + 16.0,
                    min_z: start.z.min(end.z) - 16.0,
                    max_z: start.z.max(end.z) + 16.0,
                },
            }
        }
    }

    #[test]
    fn river_chain_flow_smoothing_reduces_edge_level_jumps() {
        let raw = vec![0.10, 0.95, 0.12, 0.90, 0.30];
        let smoothed = smooth_river_flow_hints(&raw);

        let raw_max_jump = raw
            .windows(2)
            .map(|pair| (pair[0] - pair[1]).abs())
            .fold(0.0, f32::max);
        let smoothed_max_jump = smoothed
            .windows(2)
            .map(|pair| (pair[0] - pair[1]).abs())
            .fold(0.0, f32::max);

        assert!(
            smoothed_max_jump < raw_max_jump,
            "chain-direction flow smoothing should reduce edge-to-edge display width jumps"
        );
        assert!(
            smoothed.iter().all(|flow| (0.0..=1.0).contains(flow)),
            "display flow hints must stay normalized"
        );
    }

    #[test]
    fn river_chain_smoothed_source_keeps_join_valley_continuous() {
        let source = smooth_river_chain_source(
            vec![
                WorldPlanePoint::new(0.0, 16.0),
                WorldPlanePoint::new(48.0, 16.0),
                WorldPlanePoint::new(48.0, 80.0),
            ],
            vec![0.25, 0.85, 0.45],
        )
        .expect("test chain should be renderable");
        let config = MacroFieldTileConfig::new(0.0, 0.0, 7, 7, 16.0);
        let field = rasterize_river_chain_anti_aliased_polyline_field(&[source], config, 48.0);
        let joint = 1 * 7 + 3;
        let before_joint = 1 * 7 + 2;
        let after_joint = 2 * 7 + 3;

        assert!(
            field.river_valley_strength[joint]
                >= field.river_valley_strength[before_joint]
                    .min(field.river_valley_strength[after_joint])
                    * 0.85,
            "chain-smoothed AA stroke should not reopen a pointed gap at a Voronoi edge join"
        );
    }

    #[test]
    fn river_width_and_depth_increase_with_flow() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let headwater_flow = flow_hint(12.0);
        let trunk_flow = flow_hint(1024.0);

        assert!(
            river_width_blocks(headwater_flow, radius) < river_width_blocks(trunk_flow, radius),
            "river corridor width should grow with selected/display flow"
        );
        assert!(
            river_depth_factor(headwater_flow) < river_depth_factor(trunk_flow),
            "river carve depth should grow with selected/display flow"
        );
        assert!(
            river_width_blocks(headwater_flow, radius) < radius * 0.35,
            "headwater rivers should be much narrower than the maximum downstream radius"
        );
    }

    #[test]
    fn river_valley_uses_flow_scaled_width() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let distance = radius * 0.45;
        let headwater = river_valley_strength_for_distance(distance, flow_hint(12.0), radius);
        let trunk = river_valley_strength_for_distance(distance, flow_hint(1024.0), radius);

        assert!(
            trunk > headwater,
            "a downstream trunk should still carve at distances where a headwater has faded: headwater={headwater} trunk={trunk}"
        );
        assert!(
            headwater <= 0.02,
            "headwater carve should fade quickly instead of using a fixed wide corridor: {headwater}"
        );
    }

    #[test]
    fn river_valley_has_flat_bed_before_shoulder_falloff() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let trunk = flow_hint(1024.0);
        let flat = river_flat_bed_radius_blocks(trunk, radius);
        let center = river_valley_strength_for_distance(0.0, trunk, radius);
        let inside_flat = river_valley_strength_for_distance(flat * 0.85, trunk, radius);
        let shoulder = river_valley_strength_for_distance(flat + 24.0, trunk, radius);

        assert!(
            flat > 24.0,
            "downstream river should expose a wide flat bed"
        );
        assert!(
            (center - inside_flat).abs() <= 0.001,
            "river bottom should stay flat across the bed: center={center} inside={inside_flat}"
        );
        assert!(
            shoulder < center && shoulder > 0.0,
            "river shoulder should fall off after the flat bed without an immediate cliff: center={center} shoulder={shoulder}"
        );
    }

    #[test]
    fn downstream_flow_widens_flat_bed_and_stays_depth_capped() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let headwater = flow_hint(12.0);
        let trunk = flow_hint(1024.0);

        assert!(
            river_flat_bed_radius_blocks(trunk, radius)
                > river_flat_bed_radius_blocks(headwater, radius)
        );
        assert!(
            river_depth_factor(trunk) <= RIVER_TRUNK_DEPTH_FACTOR + f32::EPSILON,
            "downstream carve depth should be capped instead of becoming a deep V"
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
    fn macro_field_owner_sampling_follows_noisy_boundary_curve() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::hydrology::GraphHydrologyGraph;
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let left = test_site(
            VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.8,
        );
        let right = test_site(
            VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::OceanBasin,
            -0.8,
        );
        let edge = VoronoiEdgeId(7);
        let start = WorldPlanePoint::new(0.0, -10.0);
        let end = WorldPlanePoint::new(0.0, 10.0);
        let curve = NoisyBoundaryCurve {
            edge,
            profile: BoundaryProfile::Coast,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                sites: [left.id, right.id],
                start,
                end,
            },
            points: vec![start, WorldPlanePoint::new(5.0, 0.0), end],
            amplitude: 5.0,
            seed: 1,
            guard: BoundaryGuard {
                min_x: -20.0,
                max_x: 20.0,
                min_z: -20.0,
                max_z: 20.0,
            },
        };
        let macro_map = GraphMacroMap {
            sites: vec![left, right],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [left.id, right.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: true,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 1.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 1.6,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![curve],
            stats: Default::default(),
        };
        let patch = Default::default();
        let hydrology = GraphHydrologyGraph::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &hydrology, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 24.0;

        let sample = sample_macro_field_point(&context, config, WorldPlanePoint::new(4.0, 0.0));

        assert_eq!(
            sample.nearest_site,
            Some(left.id),
            "point is closer to the right site in straight Voronoi space, but the bowed noisy curve should classify it on the left side"
        );
        assert_eq!(sample.surface_kind, Some(MacroSurfaceKind::Continent));
        assert!(sample.coast_mask > 0.5);
    }

    #[test]
    fn coast_boundary_profile_starts_at_sea_level_and_recovers_on_land_side() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::hydrology::GraphHydrologyGraph;
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let land = test_site(
            VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.8,
        );
        let ocean = test_site(
            VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::OceanBasin,
            -0.8,
        );
        let edge = VoronoiEdgeId(71);
        let start = WorldPlanePoint::new(0.0, -10.0);
        let end = WorldPlanePoint::new(0.0, 10.0);
        let curve = NoisyBoundaryCurve {
            edge,
            profile: BoundaryProfile::Coast,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                sites: [land.id, ocean.id],
                start,
                end,
            },
            points: vec![start, end],
            amplitude: 0.0,
            seed: 1,
            guard: BoundaryGuard {
                min_x: -48.0,
                max_x: 48.0,
                min_z: -20.0,
                max_z: 20.0,
            },
        };
        let macro_map = GraphMacroMap {
            sites: vec![land, ocean],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [land.id, ocean.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: true,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 1.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 1.6,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![curve],
            stats: Default::default(),
        };
        let patch = Default::default();
        let hydrology = GraphHydrologyGraph::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &hydrology, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 24.0;

        let shoreline_land =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(-1.0, 0.0));
        let recovered_land =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(-32.0, 0.0));
        let shoreline_ocean =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(1.0, 0.0));

        assert!(
            shoreline_land.macro_elevation <= 0.02,
            "land side of coast curve should begin at sea level, got {}",
            shoreline_land.macro_elevation
        );
        assert!(
            recovered_land.macro_elevation >= 0.75,
            "land owner elevation should recover outside the shoreline blend, got {}",
            recovered_land.macro_elevation
        );
        assert!(
            shoreline_ocean.macro_elevation < 0.0,
            "ocean side should remain an underwater profile, got {}",
            shoreline_ocean.macro_elevation
        );
    }

    #[test]
    fn dry_basin_boundary_blend_does_not_create_coast_mask() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::hydrology::GraphHydrologyGraph;
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let dry = test_site(
            VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::DryBasin,
            0.08,
        );
        let land = test_site(
            VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.22,
        );
        let edge = VoronoiEdgeId(17);
        let start = WorldPlanePoint::new(0.0, -10.0);
        let end = WorldPlanePoint::new(0.0, 10.0);
        let curve = NoisyBoundaryCurve {
            edge,
            profile: BoundaryProfile::Ordinary,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                sites: [dry.id, land.id],
                start,
                end,
            },
            points: vec![start, end],
            amplitude: 0.0,
            seed: 1,
            guard: BoundaryGuard {
                min_x: -20.0,
                max_x: 20.0,
                min_z: -20.0,
                max_z: 20.0,
            },
        };
        let macro_map = GraphMacroMap {
            sites: vec![dry, land],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [dry.id, land.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: false,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 0.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 0.14,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![curve],
            stats: Default::default(),
        };
        let patch = Default::default();
        let hydrology = GraphHydrologyGraph::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &hydrology, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 24.0;

        let sample = sample_macro_field_point(&context, config, WorldPlanePoint::new(0.0, 0.0));

        assert!(
            sample.coast_mask <= f32::EPSILON,
            "dry basin / land noisy boundary blend must not render as coast: {}",
            sample.coast_mask
        );
    }

    #[test]
    fn macro_field_sample_carries_nearest_site_biome() {
        let inputs = test_inputs(42);
        let context = MacroFieldRasterContext::new(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.hydrology,
            &inputs.boundary,
        );
        let site = inputs
            .macro_map
            .sites
            .iter()
            .find(|site| inputs.macro_map.biome(site.id).is_some())
            .expect("generated macro map should expose biome cells");
        let sample = sample_macro_field_point(&context, test_tile_config(), site.position);
        let expected = inputs
            .macro_map
            .biome(site.id)
            .expect("site biome should be stable");

        assert_eq!(sample.nearest_site, Some(site.id));
        assert_eq!(sample.biome, Some(expected.biome));
        assert_eq!(sample.biome_context, Some(expected.context));
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
            near_sample.combined_macro_height < far_sample.combined_macro_height,
            "river carve should be visible in combined macro height: near={} far={}",
            near_sample.combined_macro_height,
            far_sample.combined_macro_height
        );
    }

    #[test]
    fn contour_extraction_is_deterministic_and_finite() {
        let tile = test_contour_tile(&[0.0, 16.0, 8.0, 24.0], 2, 2);

        let first = extract_macro_field_contours(&tile, 8.0, 5);
        let second = extract_macro_field_contours(&tile, 8.0, 5);

        assert_eq!(first, second);
        assert!(first.total_segment_count > 0);
        assert!(first.levels.iter().all(|level| {
            level.height_blocks.is_finite()
                && level.segments.iter().all(|segment| {
                    segment.start.x.is_finite()
                        && segment.start.z.is_finite()
                        && segment.end.x.is_finite()
                        && segment.end.z.is_finite()
                })
        }));
    }

    #[test]
    fn contour_block_scale_keeps_signed_zero_at_sea_level() {
        assert_eq!(combined_macro_height_to_blocks(0.0), 0.0);
        assert_eq!(
            combined_macro_height_to_blocks(MACRO_FIELD_CONTOUR_NORMALIZED_MIN),
            MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS
        );
        assert_eq!(
            combined_macro_height_to_blocks(MACRO_FIELD_CONTOUR_NORMALIZED_MAX),
            MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS
        );
    }

    #[test]
    fn contour_block_scale_uses_experimental_large_block_domain() {
        assert_eq!(MACRO_FIELD_CONTOUR_NORMALIZED_MIN, -0.5);
        assert_eq!(MACRO_FIELD_CONTOUR_NORMALIZED_MAX, 1.0);
        assert_eq!(MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS, -1024.0);
        assert_eq!(MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS, 2048.0);
        assert_eq!(combined_macro_height_to_blocks(-0.25), -512.0);
        assert_eq!(combined_macro_height_to_blocks(0.75), 1536.0);
    }

    #[test]
    fn contour_block_scale_saturates_outside_effective_range() {
        assert_eq!(combined_macro_height_to_blocks(-0.75), -1024.0);
        assert_eq!(combined_macro_height_to_blocks(1.25), 2048.0);
    }

    #[test]
    fn default_contour_preview_step_is_practical_for_large_block_domain() {
        assert_eq!(DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS, 32.0);
    }

    #[test]
    fn simple_ramp_field_produces_contour_crossing() {
        let tile = test_contour_tile(&[0.0, 128.0, 0.0, 128.0], 2, 2);

        let contours =
            extract_macro_field_contours(&tile, DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS, 5);

        let level = contours
            .levels
            .iter()
            .find(|level| {
                (level.height_blocks - DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS).abs()
                    <= f32::EPSILON
            })
            .expect("ramp should produce a default-step contour");
        assert_eq!(level.segments.len(), 1);
        assert!(
            (level.segments[0].start.x - 16.0).abs() <= 0.01
                || (level.segments[0].end.x - 16.0).abs() <= 0.01,
            "default 32-block contour should cross one quarter across a 64-block sample cell: {:?}",
            level.segments[0]
        );
    }

    #[test]
    fn flat_field_produces_no_contours() {
        let tile = test_contour_tile(&[12.0, 12.0, 12.0, 12.0], 2, 2);

        let contours = extract_macro_field_contours(&tile, 8.0, 5);

        assert_eq!(contours.total_segment_count, 0);
        assert!(contours.levels.is_empty());
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

    fn centered_test_tile_config(center: WorldPlanePoint) -> MacroFieldTileConfig {
        MacroFieldTileConfig::new(center.x - 512.0, center.z - 512.0, 24, 24, 64.0)
    }

    fn test_contour_tile(heights_blocks: &[f32], width: u32, height: u32) -> MacroFieldTile {
        let config = MacroFieldTileConfig::new(0.0, 0.0, width, height, 64.0);
        let samples = heights_blocks
            .iter()
            .enumerate()
            .map(|(index, height_blocks)| {
                let combined_macro_height = blocks_to_combined_macro_height(*height_blocks);
                MacroFieldSample {
                    position: config.sample_position(index),
                    nearest_site: None,
                    surface_kind: None,
                    biome_context: None,
                    biome: None,
                    macro_elevation: combined_macro_height,
                    ocean_mask: 0.0,
                    coast_mask: 0.0,
                    lake_mask: 0.0,
                    dry_basin_mask: 0.0,
                    ridge_influence: 0.0,
                    river_valley_strength: 0.0,
                    river_distance_blocks: f32::INFINITY,
                    river_flow_hint: 0.0,
                    combined_macro_height,
                }
            })
            .collect::<Vec<_>>();
        MacroFieldTile {
            config,
            stats: macro_field_stats(&samples, MacroFieldInfluenceStats::default()),
            samples,
        }
    }

    fn blocks_to_combined_macro_height(height_blocks: f32) -> f32 {
        if height_blocks >= 0.0 {
            let t = (height_blocks / MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS.max(f32::EPSILON))
                .clamp(0.0, 1.0);
            t * MACRO_FIELD_CONTOUR_NORMALIZED_MAX
        } else {
            let t = (height_blocks / MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS.min(-f32::EPSILON))
                .clamp(0.0, 1.0);
            t * MACRO_FIELD_CONTOUR_NORMALIZED_MIN
        }
    }

    fn test_site(
        id: super::super::graph::VoronoiSiteId,
        x: f32,
        z: f32,
        surface_kind: MacroSurfaceKind,
        signed_macro_elevation: f32,
    ) -> MacroSite {
        MacroSite {
            id,
            owner_region: super::super::graph::GraphRegionCoord { x: 0, z: 0 },
            position: WorldPlanePoint::new(x, z),
            surface_kind,
            continent: None,
            ocean_basin: None,
            signed_macro_elevation,
            continentality: signed_macro_elevation,
            coastness: 0.0,
            distance_to_coast_blocks: 0.0,
            distance_to_continent_core_blocks: 0.0,
            distance_to_ocean_basin_blocks: 0.0,
            mountainness: 0.0,
            ridgeness: 0.0,
            basinness: 0.0,
        }
    }
}
