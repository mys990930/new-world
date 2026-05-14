use rayon::prelude::*;
use std::collections::{HashMap, VecDeque};

use super::biome::{GraphBiomeCell, GraphBiomeContext, GraphBiomeKind};
use super::boundary::{BoundaryCache, NoisyBoundaryCurve};
use super::graph::{VoronoiEdgeId, VoronoiGraphPatch, VoronoiSiteId, WorldPlanePoint};
use super::macro_map::{GraphMacroMap, MacroSite, MacroSurfaceKind};
use super::river_plan::{RiverPlan, RiverSegmentPlan};

const MACRO_FIELD_CURVE_BUCKET_BLOCKS: f32 = 64.0;
const MACRO_FIELD_SITE_BUCKET_BLOCKS: f32 = 256.0;
const MACRO_FIELD_SCALAR_INTERPOLATION_RADIUS_BLOCKS: f32 = 768.0;
const MACRO_FIELD_SCALAR_INTERPOLATION_BUCKET_RADIUS: i32 = 4;
const MACRO_FIELD_SCALAR_INTERPOLATION_DISTANCE_POWER: f32 = 1.45;
pub const DEFAULT_MACRO_FIELD_SAMPLE_SPACING_BLOCKS: f32 = 32.0;
pub const DEFAULT_MACRO_FIELD_RIDGE_RADIUS_BLOCKS: f32 = 256.0;
pub const DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS: f32 = 240.0;
pub const DEFAULT_MACRO_FIELD_COAST_RADIUS_BLOCKS: f32 = 384.0;
pub const DEFAULT_MACRO_FIELD_RIDGE_HEIGHT_SCALE: f32 = 0.0;
pub const DEFAULT_MACRO_FIELD_RIVER_CARVE_SCALE: f32 = 0.018;
pub const DEFAULT_MACRO_FIELD_LAKE_FLATTEN_STRENGTH: f32 = 0.96;
pub const DEFAULT_MACRO_FIELD_BOUNDARY_BLEND_RADIUS_BLOCKS: f32 = 96.0;
pub const DEFAULT_MACRO_FIELD_BOUNDARY_ROUGHNESS_BLOCKS: f32 = 96.0;
pub const MACRO_FIELD_CONTOUR_NORMALIZED_MIN: f32 = -0.5;
pub const MACRO_FIELD_CONTOUR_NORMALIZED_MAX: f32 = 1.0;
pub const MACRO_FIELD_CONTOUR_HEIGHT_MIN_BLOCKS: f32 = -1024.0;
pub const MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS: f32 = 2048.0;
pub const DEFAULT_MACRO_FIELD_CONTOUR_STEP_BLOCKS: f32 = 32.0;
pub const DEFAULT_MACRO_FIELD_CONTOUR_MAJOR_EVERY: u32 = 5;

