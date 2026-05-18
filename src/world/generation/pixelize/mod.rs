use rayon::prelude::*;

use super::heightfield::{
    heightfield_column_from_sample, HeightfieldConfig, HeightfieldTerrainKind,
};
use super::macro_field::{MacroFieldSample, MacroFieldTile};
use crate::world::legacy::coord::{world_to_chunk_local, WorldBlockCoord, CHUNK_EDGE_I32};

const WORLD_BLOCK_EPSILON: f32 = 0.001;
const RIVER_CONCAVE_CUSP_MIN_STRENGTH_RATIO: f32 = 0.72;
const RIVER_CONCAVE_CUSP_MIN_NEIGHBORS: usize = 5;
const RIVER_CONCAVE_CUSP_MAX_PASSES: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelizeConfig {
    pub heightfield: HeightfieldConfig,
}

impl Default for PixelizeConfig {
    fn default() -> Self {
        Self {
            heightfield: HeightfieldConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelizedTerrainKind {
    Ocean,
    Lake,
    River,
    DryBasin,
    Ridge,
    Coast,
    Land,
}

impl From<HeightfieldTerrainKind> for PixelizedTerrainKind {
    fn from(value: HeightfieldTerrainKind) -> Self {
        match value {
            HeightfieldTerrainKind::Ocean => Self::Ocean,
            HeightfieldTerrainKind::Lake => Self::Lake,
            HeightfieldTerrainKind::River => Self::River,
            HeightfieldTerrainKind::DryBasin => Self::DryBasin,
            HeightfieldTerrainKind::Ridge => Self::Ridge,
            HeightfieldTerrainKind::Coast => Self::Coast,
            HeightfieldTerrainKind::Land => Self::Land,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelizedColumn {
    pub world_x: i32,
    pub world_z: i32,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub local_x: u8,
    pub local_z: u8,
    pub surface_y: i32,
    pub water_y: Option<i32>,
    pub terrain_kind: PixelizedTerrainKind,
    pub source_macro_elevation: f32,
    pub source_combined_macro_height: f32,
    pub source_ocean_mask: f32,
    pub source_lake_mask: f32,
    pub source_coast_mask: f32,
    pub source_dry_basin_mask: f32,
    pub source_ridge_influence: f32,
    pub source_terrain_ruggedness: f32,
    pub source_river_valley_strength: f32,
    pub source_river_flow_hint: f32,
}

impl PixelizedColumn {
    pub fn has_water_hint(&self) -> bool {
        self.water_y.is_some()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PixelizedChunkAreaStats {
    pub column_count: usize,
    pub min_surface_y: i32,
    pub max_surface_y: i32,
    pub water_column_count: usize,
    pub ocean_column_count: usize,
    pub lake_column_count: usize,
    pub river_hint_column_count: usize,
    pub dry_basin_column_count: usize,
    pub ridge_column_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PixelizedChunkArea {
    pub origin_world_x: i32,
    pub origin_world_z: i32,
    pub width: u32,
    pub height: u32,
    pub min_chunk_x: i32,
    pub max_chunk_x: i32,
    pub min_chunk_z: i32,
    pub max_chunk_z: i32,
    pub columns: Vec<PixelizedColumn>,
    pub stats: PixelizedChunkAreaStats,
    pub config: PixelizeConfig,
}

impl PixelizedChunkArea {
    pub fn column(&self, x: u32, z: u32) -> Option<&PixelizedColumn> {
        if x >= self.width || z >= self.height {
            return None;
        }
        self.columns
            .get(z as usize * self.width as usize + x as usize)
    }

    pub fn chunk_width(&self) -> u32 {
        (self.max_chunk_x - self.min_chunk_x + 1) as u32
    }

    pub fn chunk_depth(&self) -> u32 {
        (self.max_chunk_z - self.min_chunk_z + 1) as u32
    }
}

pub fn generate_pixelized_chunk_area(
    macro_tile: &MacroFieldTile,
    config: PixelizeConfig,
) -> PixelizedChunkArea {
    validate_pixelize_tile(macro_tile);

    let mut columns = macro_tile
        .samples
        .par_iter()
        .enumerate()
        .map(|(index, sample)| {
            pixelized_column_from_macro_sample_at_position(
                sample,
                config,
                macro_tile.config.sample_position(index),
            )
        })
        .collect::<Vec<_>>();
    smooth_pixelized_river_concave_cusps(
        &mut columns,
        &macro_tile.samples,
        macro_tile.config.width as usize,
        macro_tile.config.height as usize,
        config,
    );
    let stats = pixelized_chunk_area_stats(&columns);
    let origin_world_x = world_block_from_sample_position(macro_tile.config.origin.x);
    let origin_world_z = world_block_from_sample_position(macro_tile.config.origin.z);
    let last_world_x = origin_world_x + macro_tile.config.width as i32 - 1;
    let last_world_z = origin_world_z + macro_tile.config.height as i32 - 1;

    PixelizedChunkArea {
        origin_world_x,
        origin_world_z,
        width: macro_tile.config.width,
        height: macro_tile.config.height,
        min_chunk_x: origin_world_x
            .div_euclid(CHUNK_EDGE_I32)
            .min(last_world_x.div_euclid(CHUNK_EDGE_I32)),
        max_chunk_x: origin_world_x
            .div_euclid(CHUNK_EDGE_I32)
            .max(last_world_x.div_euclid(CHUNK_EDGE_I32)),
        min_chunk_z: origin_world_z
            .div_euclid(CHUNK_EDGE_I32)
            .min(last_world_z.div_euclid(CHUNK_EDGE_I32)),
        max_chunk_z: origin_world_z
            .div_euclid(CHUNK_EDGE_I32)
            .max(last_world_z.div_euclid(CHUNK_EDGE_I32)),
        columns,
        stats,
        config,
    }
}

pub fn pixelized_column_from_macro_sample(
    sample: &MacroFieldSample,
    config: PixelizeConfig,
) -> PixelizedColumn {
    pixelized_column_from_macro_sample_at_position(sample, config, sample.position)
}

fn pixelized_column_from_macro_sample_at_position(
    sample: &MacroFieldSample,
    config: PixelizeConfig,
    position: super::graph::WorldPlanePoint,
) -> PixelizedColumn {
    validate_macro_sample(sample);
    assert_world_block_position(position.x);
    assert_world_block_position(position.z);
    assert!(
        (sample.position.x - position.x).abs() <= WORLD_BLOCK_EPSILON
            && (sample.position.z - position.z).abs() <= WORLD_BLOCK_EPSILON,
        "pixelize tile samples must match their row-major macro field positions"
    );

    let height_column = heightfield_column_from_sample(sample, config.heightfield);
    let world_x = world_block_from_sample_position(position.x);
    let world_z = world_block_from_sample_position(position.z);
    let (chunk, local) = world_to_chunk_local(WorldBlockCoord(world_x, 0, world_z));

    PixelizedColumn {
        world_x,
        world_z,
        chunk_x: chunk.0,
        chunk_z: chunk.2,
        local_x: local.x,
        local_z: local.z,
        surface_y: height_column.surface_y,
        water_y: height_column.water_y,
        terrain_kind: height_column.terrain_kind.into(),
        source_macro_elevation: sample.macro_elevation,
        source_combined_macro_height: sample.combined_macro_height,
        source_ocean_mask: sample.ocean_mask,
        source_lake_mask: sample.lake_mask,
        source_coast_mask: sample.coast_mask,
        source_dry_basin_mask: sample.dry_basin_mask,
        source_ridge_influence: sample.ridge_influence,
        source_terrain_ruggedness: sample
            .biome_context
            .map(|context| context.ruggedness.clamp(0.0, 1.0))
            .unwrap_or(0.0),
        source_river_valley_strength: sample.river_valley_strength,
        source_river_flow_hint: sample.river_flow_hint,
    }
}

fn smooth_pixelized_river_concave_cusps(
    columns: &mut [PixelizedColumn],
    samples: &[MacroFieldSample],
    width: usize,
    height: usize,
    config: PixelizeConfig,
) {
    if columns.len() != samples.len() || width == 0 || height == 0 {
        return;
    }

    for _ in 0..RIVER_CONCAVE_CUSP_MAX_PASSES {
        let promote = (0..columns.len())
            .filter(|&index| {
                is_pixelized_river_concave_cusp(columns, samples, width, height, index, config)
            })
            .collect::<Vec<_>>();
        if promote.is_empty() {
            break;
        }

        for index in promote {
            let mut promoted_sample = samples[index];
            promoted_sample.river_valley_strength = promoted_sample
                .river_valley_strength
                .max(config.heightfield.river_water_threshold);
            let mut promoted = pixelized_column_from_macro_sample_at_position(
                &promoted_sample,
                config,
                samples[index].position,
            );
            promoted.source_river_valley_strength = samples[index].river_valley_strength;
            promoted.source_river_flow_hint = samples[index].river_flow_hint;
            columns[index] = promoted;
        }
    }
}

fn is_pixelized_river_concave_cusp(
    columns: &[PixelizedColumn],
    samples: &[MacroFieldSample],
    width: usize,
    height: usize,
    index: usize,
    config: PixelizeConfig,
) -> bool {
    let column = columns[index];
    if column.water_y.is_some()
        || matches!(
            column.terrain_kind,
            PixelizedTerrainKind::Ocean
                | PixelizedTerrainKind::Lake
                | PixelizedTerrainKind::DryBasin
        )
    {
        return false;
    }

    let sample = samples[index];
    let threshold = config.heightfield.river_water_threshold;
    if sample.river_flow_hint <= 0.0
        || !sample.river_distance_blocks.is_finite()
        || sample.river_valley_strength < threshold * RIVER_CONCAVE_CUSP_MIN_STRENGTH_RATIO
        || sample.ocean_mask > 0.5
        || sample.lake_mask > 0.5
        || sample.dry_basin_mask > 0.5
    {
        return false;
    }

    let x = index % width;
    let z = index / width;
    let neighbor_count = river_neighbor_count(columns, width, height, x, z);
    neighbor_count >= RIVER_CONCAVE_CUSP_MIN_NEIGHBORS
        && has_orthogonal_river_support(columns, width, height, x, z)
}

fn river_neighbor_count(
    columns: &[PixelizedColumn],
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
            if neighbor_is_river(columns, width, height, x, z, dx, dz) {
                count += 1;
            }
        }
    }
    count
}

fn has_orthogonal_river_support(
    columns: &[PixelizedColumn],
    width: usize,
    height: usize,
    x: usize,
    z: usize,
) -> bool {
    let north = neighbor_is_river(columns, width, height, x, z, 0, -1);
    let south = neighbor_is_river(columns, width, height, x, z, 0, 1);
    let west = neighbor_is_river(columns, width, height, x, z, -1, 0);
    let east = neighbor_is_river(columns, width, height, x, z, 1, 0);

    (north || south) && (west || east)
}

fn neighbor_is_river(
    columns: &[PixelizedColumn],
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

    matches!(
        columns[nz * width + nx].terrain_kind,
        PixelizedTerrainKind::River
    )
}

fn validate_pixelize_tile(macro_tile: &MacroFieldTile) {
    assert_eq!(
        macro_tile.samples.len(),
        macro_tile.config.sample_count(),
        "pixelize requires a complete macro field sample grid"
    );
    assert!(
        macro_tile.config.width > 0 && macro_tile.config.height > 0,
        "pixelize requires a non-empty macro field tile"
    );
    assert!(
        (macro_tile.config.sample_spacing_blocks - 1.0).abs() <= WORLD_BLOCK_EPSILON,
        "pixelize requires one macro field sample per world block"
    );
    assert_world_block_position(macro_tile.config.origin.x);
    assert_world_block_position(macro_tile.config.origin.z);
    assert_eq!(
        world_block_from_sample_position(macro_tile.config.origin.x).rem_euclid(CHUNK_EDGE_I32),
        0,
        "pixelize macro field origin x must be chunk-aligned"
    );
    assert_eq!(
        world_block_from_sample_position(macro_tile.config.origin.z).rem_euclid(CHUNK_EDGE_I32),
        0,
        "pixelize macro field origin z must be chunk-aligned"
    );
    assert_eq!(
        macro_tile.config.width as i32 % CHUNK_EDGE_I32,
        0,
        "pixelize macro field width must cover whole chunks"
    );
    assert_eq!(
        macro_tile.config.height as i32 % CHUNK_EDGE_I32,
        0,
        "pixelize macro field height must cover whole chunks"
    );
}

fn validate_macro_sample(sample: &MacroFieldSample) {
    assert_world_block_position(sample.position.x);
    assert_world_block_position(sample.position.z);
    assert!(sample.macro_elevation.is_finite());
    assert!(sample.ocean_mask.is_finite());
    assert!(sample.coast_mask.is_finite());
    assert!(sample.lake_mask.is_finite());
    assert!(sample.dry_basin_mask.is_finite());
    assert!(sample.ridge_influence.is_finite());
    assert!(sample.river_valley_strength.is_finite());
    assert!(sample.river_distance_blocks.is_finite() || sample.river_distance_blocks.is_infinite());
    assert!(sample.river_flow_hint.is_finite());
    assert!(sample.combined_macro_height.is_finite());
}

fn world_block_from_sample_position(value: f32) -> i32 {
    assert_world_block_position(value);
    value.round() as i32
}

fn assert_world_block_position(value: f32) {
    assert!(value.is_finite());
    let rounded = value.round();
    assert!(
        (value - rounded).abs() <= WORLD_BLOCK_EPSILON,
        "pixelize samples must sit on integer world-block positions"
    );
    assert!(
        rounded >= i32::MIN as f32 && rounded <= i32::MAX as f32,
        "pixelize sample position must fit i32 world block coordinates"
    );
}

fn pixelized_chunk_area_stats(columns: &[PixelizedColumn]) -> PixelizedChunkAreaStats {
    if columns.is_empty() {
        return PixelizedChunkAreaStats::default();
    }

    let mut min_surface_y = i32::MAX;
    let mut max_surface_y = i32::MIN;
    let mut water_column_count = 0usize;
    let mut ocean_column_count = 0usize;
    let mut lake_column_count = 0usize;
    let mut river_hint_column_count = 0usize;
    let mut dry_basin_column_count = 0usize;
    let mut ridge_column_count = 0usize;

    for column in columns {
        min_surface_y = min_surface_y.min(column.surface_y);
        max_surface_y = max_surface_y.max(column.surface_y);
        if column.has_water_hint() {
            water_column_count += 1;
        }
        match column.terrain_kind {
            PixelizedTerrainKind::Ocean => ocean_column_count += 1,
            PixelizedTerrainKind::Lake => lake_column_count += 1,
            PixelizedTerrainKind::River => river_hint_column_count += 1,
            PixelizedTerrainKind::DryBasin => dry_basin_column_count += 1,
            PixelizedTerrainKind::Ridge => ridge_column_count += 1,
            PixelizedTerrainKind::Coast | PixelizedTerrainKind::Land => {}
        }
    }

    PixelizedChunkAreaStats {
        column_count: columns.len(),
        min_surface_y,
        max_surface_y,
        water_column_count,
        ocean_column_count,
        lake_column_count,
        river_hint_column_count,
        dry_basin_column_count,
        ridge_column_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::boundary::{generate_noisy_boundaries, BoundaryConfig};
    use crate::world::generation::graph::WorldPlanePoint;
    use crate::world::generation::graph::{
        generate_voronoi_graph_patch, graph_region_for_world_block, GraphRegionArea,
        VoronoiGraphConfig, VoronoiGraphPatchRequest, DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
        DEFAULT_SITE_SPACING_BLOCKS,
    };
    use crate::world::generation::heightfield::{
        DEFAULT_HEIGHTFIELD_MAX_BLOCKS, DEFAULT_HEIGHTFIELD_MIN_BLOCKS,
        DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD,
    };
    use crate::world::generation::hydrology::{solve_hydrology, HydrologyConfig};
    use crate::world::generation::macro_field::{
        generate_macro_field_tile, MacroFieldTileConfig, MacroFieldTileStats,
    };
    use crate::world::generation::macro_map::{
        generate_macro_map, MacroMapConfig, MacroSurfaceKind,
    };
    use crate::world::generation::river_plan::{build_river_plan, RiverPlanConfig};
    use crate::world::legacy::coord::CHUNK_EDGE;
    use crate::world::WorldMeta;
    use std::collections::HashMap;

    #[test]
    fn pixelized_chunk_area_generation_is_deterministic() {
        let macro_tile = test_macro_tile(-1, 2, 2, 1);
        let config = PixelizeConfig::default();

        let first = generate_pixelized_chunk_area(&macro_tile, config);
        let second = generate_pixelized_chunk_area(&macro_tile, config);

        assert_eq!(first, second);
    }

    #[test]
    fn pixelized_columns_keep_one_world_block_spacing() {
        let macro_tile = test_macro_tile(0, 0, 1, 1);
        let area = generate_pixelized_chunk_area(&macro_tile, PixelizeConfig::default());

        assert_eq!(area.width, CHUNK_EDGE as u32);
        assert_eq!(area.height, CHUNK_EDGE as u32);
        assert_eq!(area.columns.len(), CHUNK_EDGE * CHUNK_EDGE);
        assert_eq!(area.chunk_width(), 1);
        assert_eq!(area.chunk_depth(), 1);

        for z in 0..CHUNK_EDGE as u32 {
            for x in 0..CHUNK_EDGE as u32 {
                let column = area.column(x, z).expect("pixelized column");
                assert_eq!(column.world_x, x as i32);
                assert_eq!(column.world_z, z as i32);
                assert_eq!(column.local_x, x as u8);
                assert_eq!(column.local_z, z as u8);
            }
        }
    }

    #[test]
    fn pixelized_columns_use_euclidean_chunk_local_mapping() {
        let macro_tile = test_macro_tile(-1, -1, 2, 2);
        let area = generate_pixelized_chunk_area(&macro_tile, PixelizeConfig::default());

        let first = area.column(0, 0).expect("first column");
        assert_eq!(first.world_x, -CHUNK_EDGE_I32);
        assert_eq!(first.world_z, -CHUNK_EDGE_I32);
        assert_eq!(first.chunk_x, -1);
        assert_eq!(first.chunk_z, -1);
        assert_eq!(first.local_x, 0);
        assert_eq!(first.local_z, 0);

        let seam_left = area.column(31, 31).expect("negative chunk edge column");
        assert_eq!(seam_left.world_x, -1);
        assert_eq!(seam_left.world_z, -1);
        assert_eq!(seam_left.chunk_x, -1);
        assert_eq!(seam_left.chunk_z, -1);
        assert_eq!(seam_left.local_x, 31);
        assert_eq!(seam_left.local_z, 31);

        let origin = area.column(32, 32).expect("origin column");
        assert_eq!(origin.world_x, 0);
        assert_eq!(origin.world_z, 0);
        assert_eq!(origin.chunk_x, 0);
        assert_eq!(origin.chunk_z, 0);
        assert_eq!(origin.local_x, 0);
        assert_eq!(origin.local_z, 0);
    }

    #[test]
    fn pixelized_columns_have_finite_source_values_and_integer_heights() {
        let macro_tile = test_macro_tile(0, 0, 1, 1);
        let area = generate_pixelized_chunk_area(&macro_tile, PixelizeConfig::default());

        assert_eq!(area.stats.column_count, CHUNK_EDGE * CHUNK_EDGE);
        assert!(area.stats.min_surface_y <= area.stats.max_surface_y);
        assert!(area.columns.iter().all(|column| {
            column.source_macro_elevation.is_finite()
                && column.source_combined_macro_height.is_finite()
                && column.source_ocean_mask.is_finite()
                && column.source_lake_mask.is_finite()
                && column.source_coast_mask.is_finite()
                && column.source_dry_basin_mask.is_finite()
                && column.source_ridge_influence.is_finite()
                && column.source_terrain_ruggedness.is_finite()
                && column.source_river_valley_strength.is_finite()
                && column.source_river_flow_hint.is_finite()
                && column.surface_y >= DEFAULT_HEIGHTFIELD_MIN_BLOCKS as i32
                && column.surface_y <= DEFAULT_HEIGHTFIELD_MAX_BLOCKS as i32
                && column.water_y.is_none_or(|water_y| {
                    water_y >= DEFAULT_HEIGHTFIELD_MIN_BLOCKS as i32
                        && water_y <= DEFAULT_HEIGHTFIELD_MAX_BLOCKS as i32 + 1
                })
        }));
    }

    #[test]
    fn pixelized_water_hints_follow_ocean_lake_and_river_masks() {
        let ocean = pixelized_column_from_macro_sample(
            &sample(0, 0, -0.25, 1.0, 0.0, 0.0, 0.0),
            PixelizeConfig::default(),
        );
        let lake = pixelized_column_from_macro_sample(
            &sample(1, 0, -0.10, 0.0, 1.0, 0.0, 0.0),
            PixelizeConfig::default(),
        );
        let river = pixelized_column_from_macro_sample(
            &river_sample(2, 0, 0.10, 0.90),
            PixelizeConfig::default(),
        );
        let land = pixelized_column_from_macro_sample(
            &sample(3, 0, 0.10, 0.0, 0.0, 0.0, 0.0),
            PixelizeConfig::default(),
        );

        assert_eq!(ocean.terrain_kind, PixelizedTerrainKind::Ocean);
        assert_eq!(ocean.water_y, Some(0));
        assert_eq!(lake.terrain_kind, PixelizedTerrainKind::Lake);
        assert_eq!(lake.water_y, Some(1));
        assert_eq!(river.terrain_kind, PixelizedTerrainKind::River);
        assert!(river.water_y.is_some());
        assert_eq!(land.terrain_kind, PixelizedTerrainKind::Land);
        assert_eq!(land.water_y, None);
    }

    #[test]
    fn pixelized_area_fills_near_threshold_concave_river_cusp() {
        let mut macro_tile = test_macro_tile(0, 0, 1, 1);
        let threshold = PixelizeConfig::default().heightfield.river_water_threshold;
        let center = 16usize;
        for (x, z) in [
            (center, center - 1),
            (center, center + 1),
            (center - 1, center),
            (center + 1, center),
            (center - 1, center - 1),
            (center + 1, center - 1),
        ] {
            set_river_strength(&mut macro_tile, x, z, threshold, 0.72);
        }
        set_river_strength(
            &mut macro_tile,
            center,
            center,
            threshold * RIVER_CONCAVE_CUSP_MIN_STRENGTH_RATIO,
            0.72,
        );

        let single = pixelized_column_from_macro_sample(
            macro_tile
                .sample(center as u32, center as u32)
                .expect("cusp sample"),
            PixelizeConfig::default(),
        );
        let area = generate_pixelized_chunk_area(&macro_tile, PixelizeConfig::default());
        let smoothed = area
            .column(center as u32, center as u32)
            .expect("cusp column");

        assert_eq!(
            single.terrain_kind,
            PixelizedTerrainKind::Land,
            "single-column conversion keeps the hard threshold; area pixelize owns cusp cleanup"
        );
        assert_eq!(smoothed.terrain_kind, PixelizedTerrainKind::River);
        assert!(
            smoothed.water_y.is_some(),
            "near-threshold concave cusp should become a river water hint at pixelize stage"
        );
        assert!(
            smoothed.source_river_valley_strength < threshold,
            "source macro strength should remain diagnostic rather than being rewritten"
        );
    }

    #[test]
    fn pixelized_area_keeps_convex_river_corner_rounded() {
        let mut macro_tile = test_macro_tile(0, 0, 1, 1);
        let threshold = PixelizeConfig::default().heightfield.river_water_threshold;
        let outside = 16usize;
        for (x, z) in [
            (outside, outside - 1),
            (outside - 1, outside),
            (outside - 1, outside - 1),
        ] {
            set_river_strength(&mut macro_tile, x, z, threshold, 0.72);
        }
        set_river_strength(
            &mut macro_tile,
            outside,
            outside,
            threshold * RIVER_CONCAVE_CUSP_MIN_STRENGTH_RATIO,
            0.72,
        );

        let area = generate_pixelized_chunk_area(&macro_tile, PixelizeConfig::default());
        let convex_corner = area
            .column(outside as u32, outside as u32)
            .expect("convex outside corner");

        assert_eq!(
            convex_corner.terrain_kind,
            PixelizedTerrainKind::Land,
            "convex outside corners should not be squared off by concave cusp cleanup"
        );
        assert_eq!(convex_corner.water_y, None);
    }

    #[test]
    fn pixelized_column_preserves_source_ruggedness() {
        let mut sample = sample(0, 0, 0.10, 0.0, 0.0, 0.0, 0.0);
        sample.biome_context = Some(test_biome_context(0.73));

        let column = pixelized_column_from_macro_sample(&sample, PixelizeConfig::default());

        assert_eq!(column.source_terrain_ruggedness, 0.73);
    }

    #[test]
    #[ignore = "diagnostic helper for seed 42 r4 pixelized vertical discontinuities"]
    fn diagnose_seed42_r4_adjacent_surface_deltas() {
        let (area, nearest_sites, site_kind) = seed42_r4_pixelized_area_with_source();
        let width = area.width as usize;
        let height = area.height as usize;
        let mut pairs = Vec::new();
        for z in 0..height {
            for x in 0..width {
                let index = z * width + x;
                if x + 1 < width {
                    pairs.push(diagnostic_pair(
                        &area,
                        &nearest_sites,
                        &site_kind,
                        index,
                        index + 1,
                        "E",
                    ));
                }
                if z + 1 < height {
                    pairs.push(diagnostic_pair(
                        &area,
                        &nearest_sites,
                        &site_kind,
                        index,
                        index + width,
                        "S",
                    ));
                }
            }
        }
        pairs.sort_by(|left, right| {
            right
                .surface_delta
                .cmp(&left.surface_delta)
                .then_with(|| right.combined_delta.total_cmp(&left.combined_delta))
        });

        eprintln!(
            "seed=42 center_chunk=(-70,0) r=4 columns={}x{} chunk_range={}..{},{}..{} top_adjacent_surface_y_deltas:",
            area.width,
            area.height,
            area.min_chunk_x,
            area.max_chunk_x,
            area.min_chunk_z,
            area.max_chunk_z
        );
        for (rank, pair) in pairs.iter().take(40).enumerate() {
            eprintln!(
                "#{:02} dir={} surface_delta={} combined_delta={:.6} macro_delta={:.6} crosses water={} ocean={} lake={} river={} boundary={} contour={} left={} right={}",
                rank + 1,
                pair.direction,
                pair.surface_delta,
                pair.combined_delta,
                pair.macro_delta,
                pair.crosses_water,
                pair.crosses_ocean,
                pair.crosses_lake,
                pair.crosses_river,
                pair.crosses_boundary,
                pair.crosses_contour,
                pair.left,
                pair.right
            );
        }
    }

    #[test]
    fn seed42_r4_same_site_inland_columns_do_not_jump_on_straight_interpolation_boundary() {
        let (area, nearest_sites, _site_kind) = seed42_r4_broad_pixelized_area_with_source();
        let left = seed42_r4_column_index(&area, -1960, -92);
        let right = seed42_r4_column_index(&area, -1959, -92);
        let left_column = area.columns[left];
        let right_column = area.columns[right];

        assert_eq!(
            nearest_sites[left], nearest_sites[right],
            "regression pair should stay within the same owner site, not a Voronoi boundary"
        );
        assert!(left_column.water_y.is_none() && right_column.water_y.is_none());
        assert!(left_column.source_ocean_mask <= 0.5 && right_column.source_ocean_mask <= 0.5);
        assert!(left_column.source_lake_mask <= 0.5 && right_column.source_lake_mask <= 0.5);
        assert!(
            (left_column.surface_y - right_column.surface_y).abs() <= 1,
            "same-site inland adjacent columns should not jump more than one block: left={:?} right={:?}",
            left_column,
            right_column
        );
        assert!(
            (left_column.source_combined_macro_height - right_column.source_combined_macro_height)
                .abs()
                <= 1.0 / DEFAULT_HEIGHTFIELD_MAX_BLOCKS,
            "macro scalar should not contain a hidden >1 block discontinuity: left={} right={}",
            left_column.source_combined_macro_height,
            right_column.source_combined_macro_height
        );
    }

    #[test]
    #[ignore = "exact supplied pixelize-preview target; expensive stage-11 regression"]
    fn seed42_cx64_cz24_r12_pixelize_has_no_dense_concave_river_cusp_candidates() {
        let (macro_tile, area) = seed42_pixelized_macro_tile_and_area(-64, -24, 12);
        let remaining = (0..area.columns.len())
            .filter(|&index| {
                is_pixelized_river_concave_cusp(
                    &area.columns,
                    &macro_tile.samples,
                    area.width as usize,
                    area.height as usize,
                    index,
                    area.config,
                )
            })
            .count();

        assert!(
            area.stats.river_hint_column_count > 0,
            "target preview footprint should include river columns"
        );
        assert_eq!(
            remaining, 0,
            "pixelize river mask should not leave dense near-threshold concave cusp candidates"
        );
    }

    #[test]
    #[ignore = "diagnostic helper for coast y gradient validation"]
    fn diagnose_seed42_r4_coast_y_gradient_spans() {
        let (area, nearest_sites, site_kind) = seed42_r4_broad_pixelized_area_with_source();
        let summaries = coast_gradient_summaries(&area);

        for summary in &summaries {
            eprintln!(
                "seed=42 class={} pairs={} max_surface={} p95_surface={} max_combined={:.6} p95_combined={:.6} max_macro={:.6} p95_macro={:.6}",
                summary.label,
                summary.pair_count,
                summary.max_surface_delta,
                summary.p95_surface_delta,
                summary.max_combined_delta,
                summary.p95_combined_delta,
                summary.max_macro_delta,
                summary.p95_macro_delta
            );
        }

        for summary in summaries.iter().filter(|summary| summary.label != "inland") {
            if let Some(pair) = summary.top_pairs.first().copied() {
                eprintln!(
                    "  {}-peak dir={} surface_delta={} combined_delta={:.6} macro_delta={:.6} left={} right={}",
                    summary.label,
                    pair.direction,
                    pair.surface_delta,
                    pair.combined_delta,
                    pair.macro_delta,
                    diagnostic_column(
                        area.columns[pair.left_index],
                        nearest_sites[pair.left_index],
                        &site_kind,
                    ),
                    diagnostic_column(
                        area.columns[pair.right_index],
                        nearest_sites[pair.right_index],
                        &site_kind,
                    )
                );
                print_transect_window(&area, pair, 10);
            }
        }

        if let Some(summary) = summaries.iter().find(|summary| summary.label == "inland") {
            for pair in summary.top_pairs.iter().take(2).copied() {
                eprintln!(
                    "  inland-span dir={} surface_delta={} combined_delta={:.6} macro_delta={:.6} left={} right={}",
                    pair.direction,
                    pair.surface_delta,
                    pair.combined_delta,
                    pair.macro_delta,
                    diagnostic_column(
                        area.columns[pair.left_index],
                        nearest_sites[pair.left_index],
                        &site_kind,
                    ),
                    diagnostic_column(
                        area.columns[pair.right_index],
                        nearest_sites[pair.right_index],
                        &site_kind,
                    )
                );
                print_transect_window(&area, pair, 10);
            }
        }
    }

    fn seed42_r4_column_index(area: &PixelizedChunkArea, world_x: i32, world_z: i32) -> usize {
        let local_x = world_x - area.origin_world_x;
        let local_z = world_z - area.origin_world_z;
        assert!(
            local_x >= 0 && local_z >= 0,
            "world coordinate is before pixelized area origin"
        );
        let local_x = local_x as u32;
        let local_z = local_z as u32;
        assert!(
            local_x < area.width && local_z < area.height,
            "world coordinate is outside pixelized area"
        );
        local_z as usize * area.width as usize + local_x as usize
    }

    #[derive(Debug)]
    struct DiagnosticPair {
        direction: &'static str,
        surface_delta: i32,
        combined_delta: f32,
        macro_delta: f32,
        crosses_water: bool,
        crosses_ocean: bool,
        crosses_lake: bool,
        crosses_river: bool,
        crosses_boundary: bool,
        crosses_contour: bool,
        left: String,
        right: String,
    }

    fn diagnostic_pair(
        area: &PixelizedChunkArea,
        nearest_sites: &[Option<crate::world::generation::graph::VoronoiSiteId>],
        site_kind: &HashMap<crate::world::generation::graph::VoronoiSiteId, MacroSurfaceKind>,
        left_index: usize,
        right_index: usize,
        direction: &'static str,
    ) -> DiagnosticPair {
        let left = area.columns[left_index];
        let right = area.columns[right_index];
        DiagnosticPair {
            direction,
            surface_delta: (left.surface_y - right.surface_y).abs(),
            combined_delta: (left.source_combined_macro_height
                - right.source_combined_macro_height)
                .abs(),
            macro_delta: (left.source_macro_elevation - right.source_macro_elevation).abs(),
            crosses_water: left.water_y.is_some() != right.water_y.is_some(),
            crosses_ocean: (left.source_ocean_mask > 0.5) != (right.source_ocean_mask > 0.5),
            crosses_lake: (left.source_lake_mask > 0.5) != (right.source_lake_mask > 0.5),
            crosses_river: matches!(left.terrain_kind, PixelizedTerrainKind::River)
                != matches!(right.terrain_kind, PixelizedTerrainKind::River),
            crosses_boundary: nearest_sites[left_index] != nearest_sites[right_index],
            crosses_contour: left.surface_y != right.surface_y,
            left: diagnostic_column(left, nearest_sites[left_index], site_kind),
            right: diagnostic_column(right, nearest_sites[right_index], site_kind),
        }
    }

    fn diagnostic_column(
        column: PixelizedColumn,
        nearest_site: Option<crate::world::generation::graph::VoronoiSiteId>,
        site_kind: &HashMap<crate::world::generation::graph::VoronoiSiteId, MacroSurfaceKind>,
    ) -> String {
        format!(
            "world=({},{}),chunk=({},{}),local=({},{}),surface_y={},water_y={:?},terrain={:?},kind={:?},site={:?},macro={:.6},combined={:.6},ocean={:.3},lake={:.3},coast={:.3},dry={:.3},river={:.3}/{:.3},ridge={:.3}",
            column.world_x,
            column.world_z,
            column.chunk_x,
            column.chunk_z,
            column.local_x,
            column.local_z,
            column.surface_y,
            column.water_y,
            column.terrain_kind,
            nearest_site.and_then(|site| site_kind.get(&site).copied()),
            nearest_site,
            column.source_macro_elevation,
            column.source_combined_macro_height,
            column.source_ocean_mask,
            column.source_lake_mask,
            column.source_coast_mask,
            column.source_dry_basin_mask,
            column.source_river_valley_strength,
            column.source_river_flow_hint,
            column.source_ridge_influence
        )
    }

    #[derive(Debug, Clone, Copy)]
    struct GradientPair {
        direction: &'static str,
        left_index: usize,
        right_index: usize,
        surface_delta: i32,
        combined_delta: f32,
        macro_delta: f32,
    }

    #[derive(Debug, Default)]
    struct GradientSummary {
        label: &'static str,
        pair_count: usize,
        max_surface_delta: i32,
        p95_surface_delta: i32,
        max_combined_delta: f32,
        p95_combined_delta: f32,
        max_macro_delta: f32,
        p95_macro_delta: f32,
        top_pairs: Vec<GradientPair>,
    }

    fn coast_gradient_summaries(area: &PixelizedChunkArea) -> Vec<GradientSummary> {
        let width = area.width as usize;
        let height = area.height as usize;
        let mut coast_surface = Vec::new();
        let mut coast_combined = Vec::new();
        let mut coast_macro = Vec::new();
        let mut coast_top = Vec::new();
        let mut inland_surface = Vec::new();
        let mut inland_combined = Vec::new();
        let mut inland_macro = Vec::new();
        let mut inland_top = Vec::new();

        for z in 0..height {
            for x in 0..width {
                let index = z * width + x;
                if x + 1 < width {
                    classify_gradient_pair(
                        area,
                        index,
                        index + 1,
                        "E",
                        &mut coast_surface,
                        &mut coast_combined,
                        &mut coast_macro,
                        &mut coast_top,
                        &mut inland_surface,
                        &mut inland_combined,
                        &mut inland_macro,
                        &mut inland_top,
                    );
                }
                if z + 1 < height {
                    classify_gradient_pair(
                        area,
                        index,
                        index + width,
                        "S",
                        &mut coast_surface,
                        &mut coast_combined,
                        &mut coast_macro,
                        &mut coast_top,
                        &mut inland_surface,
                        &mut inland_combined,
                        &mut inland_macro,
                        &mut inland_top,
                    );
                }
            }
        }

        vec![
            gradient_summary(
                "coast",
                coast_surface,
                coast_combined,
                coast_macro,
                coast_top,
            ),
            gradient_summary(
                "inland",
                inland_surface,
                inland_combined,
                inland_macro,
                inland_top,
            ),
        ]
    }

    #[allow(clippy::too_many_arguments)]
    fn classify_gradient_pair(
        area: &PixelizedChunkArea,
        left_index: usize,
        right_index: usize,
        direction: &'static str,
        coast_surface: &mut Vec<i32>,
        coast_combined: &mut Vec<f32>,
        coast_macro: &mut Vec<f32>,
        coast_top: &mut Vec<GradientPair>,
        inland_surface: &mut Vec<i32>,
        inland_combined: &mut Vec<f32>,
        inland_macro: &mut Vec<f32>,
        inland_top: &mut Vec<GradientPair>,
    ) {
        let left = area.columns[left_index];
        let right = area.columns[right_index];
        let pair = GradientPair {
            direction,
            left_index,
            right_index,
            surface_delta: (left.surface_y - right.surface_y).abs(),
            combined_delta: (left.source_combined_macro_height
                - right.source_combined_macro_height)
                .abs(),
            macro_delta: (left.source_macro_elevation - right.source_macro_elevation).abs(),
        };
        let is_coast = left.source_coast_mask > 0.05 || right.source_coast_mask > 0.05;
        let is_inland = left.source_coast_mask <= 0.05
            && right.source_coast_mask <= 0.05
            && left.source_ocean_mask <= 0.5
            && right.source_ocean_mask <= 0.5
            && left.source_lake_mask <= 0.5
            && right.source_lake_mask <= 0.5
            && left.source_dry_basin_mask <= 0.5
            && right.source_dry_basin_mask <= 0.5
            && left.water_y.is_none()
            && right.water_y.is_none();

        if is_coast {
            coast_surface.push(pair.surface_delta);
            coast_combined.push(pair.combined_delta);
            coast_macro.push(pair.macro_delta);
            coast_top.push(pair);
            coast_top.sort_by(|a, b| b.surface_delta.cmp(&a.surface_delta));
            coast_top.truncate(3);
        }
        if is_inland {
            inland_surface.push(pair.surface_delta);
            inland_combined.push(pair.combined_delta);
            inland_macro.push(pair.macro_delta);
            inland_top.push(pair);
            inland_top.sort_by(|a, b| b.surface_delta.cmp(&a.surface_delta));
            inland_top.truncate(3);
        }
    }

    fn gradient_summary(
        label: &'static str,
        mut surface: Vec<i32>,
        mut combined: Vec<f32>,
        mut macro_delta: Vec<f32>,
        top_pairs: Vec<GradientPair>,
    ) -> GradientSummary {
        surface.sort_unstable();
        combined.sort_by(f32::total_cmp);
        macro_delta.sort_by(f32::total_cmp);
        let pair_count = surface.len();
        let p95_index = |len: usize| ((len.saturating_sub(1)) as f32 * 0.95).round() as usize;
        GradientSummary {
            label,
            pair_count,
            max_surface_delta: surface.last().copied().unwrap_or_default(),
            p95_surface_delta: surface
                .get(p95_index(surface.len()))
                .copied()
                .unwrap_or_default(),
            max_combined_delta: combined.last().copied().unwrap_or_default(),
            p95_combined_delta: combined
                .get(p95_index(combined.len()))
                .copied()
                .unwrap_or_default(),
            max_macro_delta: macro_delta.last().copied().unwrap_or_default(),
            p95_macro_delta: macro_delta
                .get(p95_index(macro_delta.len()))
                .copied()
                .unwrap_or_default(),
            top_pairs,
        }
    }

    fn print_transect_window(area: &PixelizedChunkArea, pair: GradientPair, half_width: usize) {
        let width = area.width as usize;
        let height = area.height as usize;
        match pair.direction {
            "E" => {
                let z = pair.left_index / width;
                let x = pair.left_index % width;
                let start = x.saturating_sub(half_width);
                let end = (x + half_width + 1).min(width);
                let values = (start..end)
                    .map(|current_x| {
                        let column = area.columns[z * width + current_x];
                        format!(
                            "x={} y={} comb={:.6} macro={:.6} coast={:.3}",
                            current_x,
                            column.surface_y,
                            column.source_combined_macro_height,
                            column.source_macro_elevation,
                            column.source_coast_mask
                        )
                    })
                    .collect::<Vec<_>>();
                eprintln!(
                    "    transect z={} x={}..{} | {}",
                    z,
                    start,
                    end.saturating_sub(1),
                    values.join(" ; ")
                );
            }
            "S" => {
                let x = pair.left_index % width;
                let y = pair.left_index / width;
                let start = y.saturating_sub(half_width);
                let end = (y + half_width + 1).min(height);
                let values = (start..end)
                    .map(|current_y| {
                        let column = area.columns[current_y * width + x];
                        format!(
                            "z={} y={} comb={:.6} macro={:.6} coast={:.3}",
                            current_y,
                            column.surface_y,
                            column.source_combined_macro_height,
                            column.source_macro_elevation,
                            column.source_coast_mask
                        )
                    })
                    .collect::<Vec<_>>();
                eprintln!(
                    "    transect x={} z={}..{} | {}",
                    x,
                    start,
                    end.saturating_sub(1),
                    values.join(" ; ")
                );
            }
            _ => {}
        }
    }

    fn seed42_r4_pixelized_area_with_source() -> (
        PixelizedChunkArea,
        Vec<Option<crate::world::generation::graph::VoronoiSiteId>>,
        HashMap<crate::world::generation::graph::VoronoiSiteId, MacroSurfaceKind>,
    ) {
        seed42_pixelized_area_with_source(-70, 0, 4)
    }

    fn seed42_pixelized_area_with_source(
        center_chunk_x: i32,
        center_chunk_z: i32,
        radius: i32,
    ) -> (
        PixelizedChunkArea,
        Vec<Option<crate::world::generation::graph::VoronoiSiteId>>,
        HashMap<crate::world::generation::graph::VoronoiSiteId, MacroSurfaceKind>,
    ) {
        let (macro_tile, area, macro_map) =
            seed42_pixelized_macro_tile_area_and_macro_map(center_chunk_x, center_chunk_z, radius);
        let site_kind = macro_map
            .sites
            .iter()
            .map(|site| (site.id, site.surface_kind))
            .collect::<HashMap<_, _>>();
        let nearest_sites = macro_tile
            .samples
            .iter()
            .map(|sample| sample.nearest_site)
            .collect::<Vec<_>>();

        (area, nearest_sites, site_kind)
    }

    fn seed42_pixelized_macro_tile_and_area(
        center_chunk_x: i32,
        center_chunk_z: i32,
        radius: i32,
    ) -> (MacroFieldTile, PixelizedChunkArea) {
        let (macro_tile, area, _macro_map) =
            seed42_pixelized_macro_tile_area_and_macro_map(center_chunk_x, center_chunk_z, radius);
        (macro_tile, area)
    }

    fn seed42_pixelized_macro_tile_area_and_macro_map(
        center_chunk_x: i32,
        center_chunk_z: i32,
        radius: i32,
    ) -> (
        MacroFieldTile,
        PixelizedChunkArea,
        crate::world::generation::macro_map::GraphMacroMap,
    ) {
        let (patch, macro_map) = seed42_macro_inputs(center_chunk_x, center_chunk_z, radius);
        let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        let river_plan =
            build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let boundary = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(42, 11));
        let min_chunk_x = center_chunk_x - radius;
        let min_chunk_z = center_chunk_z - radius;
        let chunk_count = radius.saturating_mul(2).saturating_add(1) as u32;
        let config = MacroFieldTileConfig::new(
            (min_chunk_x * CHUNK_EDGE_I32) as f32,
            (min_chunk_z * CHUNK_EDGE_I32) as f32,
            chunk_count * CHUNK_EDGE as u32,
            chunk_count * CHUNK_EDGE as u32,
            1.0,
        );
        let macro_tile =
            generate_macro_field_tile(&patch, &macro_map, &river_plan, &boundary, config);
        let area = generate_pixelized_chunk_area(&macro_tile, PixelizeConfig::default());

        (macro_tile, area, macro_map)
    }

    fn seed42_r4_broad_pixelized_area_with_source() -> (
        PixelizedChunkArea,
        Vec<Option<crate::world::generation::graph::VoronoiSiteId>>,
        HashMap<crate::world::generation::graph::VoronoiSiteId, MacroSurfaceKind>,
    ) {
        let (patch, macro_map) = seed42_r4_macro_inputs();
        let site_kind = macro_map
            .sites
            .iter()
            .map(|site| (site.id, site.surface_kind))
            .collect::<HashMap<_, _>>();
        let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
        let river_plan =
            build_river_plan(&patch, &macro_map, &hydrology, RiverPlanConfig::default());
        let boundary = generate_noisy_boundaries(&patch, &macro_map, BoundaryConfig::new(42, 11));
        let min_chunk_x = -78;
        let min_chunk_z = -8;
        let config = MacroFieldTileConfig::new(
            (min_chunk_x * CHUNK_EDGE_I32) as f32,
            (min_chunk_z * CHUNK_EDGE_I32) as f32,
            (17 * CHUNK_EDGE) as u32,
            (17 * CHUNK_EDGE) as u32,
            1.0,
        );
        let macro_tile =
            generate_macro_field_tile(&patch, &macro_map, &river_plan, &boundary, config);
        let nearest_sites = macro_tile
            .samples
            .iter()
            .map(|sample| sample.nearest_site)
            .collect::<Vec<_>>();
        let area = generate_pixelized_chunk_area(&macro_tile, PixelizeConfig::default());

        (area, nearest_sites, site_kind)
    }

    fn seed42_r4_macro_inputs() -> (
        crate::world::generation::graph::VoronoiGraphPatch,
        crate::world::generation::macro_map::GraphMacroMap,
    ) {
        seed42_macro_inputs(-70, 0, 4)
    }

    fn seed42_macro_inputs(
        center_chunk_x: i32,
        center_chunk_z: i32,
        radius: i32,
    ) -> (
        crate::world::generation::graph::VoronoiGraphPatch,
        crate::world::generation::macro_map::GraphMacroMap,
    ) {
        let meta = WorldMeta::new(42);
        let min_x = (center_chunk_x - radius) * CHUNK_EDGE_I32;
        let max_x = ((center_chunk_x + radius + 1) * CHUNK_EDGE_I32).saturating_sub(1);
        let min_z = (center_chunk_z - radius) * CHUNK_EDGE_I32;
        let max_z = ((center_chunk_z + radius + 1) * CHUNK_EDGE_I32).saturating_sub(1);
        let area = GraphRegionArea::new(
            graph_region_for_world_block(min_x, min_z, DEFAULT_GRAPH_REGION_SIZE_BLOCKS),
            graph_region_for_world_block(max_x, max_z, DEFAULT_GRAPH_REGION_SIZE_BLOCKS),
        )
        .expect("valid seed42 r4 graph area");
        let center_world_x = center_chunk_x * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2;
        let center_world_z = center_chunk_z * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 / 2;
        let center_region = graph_region_for_world_block(
            center_world_x,
            center_world_z,
            DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
        );
        let padding_regions = (center_region.x - area.min.x)
            .abs()
            .max((area.max.x - center_region.x).abs())
            .max((center_region.z - area.min.z).abs())
            .max((area.max.z - center_region.z).abs())
            .saturating_add(1) as u32;
        let patch = generate_voronoi_graph_patch(VoronoiGraphPatchRequest::new(
            VoronoiGraphConfig {
                seed: meta.seed,
                generator_version: meta.generator_version,
                region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
                site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
                padding_regions,
            },
            center_world_x,
            center_world_z,
        ));
        let macro_map = generate_macro_map(&patch, MacroMapConfig::new(42, 11));

        (patch, macro_map)
    }

    fn test_macro_tile(
        origin_chunk_x: i32,
        origin_chunk_z: i32,
        chunk_width: u32,
        chunk_depth: u32,
    ) -> MacroFieldTile {
        let width = chunk_width * CHUNK_EDGE as u32;
        let height = chunk_depth * CHUNK_EDGE as u32;
        let origin_x = origin_chunk_x * CHUNK_EDGE_I32;
        let origin_z = origin_chunk_z * CHUNK_EDGE_I32;
        let config =
            MacroFieldTileConfig::new(origin_x as f32, origin_z as f32, width, height, 1.0);
        let samples = (0..config.sample_count())
            .map(|index| {
                let x = index % width as usize;
                let z = index / width as usize;
                let world_x = origin_x + x as i32;
                let world_z = origin_z + z as i32;
                let normalized_height = (((x + z) % 9) as f32 - 3.0) / 32.0;
                sample(world_x, world_z, normalized_height, 0.0, 0.0, 0.0, 0.0)
            })
            .collect::<Vec<_>>();

        MacroFieldTile {
            config,
            samples,
            stats: MacroFieldTileStats::default(),
        }
    }

    fn sample(
        world_x: i32,
        world_z: i32,
        combined_macro_height: f32,
        ocean_mask: f32,
        lake_mask: f32,
        dry_basin_mask: f32,
        ridge_influence: f32,
    ) -> MacroFieldSample {
        MacroFieldSample {
            position: WorldPlanePoint::new(world_x as f32, world_z as f32),
            nearest_site: None,
            surface_kind: None,
            biome_context: None,
            biome: None,
            macro_elevation: combined_macro_height,
            ocean_mask,
            coast_mask: 0.0,
            lake_mask,
            dry_basin_mask,
            ridge_influence,
            river_valley_strength: 0.0,
            river_distance_blocks: f32::INFINITY,
            river_flow_hint: 0.0,
            river_bed_depth_hint: 0.0,
            river_bank_roughness_hint: 0.0,
            river_gravel_hint: 0.0,
            river_cutbank_hint: 0.0,
            combined_macro_height,
        }
    }

    fn test_biome_context(ruggedness: f32) -> crate::world::generation::biome::GraphBiomeContext {
        crate::world::generation::biome::GraphBiomeContext {
            temperature: 0.5,
            hydration: 0.5,
            elevation: 0.0,
            continentality: 0.0,
            coastness: 0.0,
            mountainness: 0.0,
            ruggedness,
            water_role: crate::world::generation::biome::GraphBiomeWaterRole::Land,
        }
    }

    fn river_sample(
        world_x: i32,
        world_z: i32,
        combined_macro_height: f32,
        river_flow_hint: f32,
    ) -> MacroFieldSample {
        let mut sample = sample(world_x, world_z, combined_macro_height, 0.0, 0.0, 0.0, 0.0);
        sample.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD;
        sample.river_flow_hint = river_flow_hint;
        sample
    }

    fn set_river_strength(
        macro_tile: &mut MacroFieldTile,
        x: usize,
        z: usize,
        river_valley_strength: f32,
        river_flow_hint: f32,
    ) {
        let index = z * macro_tile.config.width as usize + x;
        let sample = macro_tile.samples.get_mut(index).expect("test sample");
        sample.river_valley_strength = river_valley_strength;
        sample.river_flow_hint = river_flow_hint;
        sample.river_distance_blocks = 1.0;
        sample.river_bed_depth_hint = 0.45;
        sample.river_bank_roughness_hint = 0.35;
        sample.river_gravel_hint = 0.25;
    }
}
