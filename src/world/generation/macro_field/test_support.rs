use super::contour::*;
use super::generate_macro_field_tile;
use super::influence::*;
use super::types::*;
use crate::world::generation::boundary::{
    BoundaryAnchors, BoundaryCache, BoundaryConfig, BoundaryGuard, BoundaryProfile,
    NoisyBoundaryCurve, generate_noisy_boundaries,
};
use crate::world::generation::graph::{
    DEFAULT_GRAPH_REGION_SIZE_BLOCKS, DEFAULT_SITE_SPACING_BLOCKS, VoronoiCornerId, VoronoiEdgeId,
    VoronoiGraphConfig, VoronoiGraphPatchRequest, VoronoiSiteId, WorldPlanePoint,
    generate_voronoi_graph_patch,
};
use crate::world::generation::hydrology::{HydrologyConfig, solve_hydrology};
use crate::world::generation::macro_map::{
    GraphMacroMap, MacroMapConfig, MacroSite, MacroSurfaceKind, generate_macro_map,
};
use crate::world::generation::river_plan::{RiverPlan, RiverPlanConfig, build_river_plan};

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::world::generation::macro_field) struct NeighborDeltaSummary {
    pub(in crate::world::generation::macro_field) max_delta: f32,
    pub(in crate::world::generation::macro_field) p95_delta: f32,
    pub(in crate::world::generation::macro_field) pair_count: usize,
    pub(in crate::world::generation::macro_field) owner_switch_count: usize,
    pub(in crate::world::generation::macro_field) surface_kind_switch_count: usize,
    pub(in crate::world::generation::macro_field) hard_mask_switch_count: usize,
    pub(in crate::world::generation::macro_field) river_influence_count: usize,
    pub(in crate::world::generation::macro_field) coast_influence_count: usize,
    pub(in crate::world::generation::macro_field) dry_basin_profile_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world::generation::macro_field) struct NeighborDeltaPair {
    pub(in crate::world::generation::macro_field) delta: f32,
    pub(in crate::world::generation::macro_field) left_index: usize,
    pub(in crate::world::generation::macro_field) right_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::world::generation::macro_field) struct DenseDeltaSummary {
    pub(in crate::world::generation::macro_field) center: WorldPlanePoint,
    pub(in crate::world::generation::macro_field) spacing: f32,
    pub(in crate::world::generation::macro_field) max_delta: f32,
    pub(in crate::world::generation::macro_field) p95_delta: f32,
    pub(in crate::world::generation::macro_field) over_one_block_count: usize,
    pub(in crate::world::generation::macro_field) pair_count: usize,
}

pub(in crate::world::generation::macro_field) fn combined_neighbor_delta_summary(
    tile: &MacroFieldTile,
) -> NeighborDeltaSummary {
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

pub(in crate::world::generation::macro_field) fn record_neighbor_delta(
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

pub(in crate::world::generation::macro_field) fn top_combined_neighbor_delta_pairs(
    tile: &MacroFieldTile,
) -> Vec<NeighborDeltaPair> {
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

pub(in crate::world::generation::macro_field) fn push_top_pair(
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

pub(in crate::world::generation::macro_field) fn dense_delta_summary_around_pair(
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

pub(in crate::world::generation::macro_field) struct TestInputs {
    pub(in crate::world::generation::macro_field) patch:
        crate::world::generation::graph::VoronoiGraphPatch,
    pub(in crate::world::generation::macro_field) macro_map: GraphMacroMap,
    pub(in crate::world::generation::macro_field) river_plan: RiverPlan,
    pub(in crate::world::generation::macro_field) boundary: BoundaryCache,
}

pub(in crate::world::generation::macro_field) fn test_inputs(seed: u64) -> TestInputs {
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
    let river_plan = build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
    let boundary = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(seed, 11));

    TestInputs {
        patch,
        macro_map,
        river_plan,
        boundary,
    }
}

pub(in crate::world::generation::macro_field) fn test_tile_config() -> MacroFieldTileConfig {
    MacroFieldTileConfig::new(-512.0, -512.0, 24, 24, 64.0)
}

pub(in crate::world::generation::macro_field) fn test_junction_curve(
    edge: VoronoiEdgeId,
    start_corner: VoronoiCornerId,
    end_corner: VoronoiCornerId,
    sites: [VoronoiSiteId; 2],
    start: WorldPlanePoint,
    end: WorldPlanePoint,
) -> NoisyBoundaryCurve {
    NoisyBoundaryCurve {
        edge,
        profile: BoundaryProfile::Ordinary,
        anchors: BoundaryAnchors {
            corners: [start_corner, end_corner],
            sites,
            start,
            end,
        },
        points: vec![start, end],
        amplitude: 0.0,
        seed: edge.0,
        guard: BoundaryGuard {
            min_x: start.x.min(end.x) - 16.0,
            max_x: start.x.max(end.x) + 16.0,
            min_z: start.z.min(end.z) - 16.0,
            max_z: start.z.max(end.z) + 16.0,
        },
    }
}

pub(in crate::world::generation::macro_field) fn test_contour_tile(
    heights_blocks: &[f32],
    width: u32,
    height: u32,
) -> MacroFieldTile {
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
                river_core_strength: 0.0,
                river_shoulder_strength: 0.0,
                river_valley_strength: 0.0,
                river_distance_blocks: f32::INFINITY,
                river_flow_hint: 0.0,
                river_longitudinal_blocks: 0.0,
                river_core_depth_hint: 0.0,
                river_bank_roughness_hint: 0.0,
                river_gravel_hint: 0.0,
                river_cutbank_hint: 0.0,
                estuary_water_strength: 0.0,
                estuary_water_depth_hint: 0.0,
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

pub(in crate::world::generation::macro_field) fn blocks_to_combined_macro_height(
    height_blocks: f32,
) -> f32 {
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

pub(in crate::world::generation::macro_field) fn test_site(
    id: crate::world::generation::graph::VoronoiSiteId,
    x: f32,
    z: f32,
    surface_kind: MacroSurfaceKind,
    signed_macro_elevation: f32,
) -> MacroSite {
    MacroSite {
        id,
        owner_region: crate::world::generation::graph::GraphRegionCoord { x: 0, z: 0 },
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