const RIDGE_INFLUENCE_VISIBLE_FLOOR: f32 = 0.12;
const RIDGE_FIELD_SOURCE_MIN_RIDGENESS: f32 = 0.44;
const ISOLATED_OCEAN_FRAGMENT_MAX_BLOCK_AREA: f32 = 512.0;

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
    pub boundary_roughness_blocks: f32,
    pub ridge_height_scale: f32,
    pub river_carve_scale: f32,
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
            boundary_roughness_blocks: DEFAULT_MACRO_FIELD_BOUNDARY_ROUGHNESS_BLOCKS,
            ridge_height_scale: DEFAULT_MACRO_FIELD_RIDGE_HEIGHT_SCALE,
            river_carve_scale: DEFAULT_MACRO_FIELD_RIVER_CARVE_SCALE,
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
    pub river_bed_depth_hint: f32,
    pub river_bank_roughness_hint: f32,
    pub river_gravel_hint: f32,
    pub river_cutbank_hint: f32,
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
    river_plan: &RiverPlan,
    boundary: &BoundaryCache,
    config: MacroFieldTileConfig,
) -> MacroFieldTile {
    validate_macro_field_config(config);
    assert_eq!(
        boundary.stats.missing_macro_edge_count, 0,
        "macro field requires complete canonical boundary coverage"
    );

    let context = MacroFieldRasterContext::new(patch, macro_map, river_plan, boundary);
    let influence_fields = rasterize_influence_fields(&context, config);
    let mut samples = (0..config.sample_count())
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
    prune_isolated_ocean_fragments(&mut samples, config);
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
        .map(|distance| {
            envelope(
                roughened_distance(
                    distance,
                    position,
                    config.boundary_roughness_blocks,
                    0xC0A5_7001,
                ),
                config.coast_radius_blocks,
            )
        })
        .unwrap_or(site_coastness)
        .max(site_coastness)
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
    let river_morphology = context.river_valley(position, config);
    let river_distance_blocks = river_morphology.distance_blocks;
    let river_flow_hint = river_morphology.flow_hint;
    let river_valley_strength = river_morphology.valley_strength;
    let combined_macro_height = combine_macro_height(
        macro_elevation,
        ocean_mask,
        coast_mask,
        lake_mask,
        dry_basin_mask,
        owner_sample.lake_lowering_factor,
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
        river_bed_depth_hint: river_morphology.bed_depth_hint,
        river_bank_roughness_hint: river_morphology.bank_roughness_hint,
        river_gravel_hint: river_morphology.gravel_hint,
        river_cutbank_hint: river_morphology.cutbank_hint,
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
        owner_sample.lake_lowering_factor,
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
        river_bed_depth_hint: influence.river_bed_depth_hint,
        river_bank_roughness_hint: influence.river_bank_roughness_hint,
        river_gravel_hint: influence.river_gravel_hint,
        river_cutbank_hint: influence.river_cutbank_hint,
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
    river_bed_depth_hint: f32,
    river_bank_roughness_hint: f32,
    river_gravel_hint: f32,
    river_cutbank_hint: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct MacroFieldInfluenceFields {
    ridge_distance_blocks: Vec<f32>,
    coast_distance_blocks: Vec<f32>,
    river_distance_blocks: Vec<f32>,
    river_valley_strength: Vec<f32>,
    river_flow_hint: Vec<f32>,
    river_bed_depth_hint: Vec<f32>,
    river_bank_roughness_hint: Vec<f32>,
    river_gravel_hint: Vec<f32>,
    river_cutbank_hint: Vec<f32>,
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
            coast_influence: envelope(
                roughened_distance(
                    coast_distance,
                    config.sample_position(index),
                    config.boundary_roughness_blocks,
                    0xC0A5_7001,
                ),
                config.coast_radius_blocks,
            ),
            river_valley_strength: river_valley_strength.clamp(0.0, 1.0),
            river_distance_blocks: river_distance,
            river_flow_hint,
            river_bed_depth_hint: self.river_bed_depth_hint[index].clamp(0.0, 1.0),
            river_bank_roughness_hint: self.river_bank_roughness_hint[index].clamp(0.0, 1.0),
            river_gravel_hint: self.river_gravel_hint[index].clamp(0.0, 1.0),
            river_cutbank_hint: self.river_cutbank_hint[index].clamp(0.0, 1.0),
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
    let river_sources = context
        .river_curves
        .iter()
        .map(|river| RiverRasterSource {
            edge: river.edge,
            points: &river.points,
            flow_hint: river.flow_hint,
            valley_width_blocks: river.valley_width_blocks,
            bed_depth_blocks: river.bed_depth_blocks,
            component_id: 0,
        })
        .collect::<Vec<_>>();

    let ridge = rasterize_curve_distance_field(&ridge_sources, config, config.ridge_radius_blocks);
    let coast = rasterize_curve_distance_field(&coast_sources, config, config.coast_radius_blocks);
    let river = rasterize_curve_anti_aliased_polyline_field(
        &river_sources,
        config,
        config.river_radius_blocks,
    );
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
        river_valley_strength: river.river_valley_strength,
        river_flow_hint: river.flow_hint,
        river_bed_depth_hint: river.river_bed_depth_hint,
        river_bank_roughness_hint: river.river_bank_roughness_hint,
        river_gravel_hint: river.river_gravel_hint,
        river_cutbank_hint: river.river_cutbank_hint,
        stats,
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RasterDistanceField {
    distance_blocks: Vec<f32>,
    river_valley_strength: Vec<f32>,
    flow_hint: Vec<f32>,
    river_bed_depth_hint: Vec<f32>,
    river_bank_roughness_hint: Vec<f32>,
    river_gravel_hint: Vec<f32>,
    river_cutbank_hint: Vec<f32>,
    source_pixel_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RiverRasterSource<'a> {
    edge: VoronoiEdgeId,
    points: &'a [WorldPlanePoint],
    flow_hint: f32,
    valley_width_blocks: f32,
    bed_depth_blocks: f32,
    component_id: usize,
}

fn assign_river_raster_components(sources: &mut [RiverRasterSource<'_>]) {
    let mut parent = (0..sources.len()).collect::<Vec<_>>();
    for left in 0..sources.len() {
        for right in left + 1..sources.len() {
            if river_sources_touch(sources[left], sources[right]) {
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

fn river_sources_touch(left: RiverRasterSource<'_>, right: RiverRasterSource<'_>) -> bool {
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

fn union_component(parent: &mut [usize], left: usize, right: usize) {
    let left_root = find_component(parent, left);
    let right_root = find_component(parent, right);
    if left_root != right_root {
        parent[right_root] = left_root;
    }
}

fn find_component(parent: &mut [usize], index: usize) -> usize {
    if parent[index] != index {
        let parent_index = parent[index];
        parent[index] = find_component(parent, parent_index);
    }
    parent[index]
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
        river_valley_strength: vec![0.0; sample_count],
        flow_hint: cropped_flow,
        river_bed_depth_hint: vec![0.0; sample_count],
        river_bank_roughness_hint: vec![0.0; sample_count],
        river_gravel_hint: vec![0.0; sample_count],
        river_cutbank_hint: vec![0.0; sample_count],
        source_pixel_count,
    }
}

fn rasterize_curve_anti_aliased_polyline_field(
    sources: &[RiverRasterSource<'_>],
    config: MacroFieldTileConfig,
    radius_blocks: f32,
) -> RasterDistanceField {
    let sample_count = config.sample_count();
    if sources.is_empty() {
        return RasterDistanceField {
            distance_blocks: vec![f32::INFINITY; sample_count],
            river_valley_strength: vec![0.0; sample_count],
            flow_hint: vec![0.0; sample_count],
            river_bed_depth_hint: vec![0.0; sample_count],
            river_bank_roughness_hint: vec![0.0; sample_count],
            river_gravel_hint: vec![0.0; sample_count],
            river_cutbank_hint: vec![0.0; sample_count],
            source_pixel_count: 0,
        };
    }

    let mut sources = sources.to_vec();
    assign_river_raster_components(&mut sources);

    let width = config.width as usize;
    let height = config.height as usize;
    let mut distance_blocks = vec![f32::INFINITY; sample_count];
    let mut river_valley_strength = vec![0.0; sample_count];
    let mut river_owner_component = vec![usize::MAX; sample_count];
    let mut flow_weighted_sum = vec![0.0; sample_count];
    let mut flow_weight_sum = vec![0.0; sample_count];
    let mut river_bed_depth_hint = vec![0.0; sample_count];
    let mut river_bank_roughness_hint = vec![0.0; sample_count];
    let mut river_gravel_hint = vec![0.0; sample_count];
    let mut river_cutbank_hint = vec![0.0; sample_count];

    for source in &sources {
        for segment in source.points.windows(2) {
            rasterize_segment_anti_aliased_stroke(
                &mut distance_blocks,
                &mut river_valley_strength,
                &mut river_owner_component,
                &mut flow_weighted_sum,
                &mut flow_weight_sum,
                &mut river_bed_depth_hint,
                &mut river_bank_roughness_hint,
                &mut river_gravel_hint,
                &mut river_cutbank_hint,
                width,
                height,
                config,
                segment[0],
                segment[1],
                radius_blocks,
                *source,
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
        river_bed_depth_hint,
        river_bank_roughness_hint,
        river_gravel_hint,
        river_cutbank_hint,
        source_pixel_count,
    }
}

#[allow(clippy::too_many_arguments)]
fn rasterize_segment_anti_aliased_stroke(
    distance_blocks: &mut [f32],
    river_valley_strength: &mut [f32],
    river_owner_component: &mut [usize],
    flow_weighted_sum: &mut [f32],
    flow_weight_sum: &mut [f32],
    river_bed_depth_hint: &mut [f32],
    river_bank_roughness_hint: &mut [f32],
    river_gravel_hint: &mut [f32],
    river_cutbank_hint: &mut [f32],
    width: usize,
    height: usize,
    config: MacroFieldTileConfig,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    radius_blocks: f32,
    source: RiverRasterSource<'_>,
) {
    let strength = source.flow_hint;
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
            let global_index = z * width + x;
            let position = config.sample_position(global_index);
            let distance = point_segment_distance(position, start, end);
            if distance > radius_blocks + aa_margin {
                continue;
            }
            let mut profile_sum = 0.0;
            let mut closest_subpixel_distance = distance;
            let mut bed_sum = 0.0;
            let mut rough_sum = 0.0;
            let mut gravel_sum = 0.0;
            let mut cutbank_sum = 0.0;
            for (offset_x, offset_z) in subpixel_offsets {
                let subpixel = WorldPlanePoint::new(
                    position.x + offset_x * spacing,
                    position.z + offset_z * spacing,
                );
                let subpixel_distance = point_segment_distance(subpixel, start, end);
                let valley_strength = river_valley_strength_for_distance(
                    subpixel_distance,
                    source.flow_hint,
                    source.valley_width_blocks,
                    radius_blocks,
                );
                let hints = river_hints_from_strength(
                    valley_strength,
                    source.flow_hint,
                    source.bed_depth_blocks,
                );
                closest_subpixel_distance = closest_subpixel_distance.min(subpixel_distance);
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

            let bed_hint = (bed_sum / subpixel_count).clamp(0.0, 1.0);
            let rough_hint = (rough_sum / subpixel_count).clamp(0.0, 1.0);
            let gravel_hint = (gravel_sum / subpixel_count).clamp(0.0, 1.0);
            let cutbank_hint = (cutbank_sum / subpixel_count).clamp(0.0, 1.0);
            let current_distance = distance_blocks[global_index];
            let same_thalweg_band = spacing * 0.35;
            let current_component = river_owner_component[global_index];
            if current_component == source.component_id && current_distance.is_finite() {
                distance_blocks[global_index] =
                    distance_blocks[global_index].min(closest_subpixel_distance);
                river_valley_strength[global_index] =
                    soft_union_strength(river_valley_strength[global_index], anti_aliased_strength);
                river_bed_depth_hint[global_index] =
                    river_bed_depth_hint[global_index].max(bed_hint);
                river_bank_roughness_hint[global_index] =
                    river_bank_roughness_hint[global_index].max(rough_hint);
                river_gravel_hint[global_index] = river_gravel_hint[global_index].max(gravel_hint);
                river_cutbank_hint[global_index] =
                    river_cutbank_hint[global_index].max(cutbank_hint);
                flow_weighted_sum[global_index] += strength * anti_aliased_strength;
                flow_weight_sum[global_index] += anti_aliased_strength;
            } else if closest_subpixel_distance + same_thalweg_band < current_distance {
                distance_blocks[global_index] = closest_subpixel_distance;
                river_valley_strength[global_index] = anti_aliased_strength;
                river_owner_component[global_index] = source.component_id;
                river_bed_depth_hint[global_index] = bed_hint;
                river_bank_roughness_hint[global_index] = rough_hint;
                river_gravel_hint[global_index] = gravel_hint;
                river_cutbank_hint[global_index] = cutbank_hint;
                flow_weighted_sum[global_index] = strength * anti_aliased_strength;
                flow_weight_sum[global_index] = anti_aliased_strength;
            }
        }
    }
}

fn soft_union_strength(existing: f32, incoming: f32) -> f32 {
    let existing = existing.clamp(0.0, 1.0);
    let incoming = incoming.clamp(0.0, 1.0);
    (1.0 - (1.0 - existing) * (1.0 - incoming)).clamp(0.0, 1.0)
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
    river_curves: Vec<RiverCurveRef>,
    river_grid: CurveIndexGrid,
}

impl<'a> MacroFieldRasterContext<'a> {
    pub fn new(
        _patch: &'a VoronoiGraphPatch,
        macro_map: &'a GraphMacroMap,
        river_plan: &'a RiverPlan,
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
        let mut river_curves = river_plan
            .segments
            .iter()
            .filter_map(|plan| {
                boundary_curves
                    .get(&plan.edge)
                    .copied()
                    .map(|curve| RiverCurveRef {
                        edge: plan.edge,
                        points: curve.points.clone(),
                        flow_hint: flow_hint_from_plan(plan),
                        valley_width_blocks: plan.broad_valley_width_blocks,
                        bed_depth_blocks: plan.bed_depth_blocks,
                    })
            })
            .collect::<Vec<_>>();
        river_curves.sort_by_key(|river| river.edge.0);
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
        let river_grid = CurveIndexGrid::from_river_curves(&river_curves);

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
            river_curves,
            river_grid,
        }
    }

    pub fn nearest_site(&self, position: WorldPlanePoint) -> Option<&'a MacroSite> {
        let mut nearest = None;
        self.site_grid
            .for_each_nearest_candidate_index(position, |index| {
                let Some(site) = self.sites.get(index) else {
                    return;
                };
                if nearest_site_order(position, site, nearest).is_lt() {
                    nearest = Some(site);
                }
            });
        if nearest.is_some() {
            return nearest;
        }

        self.sites.iter().min_by(|left, right| {
            squared_distance(position, left.position)
                .total_cmp(&squared_distance(position, right.position))
                .then_with(|| left.id.0.cmp(&right.id.0))
        })
    }

    pub fn biome_for_site(&self, site: VoronoiSiteId) -> Option<GraphBiomeCell> {
        self.biome_by_site_id.get(&site).copied()
    }

    fn owner_sample(&self, position: WorldPlanePoint, config: MacroFieldTileConfig) -> OwnerSample {
        if let Some(boundary) = self.nearest_boundary(position, config.boundary_blend_radius_blocks)
        {
            if boundary.distance <= config.boundary_blend_radius_blocks {
                let primary_site = boundary.primary_site();
                let secondary = boundary.secondary_site();
                let is_lake_pair = is_lake_surface(primary_site.surface_kind)
                    != is_lake_surface(secondary.surface_kind);
                let primary = self
                    .site_by_id
                    .get(&primary_site.id)
                    .copied()
                    .or_else(|| self.nearest_site(position).copied());

                return OwnerSample {
                    primary,
                    macro_elevation: self.interpolated_macro_elevation(position),
                    lake_lowering_factor: if is_lake_pair {
                        lake_boundary_lowering_factor(
                            primary_site,
                            boundary.distance,
                            config.boundary_blend_radius_blocks,
                            boundary_roughness_offset(
                                position,
                                config.boundary_roughness_blocks,
                                0x1A4E_0001,
                            ),
                        )
                    } else if is_lake_surface(primary_site.surface_kind) {
                        1.0
                    } else {
                        0.0
                    },
                };
            }
        }

        self.owner_sample_from_site(position, self.nearest_site(position))
    }

    fn owner_sample_from_site(
        &self,
        position: WorldPlanePoint,
        site: Option<&'a MacroSite>,
    ) -> OwnerSample {
        let Some(site) = site.copied() else {
            return OwnerSample::default();
        };
        OwnerSample {
            primary: Some(site),
            macro_elevation: self.interpolated_macro_elevation(position),
            lake_lowering_factor: if is_lake_surface(site.surface_kind) {
                1.0
            } else {
                0.0
            },
        }
    }

    fn interpolated_macro_elevation(&self, position: WorldPlanePoint) -> f32 {
        let mut exact_source = None;
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;
        self.site_grid.for_each_candidate_index_in_radius(
            position,
            MACRO_FIELD_SCALAR_INTERPOLATION_BUCKET_RADIUS,
            |index| {
                if let Some(site) = self.sites.get(index) {
                    let distance2 = squared_distance(position, site.position);
                    if distance2 <= 0.0001 {
                        exact_source = Some(site.signed_macro_elevation);
                    } else if exact_source.is_none() {
                        let distance = distance2.sqrt();
                        if distance <= MACRO_FIELD_SCALAR_INTERPOLATION_RADIUS_BLOCKS {
                            let radius_t = (distance
                                / MACRO_FIELD_SCALAR_INTERPOLATION_RADIUS_BLOCKS)
                                .clamp(0.0, 1.0);
                            let falloff = 1.0 - smoothstep01(radius_t);
                            let weight = falloff
                                / distance
                                    .max(1.0)
                                    .powf(MACRO_FIELD_SCALAR_INTERPOLATION_DISTANCE_POWER);
                            weighted_sum += site.signed_macro_elevation * weight;
                            weight_sum += weight;
                        }
                    }
                }
            },
        );

        if let Some(elevation) = exact_source {
            return elevation;
        }

        if weight_sum <= f32::EPSILON {
            self.nearest_site(position)
                .map(|site| site.signed_macro_elevation)
                .unwrap_or_default()
        } else {
            weighted_sum / weight_sum
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
    ) -> RiverMorphologySample {
        let Some((sample, _edge)) = self
            .river_grid
            .candidate_indices(position, config.river_radius_blocks)
            .into_iter()
            .filter_map(|index| self.river_curves.get(index))
            .map(|river| {
                Some((
                    river_morphology_sample(
                        position,
                        &river.points,
                        river.flow_hint,
                        river.valley_width_blocks,
                        river.bed_depth_blocks,
                        config.river_radius_blocks,
                    ),
                    river.edge,
                ))
            })
            .flatten()
            .min_by(|left, right| {
                left.0
                    .distance_blocks
                    .total_cmp(&right.0.distance_blocks)
                    .then_with(|| left.1.0.cmp(&right.1.0))
            })
        else {
            return RiverMorphologySample {
                distance_blocks: f32::INFINITY,
                ..RiverMorphologySample::default()
            };
        };

        sample
    }
}

#[derive(Debug, Clone)]
struct RiverCurveRef {
    edge: VoronoiEdgeId,
    points: Vec<WorldPlanePoint>,
    flow_hint: f32,
    valley_width_blocks: f32,
    bed_depth_blocks: f32,
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
    lake_lowering_factor: f32,
}

impl Default for OwnerSample {
    fn default() -> Self {
        Self {
            primary: None,
            macro_elevation: 0.0,
            lake_lowering_factor: 0.0,
        }
    }
}

fn lake_boundary_lowering_factor(
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
        grid.dedup_bucket_entries();
        grid
    }

    fn from_boundary_edges(edges: &[BoundaryEdgeRef<'_>]) -> Self {
        let mut grid = Self::default();
        for (index, edge) in edges.iter().enumerate() {
            grid.insert_curve(index, &edge.curve.points);
        }
        grid.dedup_bucket_entries();
        grid
    }

    fn from_river_curves(curves: &[RiverCurveRef]) -> Self {
        let mut grid = Self::default();
        for (index, curve) in curves.iter().enumerate() {
            grid.insert_curve(index, &curve.points);
        }
        grid.dedup_bucket_entries();
        grid
    }

    fn insert_curve(&mut self, index: usize, points: &[WorldPlanePoint]) {
        for point in points {
            self.insert_point(index, *point);
        }
        for segment in points.windows(2) {
            let start = segment[0];
            let end = segment[1];
            let distance = squared_distance(start, end).sqrt();
            let steps = (distance / (MACRO_FIELD_CURVE_BUCKET_BLOCKS * 0.5))
                .ceil()
                .max(1.0) as usize;
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                self.insert_point(
                    index,
                    WorldPlanePoint::new(
                        start.x + (end.x - start.x) * t,
                        start.z + (end.z - start.z) * t,
                    ),
                );
            }
        }
    }

    fn insert_point(&mut self, index: usize, point: WorldPlanePoint) {
        self.buckets
            .entry(curve_bucket(point))
            .or_default()
            .push(index);
    }

    fn dedup_bucket_entries(&mut self) {
        for indices in self.buckets.values_mut() {
            indices.sort_unstable();
            indices.dedup();
        }
    }

    fn candidate_indices(&self, position: WorldPlanePoint, radius: f32) -> Vec<usize> {
        if self.buckets.is_empty() {
            return Vec::new();
        }
        let center = curve_bucket(position);
        let search = curve_bucket_search_radius(radius).max(1);
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
                .entry(site_bucket(site.position))
                .or_default()
                .push(index);
        }
        grid
    }

    // Each site is inserted into exactly one bucket, so callers can traverse the
    // bucket window directly. CurveIndexGrid still sorts/dedups because curves
    // are inserted into every bucket touched by their sampled polyline.
    fn for_each_nearest_candidate_index(
        &self,
        position: WorldPlanePoint,
        mut visit: impl FnMut(usize),
    ) {
        let center = site_bucket(position);
        for search in 1..=4 {
            let mut found = false;
            for z in center.1 - search..=center.1 + search {
                for x in center.0 - search..=center.0 + search {
                    if let Some(bucket) = self.buckets.get(&(x, z)) {
                        found = true;
                        for index in bucket {
                            visit(*index);
                        }
                    }
                }
            }
            if found {
                return;
            }
        }
    }

    fn for_each_candidate_index_in_radius(
        &self,
        position: WorldPlanePoint,
        bucket_radius: i32,
        mut visit: impl FnMut(usize),
    ) {
        if self.buckets.is_empty() {
            return;
        }
        let center = site_bucket(position);
        let radius = bucket_radius.max(0);
        for z in center.1 - radius..=center.1 + radius {
            for x in center.0 - radius..=center.0 + radius {
                if let Some(bucket) = self.buckets.get(&(x, z)) {
                    for index in bucket {
                        visit(*index);
                    }
                }
            }
        }
    }
}

fn nearest_site_order(
    position: WorldPlanePoint,
    candidate: &MacroSite,
    current: Option<&MacroSite>,
) -> std::cmp::Ordering {
    let Some(current) = current else {
        return std::cmp::Ordering::Less;
    };
    squared_distance(position, candidate.position)
        .total_cmp(&squared_distance(position, current.position))
        .then_with(|| candidate.id.0.cmp(&current.id.0))
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
        (
            "boundary_roughness_blocks",
            config.boundary_roughness_blocks,
        ),
        ("ridge_height_scale", config.ridge_height_scale),
        ("river_carve_scale", config.river_carve_scale),
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
    assert!(
        config.boundary_roughness_blocks >= 0.0,
        "boundary roughness must be >= 0"
    );
}

fn combine_macro_height(
    macro_elevation: f32,
    ocean_mask: f32,
    _coast_mask: f32,
    _lake_mask: f32,
    _dry_basin_mask: f32,
    lake_lowering_factor: f32,
    ridge_influence: f32,
    river_valley_strength: f32,
    river_flow_hint: f32,
    config: MacroFieldTileConfig,
) -> f32 {
    let ridge_raise = ridge_influence * config.ridge_height_scale;
    let river_carve = river_valley_strength
        * config.river_carve_scale
        * lerp(0.14, 1.0, smoothstep01(river_flow_hint.clamp(0.0, 1.0)))
        * (1.0 - ocean_mask);
    let mut height = macro_elevation + ridge_raise - river_carve;
    if ocean_mask > 0.5 {
        height = ocean_bathymetry_macro_height(height);
    } else if lake_lowering_factor > 0.0 {
        height = lake_bed_macro_height(height, lake_lowering_factor, config);
    }
    height.clamp(-2.0, 2.0)
}

fn ocean_bathymetry_macro_height(source_height: f32) -> f32 {
    let depth = (-source_height).max(0.0).clamp(0.0, 1.0);
    let coast_adjacent = depth.min(0.018);
    let shelf = smoothstep_range(0.018, 0.07, depth) * 0.05;
    let slope = smoothstep_range(0.07, 0.42, depth) * 0.46;
    let basin = smoothstep_range(0.42, 0.9, depth) * 0.49;

    -(coast_adjacent + shelf + slope + basin).clamp(0.0, 1.0)
}

fn lake_bed_macro_height(
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

fn roughened_distance(
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

fn boundary_roughness_offset(position: WorldPlanePoint, roughness_blocks: f32, salt: u64) -> f32 {
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

fn smooth_value_noise_2d(position: WorldPlanePoint, scale_blocks: f32, salt: u64) -> f32 {
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

fn signed_lattice_noise(x: i32, z: i32, salt: u64) -> f32 {
    let mut value = salt;
    value ^= (x as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= (z as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    let unit = ((value ^ (value >> 31)) as f64 / u64::MAX as f64) as f32;
    unit * 2.0 - 1.0
}

fn smootherstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
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

fn prune_isolated_ocean_fragments(samples: &mut [MacroFieldSample], config: MacroFieldTileConfig) {
    let width = config.width as usize;
    let height = config.height as usize;
    if width == 0 || height == 0 || samples.len() != width * height {
        return;
    }

    let mut visited = vec![false; samples.len()];
    let mut components = Vec::new();
    for start in 0..samples.len() {
        if visited[start] || samples[start].ocean_mask <= 0.5 {
            continue;
        }

        let mut queue = VecDeque::from([start]);
        let mut indices = Vec::new();
        let mut touches_edge = false;
        let mut has_ocean_basin_source = false;
        visited[start] = true;

        while let Some(index) = queue.pop_front() {
            indices.push(index);
            let x = index % width;
            let z = index / width;
            touches_edge |= x == 0 || z == 0 || x + 1 == width || z + 1 == height;
            has_ocean_basin_source |= matches!(
                samples[index].surface_kind,
                Some(MacroSurfaceKind::OceanBasin)
            );

            for neighbor in ocean_component_neighbors(index, x, z, width, height) {
                if !visited[neighbor] && samples[neighbor].ocean_mask > 0.5 {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }

        components.push(OceanComponent {
            indices,
            touches_edge,
            has_ocean_basin_source,
        });
    }

    let max_fragment_samples =
        isolated_ocean_fragment_max_samples(config.sample_spacing_blocks).max(1);
    for component in &components {
        if component.touches_edge
            || (component.has_ocean_basin_source && component.indices.len() > max_fragment_samples)
        {
            continue;
        }

        for &sample_index in &component.indices {
            clear_isolated_ocean_sample(&mut samples[sample_index], config);
        }
    }
}

fn isolated_ocean_fragment_max_samples(sample_spacing_blocks: f32) -> usize {
    let sample_area = sample_spacing_blocks.max(1.0).powi(2);
    (ISOLATED_OCEAN_FRAGMENT_MAX_BLOCK_AREA / sample_area).ceil() as usize
}

#[derive(Debug)]
struct OceanComponent {
    indices: Vec<usize>,
    touches_edge: bool,
    has_ocean_basin_source: bool,
}

fn ocean_component_neighbors(
    index: usize,
    x: usize,
    z: usize,
    width: usize,
    height: usize,
) -> impl Iterator<Item = usize> {
    let mut neighbors = [None; 4];
    if x > 0 {
        neighbors[0] = Some(index - 1);
    }
    if x + 1 < width {
        neighbors[1] = Some(index + 1);
    }
    if z > 0 {
        neighbors[2] = Some(index - width);
    }
    if z + 1 < height {
        neighbors[3] = Some(index + width);
    }
    neighbors.into_iter().flatten()
}

fn clear_isolated_ocean_sample(sample: &mut MacroFieldSample, config: MacroFieldTileConfig) {
    if sample.lake_mask > 0.5 {
        return;
    }

    sample.ocean_mask = 0.0;
    sample.surface_kind = match sample.surface_kind {
        Some(MacroSurfaceKind::CoastOcean) => Some(MacroSurfaceKind::CoastLand),
        Some(MacroSurfaceKind::OceanBasin) => Some(MacroSurfaceKind::Continent),
        other => other,
    };
    sample.biome_context = None;
    sample.biome = None;
    sample.combined_macro_height = combine_macro_height(
        sample.macro_elevation,
        sample.ocean_mask,
        sample.coast_mask,
        sample.lake_mask,
        sample.dry_basin_mask,
        0.0,
        sample.ridge_influence,
        sample.river_valley_strength,
        sample.river_flow_hint,
        config,
    );
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

fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    let span = (edge1 - edge0).max(f32::EPSILON);
    smoothstep01((value - edge0) / span)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
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

fn flow_hint_from_plan(plan: &RiverSegmentPlan) -> f32 {
    let width_t = ((plan.bed_width_blocks - 1.0) / (165.0 - 1.0)).clamp(0.0, 1.0);
    let flow_t = ((plan.discharge_q + 1.0).ln() / (1024.0_f32 + 1.0).ln()).clamp(0.0, 1.0);
    width_t.max(flow_t * 0.85)
}

fn river_valley_strength_for_distance(
    distance_blocks: f32,
    flow_hint: f32,
    planned_width_blocks: f32,
    configured_radius_blocks: f32,
) -> f32 {
    let width = river_width_blocks(flow_hint, planned_width_blocks, configured_radius_blocks);
    if !distance_blocks.is_finite() || distance_blocks >= width {
        return 0.0;
    }
    let t = (distance_blocks / width.max(f32::EPSILON)).clamp(0.0, 1.0);
    (1.0 - smoothstep01(t)).powf(1.15)
}

fn river_width_blocks(
    flow_hint: f32,
    planned_width_blocks: f32,
    configured_radius_blocks: f32,
) -> f32 {
    let flow_t = smoothstep01(flow_hint.clamp(0.0, 1.0));
    let fallback = lerp(3.0, configured_radius_blocks, flow_t);
    planned_width_blocks
        .max(fallback.min(18.0))
        .clamp(1.0, configured_radius_blocks)
}

fn river_depth_factor(flow_hint: f32) -> f32 {
    lerp(0.08, 0.55, smoothstep01(flow_hint.clamp(0.0, 1.0))).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct RiverMorphologySample {
    distance_blocks: f32,
    flow_hint: f32,
    valley_strength: f32,
    bed_depth_hint: f32,
    bank_roughness_hint: f32,
    gravel_hint: f32,
    cutbank_hint: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct RiverMorphologyHints {
    bed_depth_hint: f32,
    bank_roughness_hint: f32,
    gravel_hint: f32,
    cutbank_hint: f32,
}

fn river_hints_from_strength(
    valley_strength: f32,
    flow_hint: f32,
    planned_bed_depth_blocks: f32,
) -> RiverMorphologyHints {
    let valley = valley_strength.clamp(0.0, 1.0);
    let flow = flow_hint.clamp(0.0, 1.0);
    let planned_depth_hint = (planned_bed_depth_blocks.max(0.0) / 40.0).clamp(0.0, 1.0);
    RiverMorphologyHints {
        bed_depth_hint: (valley * planned_depth_hint.max(river_depth_factor(flow) * 0.45))
            .clamp(0.0, 1.0),
        bank_roughness_hint: (valley * (1.0 - flow * 0.45)).clamp(0.0, 1.0),
        gravel_hint: (valley * (0.65 - flow * 0.25)).clamp(0.0, 1.0),
        cutbank_hint: 0.0,
    }
}

fn river_morphology_sample(
    position: WorldPlanePoint,
    points: &[WorldPlanePoint],
    flow_hint: f32,
    planned_width_blocks: f32,
    planned_bed_depth_blocks: f32,
    configured_radius_blocks: f32,
) -> RiverMorphologySample {
    let flow_hint = flow_hint.clamp(0.0, 1.0);
    let distance = polyline_distance(position, points);
    let valley_strength = river_valley_strength_for_distance(
        distance,
        flow_hint,
        planned_width_blocks,
        configured_radius_blocks,
    );
    let hints = river_hints_from_strength(valley_strength, flow_hint, planned_bed_depth_blocks);

    RiverMorphologySample {
        distance_blocks: distance,
        flow_hint,
        valley_strength: valley_strength.clamp(0.0, 1.0),
        bed_depth_hint: hints.bed_depth_hint,
        bank_roughness_hint: hints.bank_roughness_hint,
        gravel_hint: hints.gravel_hint,
        cutbank_hint: hints.cutbank_hint,
    }
}

fn polyline_distance(position: WorldPlanePoint, points: &[WorldPlanePoint]) -> f32 {
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

fn nearest_polyline_segment(
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

fn point_segment_distance(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    point_segment_distance_squared(point, start, end).sqrt()
}

fn point_segment_distance_squared(
    point: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> f32 {
    let dx = end.x - start.x;
    let dz = end.z - start.z;
    let len2 = dx * dx + dz * dz;
    if len2 <= f32::EPSILON {
        return squared_distance(point, start);
    }
    let t = (((point.x - start.x) * dx + (point.z - start.z) * dz) / len2).clamp(0.0, 1.0);
    let projected = WorldPlanePoint::new(start.x + dx * t, start.z + dz * t);
    squared_distance(point, projected)
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

fn curve_bucket_search_radius(radius: f32) -> i32 {
    ((radius + MACRO_FIELD_CURVE_BUCKET_BLOCKS * 0.5) / MACRO_FIELD_CURVE_BUCKET_BLOCKS).ceil()
        as i32
}

fn site_bucket(point: WorldPlanePoint) -> (i32, i32) {
    (
        (point.x / MACRO_FIELD_SITE_BUCKET_BLOCKS).floor() as i32,
        (point.z / MACRO_FIELD_SITE_BUCKET_BLOCKS).floor() as i32,
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
    use crate::world::generation::boundary::{
        BoundaryAnchors, BoundaryConfig, BoundaryGuard, BoundaryProfile, generate_noisy_boundaries,
    };
    use crate::world::generation::graph::{
        DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, VoronoiCornerId,
        VoronoiEdgeId, VoronoiGraphConfig, VoronoiGraphPatchRequest, VoronoiSiteId,
        generate_voronoi_graph_patch,
    };
    use crate::world::generation::hydrology::{
        GraphHydrologyGraph, HydrologyConfig, solve_hydrology,
    };
    use crate::world::generation::macro_map::{MacroMapConfig, generate_macro_map};
    use crate::world::generation::river_plan::{RiverPlan, RiverPlanConfig, build_river_plan};

    #[derive(Debug, Clone, Copy, Default)]
    struct NeighborDeltaSummary {
        max_delta: f32,
        p95_delta: f32,
        pair_count: usize,
        owner_switch_count: usize,
        surface_kind_switch_count: usize,
        hard_mask_switch_count: usize,
        river_influence_count: usize,
        coast_influence_count: usize,
        dry_basin_profile_count: usize,
    }

    #[derive(Debug, Clone, Copy)]
    struct NeighborDeltaPair {
        delta: f32,
        left_index: usize,
        right_index: usize,
    }

    #[derive(Debug, Clone, Copy)]
    struct DenseDeltaSummary {
        center: WorldPlanePoint,
        spacing: f32,
        max_delta: f32,
        p95_delta: f32,
        over_one_block_count: usize,
        pair_count: usize,
    }

    fn combined_neighbor_delta_summary(tile: &MacroFieldTile) -> NeighborDeltaSummary {
        let width = tile.config.width as usize;
        let height = tile.config.height as usize;
        let mut deltas = Vec::new();
        let mut top_pairs = Vec::new();
        let mut summary = NeighborDeltaSummary::default();
        for z in 0..height {
            for x in 0..width {
                let index = z * width + x;
                if x + 1 < width {
                    record_neighbor_delta(
                        tile,
                        index,
                        index + 1,
                        &mut deltas,
                        &mut top_pairs,
                        &mut summary,
                    );
                }
                if z + 1 < height {
                    record_neighbor_delta(
                        tile,
                        index,
                        index + width,
                        &mut deltas,
                        &mut top_pairs,
                        &mut summary,
                    );
                }
            }
        }
        deltas.sort_by(f32::total_cmp);
        summary.pair_count = deltas.len();
        summary.max_delta = deltas.last().copied().unwrap_or_default();
        if !deltas.is_empty() {
            let p95_index = ((deltas.len() - 1) as f32 * 0.95).round() as usize;
            summary.p95_delta = deltas[p95_index.min(deltas.len() - 1)];
        }
        summary
    }

    fn record_neighbor_delta(
        tile: &MacroFieldTile,
        a: usize,
        b: usize,
        deltas: &mut Vec<f32>,
        top_pairs: &mut Vec<NeighborDeltaPair>,
        summary: &mut NeighborDeltaSummary,
    ) {
        let left = tile.samples[a];
        let right = tile.samples[b];
        let delta = (left.combined_macro_height - right.combined_macro_height).abs();
        deltas.push(delta);
        top_pairs.push(NeighborDeltaPair {
            delta,
            left_index: a,
            right_index: b,
        });
        top_pairs.sort_by(|left, right| right.delta.total_cmp(&left.delta));
        top_pairs.truncate(5);
        if left.nearest_site != right.nearest_site {
            summary.owner_switch_count += 1;
        }
        if left.surface_kind != right.surface_kind {
            summary.surface_kind_switch_count += 1;
        }
        if (left.ocean_mask - right.ocean_mask).abs() > f32::EPSILON
            || (left.lake_mask - right.lake_mask).abs() > f32::EPSILON
            || (left.dry_basin_mask - right.dry_basin_mask).abs() > f32::EPSILON
        {
            summary.hard_mask_switch_count += 1;
        }
        if left.river_valley_strength.max(right.river_valley_strength) > 0.05 {
            summary.river_influence_count += 1;
        }
        if left.coast_mask.max(right.coast_mask) > 0.05 {
            summary.coast_influence_count += 1;
        }
        if left.dry_basin_mask.max(right.dry_basin_mask) > 0.5 {
            summary.dry_basin_profile_count += 1;
        }
    }

    fn top_combined_neighbor_delta_pairs(tile: &MacroFieldTile) -> Vec<NeighborDeltaPair> {
        let width = tile.config.width as usize;
        let height = tile.config.height as usize;
        let mut pairs = Vec::new();
        for z in 0..height {
            for x in 0..width {
                let index = z * width + x;
                if x + 1 < width {
                    push_top_pair(tile, index, index + 1, &mut pairs);
                }
                if z + 1 < height {
                    push_top_pair(tile, index, index + width, &mut pairs);
                }
            }
        }
        pairs
    }

    fn push_top_pair(
        tile: &MacroFieldTile,
        left_index: usize,
        right_index: usize,
        pairs: &mut Vec<NeighborDeltaPair>,
    ) {
        let delta = (tile.samples[left_index].combined_macro_height
            - tile.samples[right_index].combined_macro_height)
            .abs();
        pairs.push(NeighborDeltaPair {
            delta,
            left_index,
            right_index,
        });
        pairs.sort_by(|left, right| right.delta.total_cmp(&left.delta));
        pairs.truncate(5);
    }

    fn dense_delta_summary_around_pair(
        inputs: &TestInputs,
        coarse_tile: &MacroFieldTile,
        pair: NeighborDeltaPair,
        spacing: f32,
    ) -> DenseDeltaSummary {
        let left = coarse_tile.samples[pair.left_index];
        let right = coarse_tile.samples[pair.right_index];
        let center = WorldPlanePoint::new(
            (left.position.x + right.position.x) * 0.5,
            (left.position.z + right.position.z) * 0.5,
        );
        let span = if spacing <= 1.0 { 64.0 } else { 96.0 };
        let width = (span / spacing) as u32 + 1;
        let height = width;
        let config = MacroFieldTileConfig::new(
            center.x - span * 0.5,
            center.z - span * 0.5,
            width,
            height,
            spacing,
        );
        let tile = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
            config,
        );
        let summary = combined_neighbor_delta_summary(&tile);
        let one_block_delta = 1.0 / MACRO_FIELD_CONTOUR_HEIGHT_MAX_BLOCKS;
        let mut over_one_block_count = 0;
        let tile_width = tile.config.width as usize;
        let tile_height = tile.config.height as usize;
        for z in 0..tile_height {
            for x in 0..tile_width {
                let index = z * tile_width + x;
                if x + 1 < tile_width {
                    let right = index + 1;
                    let delta = (tile.samples[index].combined_macro_height
                        - tile.samples[right].combined_macro_height)
                        .abs();
                    if delta > one_block_delta {
                        over_one_block_count += 1;
                    }
                }
                if z + 1 < tile_height {
                    let below = index + tile_width;
                    let delta = (tile.samples[index].combined_macro_height
                        - tile.samples[below].combined_macro_height)
                        .abs();
                    if delta > one_block_delta {
                        over_one_block_count += 1;
                    }
                }
            }
        }

        DenseDeltaSummary {
            center,
            spacing,
            max_delta: summary.max_delta,
            p95_delta: summary.p95_delta,
            over_one_block_count,
            pair_count: summary.pair_count,
        }
    }

    #[test]
    fn macro_field_tile_generation_is_deterministic() {
        let inputs = test_inputs(42);
        let config = test_tile_config();

        let first = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
            config,
        );
        let second = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
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
            &inputs.river_plan,
            &inputs.boundary,
            config,
        );

        assert_eq!(tile.samples.len(), config.sample_count());
        assert_eq!(tile.stats.sample_count, config.sample_count());
        assert!(tile.sample(0, 0).is_some());
        assert!(tile.sample(config.width, 0).is_none());
    }

    #[test]
    fn isolated_ocean_fragment_is_pruned_without_touching_connected_ocean() {
        let mut tile = test_contour_tile(&[32.0; 25], 5, 5);
        let config = tile.config;
        for z in 0..5 {
            let index = z * 5;
            tile.samples[index].surface_kind = Some(MacroSurfaceKind::OceanBasin);
            tile.samples[index].ocean_mask = 1.0;
        }

        let fragment_index = 12;
        tile.samples[fragment_index].surface_kind = Some(MacroSurfaceKind::CoastOcean);
        tile.samples[fragment_index].macro_elevation = 0.04;
        tile.samples[fragment_index].ocean_mask = 1.0;
        tile.samples[fragment_index].combined_macro_height = combine_macro_height(
            tile.samples[fragment_index].macro_elevation,
            1.0,
            tile.samples[fragment_index].coast_mask,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            config,
        );

        prune_isolated_ocean_fragments(&mut tile.samples, config);

        assert_eq!(tile.samples[0].ocean_mask, 1.0);
        assert_eq!(tile.samples[20].ocean_mask, 1.0);
        assert_eq!(tile.samples[fragment_index].ocean_mask, 0.0);
        assert_eq!(
            tile.samples[fragment_index].surface_kind,
            Some(MacroSurfaceKind::CoastLand)
        );
        assert!(tile.samples[fragment_index].combined_macro_height > 0.0);
    }

    #[test]
    fn lone_ocean_fragment_is_not_kept_as_largest_component() {
        let mut tile = test_contour_tile(&[32.0; 25], 5, 5);
        let config = tile.config;
        let fragment_index = 12;
        tile.samples[fragment_index].surface_kind = Some(MacroSurfaceKind::CoastOcean);
        tile.samples[fragment_index].macro_elevation = 0.03;
        tile.samples[fragment_index].ocean_mask = 1.0;
        tile.samples[fragment_index].combined_macro_height = combine_macro_height(
            tile.samples[fragment_index].macro_elevation,
            1.0,
            tile.samples[fragment_index].coast_mask,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            config,
        );

        prune_isolated_ocean_fragments(&mut tile.samples, config);

        assert_eq!(tile.samples[fragment_index].ocean_mask, 0.0);
        assert_eq!(
            tile.samples[fragment_index].surface_kind,
            Some(MacroSurfaceKind::CoastLand)
        );
        assert!(
            tile.samples[fragment_index].combined_macro_height > 0.0,
            "isolated ocean owner sample should return to interpolated land/coast height"
        );
    }

    #[test]
    fn large_detached_coast_ocean_fragment_is_pruned() {
        let mut tile = test_contour_tile(&[32.0; 900], 30, 30);
        let config = tile.config;
        for z in 5..25 {
            for x in 5..25 {
                let index = z * 30 + x;
                tile.samples[index].surface_kind = Some(MacroSurfaceKind::CoastOcean);
                tile.samples[index].macro_elevation = 0.03;
                tile.samples[index].ocean_mask = 1.0;
                tile.samples[index].combined_macro_height = combine_macro_height(
                    tile.samples[index].macro_elevation,
                    1.0,
                    tile.samples[index].coast_mask,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    config,
                );
            }
        }

        prune_isolated_ocean_fragments(&mut tile.samples, config);

        assert!(
            tile.samples.iter().all(|sample| sample.ocean_mask <= 0.5),
            "detached CoastOcean-only blobs should not survive as standalone ocean"
        );
        assert!(
            tile.samples
                .iter()
                .all(|sample| sample.surface_kind != Some(MacroSurfaceKind::CoastOcean)),
            "cleared coast-ocean fragments should be reclassified to coast-land"
        );
    }

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
    fn ocean_bathymetry_uses_narrow_shelf_before_slope() {
        let near_coast = ocean_bathymetry_macro_height(-0.03);
        let shelf_edge = ocean_bathymetry_macro_height(-0.08);
        let slope = ocean_bathymetry_macro_height(-0.32);

        assert!(
            shelf_edge < near_coast - 0.025,
            "shelf should narrow quickly after the coast: near={near_coast} shelf_edge={shelf_edge}"
        );
        assert!(
            slope < shelf_edge - 0.25,
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
    fn combined_macro_height_is_finite_and_sane() {
        let inputs = test_inputs(42);
        let tile = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
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
    #[ignore = "diagnostic helper for combined_macro_height continuity tuning"]
    fn diagnose_combined_macro_height_neighbor_deltas() {
        for seed in [7, 42, 91] {
            let inputs = test_inputs(seed);
            let tile = generate_macro_field_tile(
                &inputs.patch,
                &inputs.macro_map,
                &inputs.river_plan,
                &inputs.boundary,
                MacroFieldTileConfig::new(-768.0, -768.0, 32, 32, 48.0),
            );
            let summary = combined_neighbor_delta_summary(&tile);
            eprintln!(
                "seed={seed} max={:.6} p95={:.6} pairs={} owner={} kind={} mask={} coast={} river={} dry={}",
                summary.max_delta,
                summary.p95_delta,
                summary.pair_count,
                summary.owner_switch_count,
                summary.surface_kind_switch_count,
                summary.hard_mask_switch_count,
                summary.coast_influence_count,
                summary.river_influence_count,
                summary.dry_basin_profile_count
            );
            for pair in top_combined_neighbor_delta_pairs(&tile).into_iter().take(3) {
                let left = tile.samples[pair.left_index];
                let right = tile.samples[pair.right_index];
                eprintln!(
                    "  top delta={:.6} left=({:.0},{:.0}) site={:?} kind={:?} macro={:.6} coast={:.3} river={:.3}/{:.3} combined={:.6} right=({:.0},{:.0}) site={:?} kind={:?} macro={:.6} coast={:.3} river={:.3}/{:.3} combined={:.6}",
                    pair.delta,
                    left.position.x,
                    left.position.z,
                    left.nearest_site,
                    left.surface_kind,
                    left.macro_elevation,
                    left.coast_mask,
                    left.river_valley_strength,
                    left.river_flow_hint,
                    left.combined_macro_height,
                    right.position.x,
                    right.position.z,
                    right.nearest_site,
                    right.surface_kind,
                    right.macro_elevation,
                    right.coast_mask,
                    right.river_valley_strength,
                    right.river_flow_hint,
                    right.combined_macro_height
                );
            }
            if let Some(pair) = top_combined_neighbor_delta_pairs(&tile).into_iter().next() {
                let dense = dense_delta_summary_around_pair(&inputs, &tile, pair, 1.0);
                eprintln!(
                    "  dense spacing={:.1} center=({:.0},{:.0}) max={:.6} p95={:.6} over_1block={}/{}",
                    dense.spacing,
                    dense.center.x,
                    dense.center.z,
                    dense.max_delta,
                    dense.p95_delta,
                    dense.over_one_block_count,
                    dense.pair_count
                );
            }
        }
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
            "river broad-valley carve should still lower combined height: river={river} base={base}"
        );
        assert_eq!(
            config.river_carve_scale, 0.018,
            "default broad-valley carve should stay block-scale and leave bed depth to heightfield"
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
            super::super::graph::VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::LakeCandidate,
            -0.04,
        );
        let land = test_site(
            super::super::graph::VoronoiSiteId(2),
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
    fn river_connected_broad_stroke_overlap_uses_soft_union_without_cusp() {
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
            field.river_valley_strength[overlap_inside] >= weakest_shoulder * 0.95,
            "connected broad stroke overlap should not leave a pointed weak cusp: overlap={} shoulder_h={} shoulder_v={}",
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
    fn river_parallel_independent_corridors_do_not_soft_union_into_one_owner() {
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
    fn river_width_and_depth_increase_with_flow() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let headwater_flow = flow_hint(12.0);
        let trunk_flow = flow_hint(1024.0);

        assert!(
            river_width_blocks(headwater_flow, 4.0, radius)
                < river_width_blocks(trunk_flow, 180.0, radius),
            "river corridor width should grow with selected/display flow"
        );
        assert!(
            river_depth_factor(headwater_flow) < river_depth_factor(trunk_flow),
            "river carve depth should grow with selected/display flow"
        );
        assert!(
            river_width_blocks(headwater_flow, 4.0, radius) < radius * 0.10,
            "headwater rivers should be much narrower than the maximum downstream radius"
        );
    }

    #[test]
    fn river_valley_uses_flow_scaled_width() {
        let radius = DEFAULT_MACRO_FIELD_RIVER_RADIUS_BLOCKS;
        let distance = radius * 0.45;
        let headwater = river_valley_strength_for_distance(distance, flow_hint(12.0), 4.0, radius);
        let trunk = river_valley_strength_for_distance(distance, flow_hint(1024.0), 180.0, radius);

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
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
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
    fn ordinary_land_boundary_secondary_switch_does_not_override_scalar_elevation() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let primary = test_site(
            VoronoiSiteId(1),
            -100.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.02,
        );
        let coastland_secondary = test_site(
            VoronoiSiteId(2),
            100.0,
            -10.0,
            MacroSurfaceKind::CoastLand,
            -0.20,
        );
        let continent_secondary = test_site(
            VoronoiSiteId(3),
            100.0,
            10.0,
            MacroSurfaceKind::Continent,
            0.80,
        );
        let edge_a = VoronoiEdgeId(81);
        let edge_b = VoronoiEdgeId(82);
        let start_a = WorldPlanePoint::new(0.0, -32.0);
        let end_a = WorldPlanePoint::new(0.0, 32.0);
        let start_b = WorldPlanePoint::new(1.0, -32.0);
        let end_b = WorldPlanePoint::new(1.0, 32.0);
        let boundary = BoundaryCache {
            curves: vec![
                NoisyBoundaryCurve {
                    edge: edge_a,
                    profile: BoundaryProfile::Ordinary,
                    anchors: BoundaryAnchors {
                        corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                        sites: [primary.id, coastland_secondary.id],
                        start: start_a,
                        end: end_a,
                    },
                    points: vec![start_a, end_a],
                    amplitude: 0.0,
                    seed: 1,
                    guard: BoundaryGuard {
                        min_x: -8.0,
                        max_x: 8.0,
                        min_z: -40.0,
                        max_z: 40.0,
                    },
                },
                NoisyBoundaryCurve {
                    edge: edge_b,
                    profile: BoundaryProfile::Ordinary,
                    anchors: BoundaryAnchors {
                        corners: [VoronoiCornerId(3), VoronoiCornerId(4)],
                        sites: [primary.id, continent_secondary.id],
                        start: start_b,
                        end: end_b,
                    },
                    points: vec![start_b, end_b],
                    amplitude: 0.0,
                    seed: 2,
                    guard: BoundaryGuard {
                        min_x: -8.0,
                        max_x: 8.0,
                        min_z: -40.0,
                        max_z: 40.0,
                    },
                },
            ],
            stats: Default::default(),
        };
        let macro_map = GraphMacroMap {
            sites: vec![primary, coastland_secondary, continent_secondary],
            corners: Vec::new(),
            edges: vec![
                MacroEdge {
                    id: edge_a,
                    sites: [primary.id, coastland_secondary.id],
                    corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                    guide: MacroEdgeGuide {
                        is_coast: false,
                        is_ridge_candidate: false,
                        is_river_candidate: false,
                        is_fault_candidate: false,
                        coastness: 0.0,
                        mountainness: 0.0,
                        ridgeness: 0.0,
                        signed_elevation_gradient: 0.22,
                        drainage_divide_potential: 0.0,
                        river_potential: 0.0,
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                },
                MacroEdge {
                    id: edge_b,
                    sites: [primary.id, continent_secondary.id],
                    corners: [VoronoiCornerId(3), VoronoiCornerId(4)],
                    guide: MacroEdgeGuide {
                        is_coast: false,
                        is_ridge_candidate: false,
                        is_river_candidate: false,
                        is_fault_candidate: false,
                        coastness: 0.0,
                        mountainness: 0.0,
                        ridgeness: 0.0,
                        signed_elevation_gradient: 0.78,
                        drainage_divide_potential: 0.0,
                        river_potential: 0.0,
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                },
            ],
            biomes: Vec::new(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 8.0;
        let left_position = WorldPlanePoint::new(-0.1, 0.0);
        let right_position = WorldPlanePoint::new(0.9, 0.0);

        let left = context.owner_sample(left_position, config);
        let right = context.owner_sample(right_position, config);

        assert_eq!(left.primary.map(|site| site.id), Some(primary.id));
        assert_eq!(right.primary.map(|site| site.id), Some(primary.id));
        let low = coastland_secondary
            .signed_macro_elevation
            .min(primary.signed_macro_elevation)
            .min(continent_secondary.signed_macro_elevation);
        let high = coastland_secondary
            .signed_macro_elevation
            .max(primary.signed_macro_elevation)
            .max(continent_secondary.signed_macro_elevation);
        assert!(
            (low..=high).contains(&left.macro_elevation),
            "ordinary coastland/land secondary selection must stay inside macro_map source range: {} not in {low}..={high}",
            left.macro_elevation
        );
        assert!(
            (low..=high).contains(&right.macro_elevation),
            "ordinary land/land secondary selection must stay inside macro_map source range: {} not in {low}..={high}",
            right.macro_elevation
        );
        assert!(
            (left.macro_elevation - right.macro_elevation).abs() < 0.005,
            "adjacent ordinary-boundary secondary switches should stay continuous, left={} right={}",
            left.macro_elevation,
            right.macro_elevation
        );
    }

    #[test]
    fn macro_field_scalar_interpolates_between_macro_map_sites() {
        use crate::world::generation::graph::VoronoiSiteId;
        use crate::world::generation::macro_map::MacroSurfaceKind;

        let low_site = test_site(
            VoronoiSiteId(101),
            -128.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.0,
        );
        let high_site = test_site(
            VoronoiSiteId(102),
            128.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.8,
        );
        let macro_map = GraphMacroMap {
            sites: vec![low_site, high_site],
            corners: Vec::new(),
            edges: Vec::new(),
            biomes: Vec::new(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let boundary = BoundaryCache {
            curves: Vec::new(),
            stats: Default::default(),
        };
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let config = test_tile_config();

        let low_sample = sample_macro_field_point(&context, config, low_site.position);
        let center_sample =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(0.0, 0.0));
        let high_sample = sample_macro_field_point(&context, config, high_site.position);

        assert!(
            (low_sample.macro_elevation - low_site.signed_macro_elevation).abs() <= f32::EPSILON,
            "source site center should preserve its macro_map elevation"
        );
        assert!(
            (high_sample.macro_elevation - high_site.signed_macro_elevation).abs() <= f32::EPSILON,
            "source site center should preserve its macro_map elevation"
        );
        assert!(
            center_sample.macro_elevation > low_site.signed_macro_elevation
                && center_sample.macro_elevation < high_site.signed_macro_elevation,
            "between-site scalar should be an interpolated contour-bearing value, got {}",
            center_sample.macro_elevation
        );
    }

    #[test]
    fn coast_boundary_preserves_owner_source_elevation() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
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
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 24.0;

        let shoreline_land =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(-1.0, 0.0));
        let recovered_land =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(-32.0, 0.0));
        let shoreline_ocean =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(1.0, 0.0));

        let low = land
            .signed_macro_elevation
            .min(ocean.signed_macro_elevation);
        let high = land
            .signed_macro_elevation
            .max(ocean.signed_macro_elevation);
        assert!(
            (low..=high).contains(&shoreline_land.macro_elevation),
            "land-side coast scalar should interpolate macro_map sources without creating a new profile: {} not in {low}..={high}",
            shoreline_land.macro_elevation
        );
        assert!(
            recovered_land.macro_elevation > shoreline_land.macro_elevation,
            "land sample farther from ocean should recover through local source interpolation: shoreline={} recovered={}",
            shoreline_land.macro_elevation,
            recovered_land.macro_elevation
        );
        assert!(
            (low..=high).contains(&shoreline_ocean.macro_elevation),
            "ocean side scalar should stay within macro_map source range, got {} not in {low}..={high}",
            shoreline_ocean.macro_elevation
        );
        assert!(
            shoreline_land.coast_mask > 0.0 && shoreline_ocean.coast_mask > 0.0,
            "explicit coast guide influence should keep coast mask data intact"
        );
    }

    #[test]
    fn nearby_ordinary_boundary_does_not_override_coast_source_elevation() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let land = test_site(
            VoronoiSiteId(1),
            -2.0,
            -10.0,
            MacroSurfaceKind::CoastLand,
            0.8,
        );
        let ocean = test_site(
            VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::CoastOcean,
            -0.8,
        );
        let inland = test_site(
            VoronoiSiteId(3),
            -2.0,
            20.0,
            MacroSurfaceKind::Continent,
            0.8,
        );
        let coast_edge = VoronoiEdgeId(71);
        let ordinary_edge = VoronoiEdgeId(72);
        let coast_start = WorldPlanePoint::new(0.0, -10.0);
        let coast_end = WorldPlanePoint::new(0.0, 10.0);
        let ordinary_start = WorldPlanePoint::new(-20.0, 0.05);
        let ordinary_end = WorldPlanePoint::new(0.0, 0.05);
        let macro_map = GraphMacroMap {
            sites: vec![land, ocean, inland],
            corners: Vec::new(),
            edges: vec![
                MacroEdge {
                    id: coast_edge,
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
                },
                MacroEdge {
                    id: ordinary_edge,
                    sites: [land.id, inland.id],
                    corners: [VoronoiCornerId(3), VoronoiCornerId(4)],
                    guide: MacroEdgeGuide {
                        is_coast: false,
                        is_ridge_candidate: false,
                        is_river_candidate: false,
                        is_fault_candidate: false,
                        coastness: 0.0,
                        mountainness: 0.0,
                        ridgeness: 0.0,
                        signed_elevation_gradient: 0.0,
                        drainage_divide_potential: 0.0,
                        river_potential: 0.0,
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                },
            ],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![
                NoisyBoundaryCurve {
                    edge: coast_edge,
                    profile: BoundaryProfile::Coast,
                    anchors: BoundaryAnchors {
                        corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                        sites: [land.id, ocean.id],
                        start: coast_start,
                        end: coast_end,
                    },
                    points: vec![coast_start, coast_end],
                    amplitude: 0.0,
                    seed: 1,
                    guard: BoundaryGuard {
                        min_x: -48.0,
                        max_x: 48.0,
                        min_z: -20.0,
                        max_z: 20.0,
                    },
                },
                NoisyBoundaryCurve {
                    edge: ordinary_edge,
                    profile: BoundaryProfile::Ordinary,
                    anchors: BoundaryAnchors {
                        corners: [VoronoiCornerId(3), VoronoiCornerId(4)],
                        sites: [land.id, inland.id],
                        start: ordinary_start,
                        end: ordinary_end,
                    },
                    points: vec![ordinary_start, ordinary_end],
                    amplitude: 0.0,
                    seed: 2,
                    guard: BoundaryGuard {
                        min_x: -24.0,
                        max_x: 0.0,
                        min_z: -20.0,
                        max_z: 24.0,
                    },
                },
            ],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 24.0;

        let near_ordinary_and_coast =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(-1.0, 0.0));
        assert_eq!(near_ordinary_and_coast.nearest_site, Some(land.id));
        assert!(
            (ocean.signed_macro_elevation..=land.signed_macro_elevation)
                .contains(&near_ordinary_and_coast.macro_elevation),
            "ordinary/coast boundary proximity should interpolate only existing source elevations, got {}",
            near_ordinary_and_coast.macro_elevation
        );
    }

    #[test]
    fn coast_adjacent_owner_samples_preserve_macro_map_source_elevation() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let ocean = test_site(
            VoronoiSiteId(1),
            96.0,
            0.0,
            MacroSurfaceKind::CoastOcean,
            -0.36,
        );
        let coast = test_site(
            VoronoiSiteId(2),
            -48.0,
            0.0,
            MacroSurfaceKind::CoastLand,
            0.045,
        );
        let inland_a = test_site(
            VoronoiSiteId(3),
            -256.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.44,
        );
        let inland_b = test_site(
            VoronoiSiteId(4),
            -544.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.58,
        );
        let inland_c = test_site(
            VoronoiSiteId(5),
            -864.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.39,
        );
        let edge = VoronoiEdgeId(90);
        let start = WorldPlanePoint::new(0.0, -1024.0);
        let end = WorldPlanePoint::new(0.0, 1024.0);
        let macro_map = GraphMacroMap {
            sites: vec![ocean, coast, inland_a, inland_b, inland_c],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [coast.id, ocean.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: true,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 1.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 0.405,
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
                    sites: [coast.id, ocean.id],
                    start,
                    end,
                },
                points: vec![start, end],
                amplitude: 0.0,
                seed: 7,
                guard: BoundaryGuard {
                    min_x: -128.0,
                    max_x: 128.0,
                    min_z: -1024.0,
                    max_z: 1024.0,
                },
            }],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 96.0;
        let coast_x = [-8.0, -16.0, -24.0, -32.0, -40.0, -48.0, -56.0, -64.0];
        for x in coast_x {
            let sample = sample_macro_field_point(&context, config, WorldPlanePoint::new(x, 0.0));
            assert_eq!(sample.nearest_site, Some(coast.id));
            let source_min = ocean
                .signed_macro_elevation
                .min(coast.signed_macro_elevation);
            let source_max = inland_b
                .signed_macro_elevation
                .max(coast.signed_macro_elevation);
            assert!(
                (source_min..=source_max).contains(&sample.macro_elevation),
                "coast owner sample should remain within macro_map source elevation range at x={x}: {} not in {source_min}..={source_max}",
                sample.macro_elevation
            );
            assert!(
                (sample.combined_macro_height - sample.macro_elevation).abs() <= f32::EPSILON,
                "coast owner sample without river/lake should not get a macro_field height override"
            );
        }
    }

    #[test]
    fn coastland_continent_scalar_preserves_each_owner_source() {
        use crate::world::generation::graph::VoronoiSiteId;
        use crate::world::generation::macro_map::MacroSurfaceKind;

        let coast = test_site(
            VoronoiSiteId(31),
            0.0,
            0.0,
            MacroSurfaceKind::CoastLand,
            0.04,
        );
        let inland = test_site(
            VoronoiSiteId(32),
            -128.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.80,
        );
        let macro_map = GraphMacroMap {
            sites: vec![coast, inland],
            corners: Vec::new(),
            edges: Vec::new(),
            biomes: Vec::new(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let boundary = BoundaryCache {
            curves: Vec::new(),
            stats: Default::default(),
        };
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let config = test_tile_config();
        let coast_sample = sample_macro_field_point(&context, config, coast.position);
        let inland_sample = sample_macro_field_point(&context, config, inland.position);

        assert_eq!(coast_sample.nearest_site, Some(coast.id));
        assert_eq!(inland_sample.nearest_site, Some(inland.id));
        assert!(
            (coast_sample.macro_elevation - coast.signed_macro_elevation).abs() <= f32::EPSILON,
            "coastland sample should preserve macro_map source elevation: sample={} source={}",
            coast_sample.macro_elevation,
            coast.signed_macro_elevation
        );
        assert!(
            (inland_sample.macro_elevation - inland.signed_macro_elevation).abs() <= f32::EPSILON,
            "continent sample should preserve macro_map source elevation: sample={} source={}",
            inland_sample.macro_elevation,
            inland.signed_macro_elevation
        );
    }

    #[test]
    fn coastland_continent_boundary_owner_switch_preserves_owner_sources() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let coast = test_site(
            VoronoiSiteId(41),
            -64.0,
            0.0,
            MacroSurfaceKind::CoastLand,
            0.045,
        );
        let inland = test_site(
            VoronoiSiteId(42),
            64.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.52,
        );
        let edge = VoronoiEdgeId(91);
        let start = WorldPlanePoint::new(0.0, -256.0);
        let end = WorldPlanePoint::new(0.0, 256.0);
        let macro_map = GraphMacroMap {
            sites: vec![coast, inland],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [coast.id, inland.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: false,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 0.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 0.475,
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
                profile: BoundaryProfile::Ordinary,
                anchors: BoundaryAnchors {
                    corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                    sites: [coast.id, inland.id],
                    start,
                    end,
                },
                points: vec![start, end],
                amplitude: 0.0,
                seed: 9,
                guard: BoundaryGuard {
                    min_x: -96.0,
                    max_x: 96.0,
                    min_z: -256.0,
                    max_z: 256.0,
                },
            }],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 48.0;

        let coast_side =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(-1.0, 0.0));
        let inland_side =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(1.0, 0.0));

        assert_eq!(coast_side.nearest_site, Some(coast.id));
        assert_eq!(inland_side.nearest_site, Some(inland.id));
        assert!(
            (coast.signed_macro_elevation..=inland.signed_macro_elevation)
                .contains(&coast_side.macro_elevation),
            "coastland side should interpolate macro_map source elevation range: {}",
            coast_side.macro_elevation
        );
        assert!(
            (coast.signed_macro_elevation..=inland.signed_macro_elevation)
                .contains(&inland_side.macro_elevation),
            "continent side should interpolate macro_map source elevation range: {}",
            inland_side.macro_elevation
        );
        assert!(
            (coast_side.macro_elevation - inland_side.macro_elevation).abs() < 0.02,
            "ordinary boundary scalar should stay continuous across owner switch: coast_side={} inland_side={}",
            coast_side.macro_elevation,
            inland_side.macro_elevation
        );
    }

    #[test]
    fn dry_basin_boundary_blend_does_not_create_coast_mask() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
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
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
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
            &inputs.river_plan,
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
        river_plan: RiverPlan,
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
        let river_plan =
            build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let boundary = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(seed, 11));

        TestInputs {
            patch,
            macro_map,
            hydrology,
            river_plan,
            boundary,
        }
    }

    fn test_tile_config() -> MacroFieldTileConfig {
        MacroFieldTileConfig::new(-512.0, -512.0, 24, 24, 64.0)
    }

    fn test_noisy_curve(edge: u32, points: Vec<WorldPlanePoint>) -> NoisyBoundaryCurve {
        let start = points
            .first()
            .copied()
            .unwrap_or_else(|| WorldPlanePoint::new(0.0, 0.0));
        let end = points.last().copied().unwrap_or(start);
        let (mut min_x, mut max_x) = (start.x, start.x);
        let (mut min_z, mut max_z) = (start.z, start.z);
        for point in &points {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_z = min_z.min(point.z);
            max_z = max_z.max(point.z);
        }
        NoisyBoundaryCurve {
            edge: VoronoiEdgeId(edge as u64),
            profile: BoundaryProfile::Ordinary,
            anchors: BoundaryAnchors {
                corners: [
                    VoronoiCornerId(edge as u64 * 2),
                    VoronoiCornerId(edge as u64 * 2 + 1),
                ],
                sites: [
                    VoronoiSiteId(edge as u64 * 2),
                    VoronoiSiteId(edge as u64 * 2 + 1),
                ],
                start,
                end,
            },
            points,
            amplitude: 0.0,
            seed: edge as u64,
            guard: BoundaryGuard {
                min_x: min_x - 64.0,
                max_x: max_x + 64.0,
                min_z: min_z - 64.0,
                max_z: max_z + 64.0,
            },
        }
    }

    fn test_river_source<'a>(
        curve: &'a NoisyBoundaryCurve,
        flow_hint: f32,
    ) -> RiverRasterSource<'a> {
        RiverRasterSource {
            edge: curve.edge,
            points: &curve.points,
            flow_hint,
            valley_width_blocks: lerp(4.0, 180.0, flow_hint.clamp(0.0, 1.0)),
            bed_depth_blocks: lerp(1.5, 18.0, flow_hint.clamp(0.0, 1.0)),
            component_id: 0,
        }
    }

    fn flow_hint(flow_accumulation: f32) -> f32 {
        (flow_accumulation.max(0.0).sqrt() / 32.0).clamp(0.0, 1.0)
    }

    fn river_width_blocks(
        flow_hint: f32,
        planned_width_blocks: f32,
        configured_radius_blocks: f32,
    ) -> f32 {
        super::river_width_blocks(flow_hint, planned_width_blocks, configured_radius_blocks)
    }

    fn river_depth_factor(flow_hint: f32) -> f32 {
        super::river_depth_factor(flow_hint)
    }

    fn river_valley_strength_for_distance(
        distance_blocks: f32,
        flow_hint: f32,
        planned_width_blocks: f32,
        configured_radius_blocks: f32,
    ) -> f32 {
        super::river_valley_strength_for_distance(
            distance_blocks,
            flow_hint,
            planned_width_blocks,
            configured_radius_blocks,
        )
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
                    river_bed_depth_hint: 0.0,
                    river_bank_roughness_hint: 0.0,
                    river_gravel_hint: 0.0,
                    river_cutbank_hint: 0.0,
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
