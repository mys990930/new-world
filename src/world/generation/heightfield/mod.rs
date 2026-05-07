use rayon::prelude::*;

use super::graph::WorldPlanePoint;
use super::macro_field::{MacroFieldSample, MacroFieldTile};

pub const DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS: f32 = 0.0;
pub const DEFAULT_HEIGHTFIELD_MIN_BLOCKS: f32 = -48.0;
pub const DEFAULT_HEIGHTFIELD_MAX_BLOCKS: f32 = 160.0;
pub const DEFAULT_HEIGHTFIELD_NORMALIZED_MIN: f32 = -0.75;
pub const DEFAULT_HEIGHTFIELD_NORMALIZED_MAX: f32 = 1.25;
pub const DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD: f32 = 0.72;
pub const DEFAULT_HEIGHTFIELD_OCEAN_BED_BLOCKS: f32 = -12.0;
pub const DEFAULT_HEIGHTFIELD_LAKE_BED_BLOCKS: f32 = -2.0;
pub const DEFAULT_HEIGHTFIELD_SHORE_RAMP_BLOCKS: f32 = 128.0;
pub const DEFAULT_HEIGHTFIELD_SHORE_MIN_LAND_BLOCKS: f32 = 1.0;
pub const DEFAULT_HEIGHTFIELD_CONTOUR_STEP_BLOCKS: f32 = 1.0;
pub const DEFAULT_HEIGHTFIELD_CONTOUR_BAND_SMOOTHING: f32 = 0.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightfieldContourConfig {
    pub step_blocks: f32,
    pub band_smoothing: f32,
}

impl Default for HeightfieldContourConfig {
    fn default() -> Self {
        Self {
            step_blocks: DEFAULT_HEIGHTFIELD_CONTOUR_STEP_BLOCKS,
            band_smoothing: DEFAULT_HEIGHTFIELD_CONTOUR_BAND_SMOOTHING,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightfieldConfig {
    pub sea_level_blocks: f32,
    pub min_height_blocks: f32,
    pub max_height_blocks: f32,
    pub normalized_min_height: f32,
    pub normalized_max_height: f32,
    pub river_water_threshold: f32,
    pub ocean_bed_blocks: f32,
    pub lake_bed_blocks: f32,
    pub shore_ramp_blocks: f32,
    pub shore_min_land_blocks: f32,
    pub contour: HeightfieldContourConfig,
}

impl Default for HeightfieldConfig {
    fn default() -> Self {
        Self {
            sea_level_blocks: DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
            min_height_blocks: DEFAULT_HEIGHTFIELD_MIN_BLOCKS,
            max_height_blocks: DEFAULT_HEIGHTFIELD_MAX_BLOCKS,
            normalized_min_height: DEFAULT_HEIGHTFIELD_NORMALIZED_MIN,
            normalized_max_height: DEFAULT_HEIGHTFIELD_NORMALIZED_MAX,
            river_water_threshold: DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD,
            ocean_bed_blocks: DEFAULT_HEIGHTFIELD_OCEAN_BED_BLOCKS,
            lake_bed_blocks: DEFAULT_HEIGHTFIELD_LAKE_BED_BLOCKS,
            shore_ramp_blocks: DEFAULT_HEIGHTFIELD_SHORE_RAMP_BLOCKS,
            shore_min_land_blocks: DEFAULT_HEIGHTFIELD_SHORE_MIN_LAND_BLOCKS,
            contour: HeightfieldContourConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightfieldTerrainKind {
    Ocean,
    Lake,
    River,
    DryBasin,
    Ridge,
    Coast,
    Land,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightfieldColumn {
    pub position: WorldPlanePoint,
    pub raw_surface_height_blocks: f32,
    pub contour_guided_surface_height_blocks: f32,
    pub constrained_surface_height_blocks: f32,
    pub surface_height_blocks: f32,
    pub surface_y: i32,
    pub water_level_blocks: Option<f32>,
    pub water_y: Option<i32>,
    pub terrain_kind: HeightfieldTerrainKind,
    pub macro_elevation: f32,
    pub combined_macro_height: f32,
    pub ocean_mask: f32,
    pub lake_mask: f32,
    pub dry_basin_mask: f32,
    pub coast_mask: f32,
    pub ridge_influence: f32,
    pub river_valley_strength: f32,
    pub river_flow_hint: f32,
    pub meso_delta_blocks: f32,
    pub micro_relief_blocks: f32,
}

impl HeightfieldColumn {
    pub fn has_water_column(&self) -> bool {
        self.water_level_blocks
            .is_some_and(|water| water > self.surface_height_blocks)
    }

    pub fn visible_surface_height_blocks(&self) -> f32 {
        self.water_level_blocks
            .filter(|water| *water > self.surface_height_blocks)
            .unwrap_or(self.surface_height_blocks)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HeightfieldTileStats {
    pub column_count: usize,
    pub min_surface_height_blocks: f32,
    pub max_surface_height_blocks: f32,
    pub average_surface_height_blocks: f32,
    pub contour_step_blocks: f32,
    pub contour_band_smoothing: f32,
    pub max_raw_neighbor_delta_blocks: f32,
    pub max_contour_guided_neighbor_delta_blocks: f32,
    pub max_constrained_neighbor_delta_blocks: f32,
    pub max_snapped_neighbor_delta_blocks: f32,
    pub max_visible_neighbor_delta_blocks: f32,
    pub max_shore_visible_neighbor_delta_blocks: f32,
    pub water_column_count: usize,
    pub ocean_column_count: usize,
    pub lake_column_count: usize,
    pub river_hint_column_count: usize,
    pub dry_basin_column_count: usize,
    pub ridge_column_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeightfieldTile {
    pub width: u32,
    pub height: u32,
    pub sample_spacing_blocks: f32,
    pub columns: Vec<HeightfieldColumn>,
    pub stats: HeightfieldTileStats,
    pub config: HeightfieldConfig,
}

impl HeightfieldTile {
    pub fn column(&self, x: u32, z: u32) -> Option<&HeightfieldColumn> {
        if x >= self.width || z >= self.height {
            return None;
        }
        self.columns
            .get(z as usize * self.width as usize + x as usize)
    }
}

pub fn generate_heightfield_tile(
    macro_tile: &MacroFieldTile,
    config: HeightfieldConfig,
) -> HeightfieldTile {
    validate_heightfield_config(config);
    let mut columns = macro_tile
        .samples
        .par_iter()
        .map(|sample| heightfield_column_from_sample(sample, config))
        .collect::<Vec<_>>();
    apply_neighbor_shoreline_continuity(
        &mut columns,
        macro_tile.config.width as usize,
        macro_tile.config.height as usize,
        macro_tile.config.sample_spacing_blocks,
        config,
    );
    let stats = heightfield_stats(&columns, config);

    HeightfieldTile {
        width: macro_tile.config.width,
        height: macro_tile.config.height,
        sample_spacing_blocks: macro_tile.config.sample_spacing_blocks,
        columns,
        stats,
        config,
    }
}

pub fn heightfield_column_from_sample(
    sample: &MacroFieldSample,
    config: HeightfieldConfig,
) -> HeightfieldColumn {
    validate_heightfield_config(config);
    let raw_surface_height_blocks = normalized_to_blocks(sample.combined_macro_height, config);
    let contour_guided_surface_height_blocks =
        resolve_contour_band_height(raw_surface_height_blocks, config.contour);
    let is_ocean = sample.ocean_mask > 0.5;
    let is_lake = sample.lake_mask > 0.5;
    let water_bed_ceiling = if is_ocean {
        Some(config.sea_level_blocks + config.ocean_bed_blocks)
    } else if is_lake {
        Some(config.sea_level_blocks + config.lake_bed_blocks)
    } else {
        None
    };
    let meso_delta_blocks = 0.0;
    let micro_relief_blocks = 0.0;
    let mut surface_height_blocks =
        contour_guided_surface_height_blocks + meso_delta_blocks + micro_relief_blocks;
    if let Some(bed_ceiling) = water_bed_ceiling {
        surface_height_blocks = surface_height_blocks.min(bed_ceiling);
    } else if sample.coast_mask > 0.0 {
        surface_height_blocks =
            apply_shoreline_ramp(surface_height_blocks, sample.coast_mask, config);
    }
    let constrained_surface_height_blocks =
        surface_height_blocks.clamp(config.min_height_blocks, config.max_height_blocks);
    let final_surface_height_blocks = if water_bed_ceiling.is_some() {
        constrained_surface_height_blocks
    } else {
        snap_to_contour_step(constrained_surface_height_blocks, config.contour)
    };
    let surface_y = snap_height_to_block(final_surface_height_blocks);
    let surface_height_blocks = surface_y as f32;
    let is_river_hint = sample.river_valley_strength >= config.river_water_threshold
        && sample.river_flow_hint > 0.0
        && !is_ocean
        && !is_lake;
    let water_level_blocks = if is_ocean || is_lake {
        Some(snap_height_to_block(config.sea_level_blocks) as f32)
    } else if is_river_hint {
        Some(surface_height_blocks + 1.0)
    } else {
        None
    };
    let water_y = water_level_blocks.map(snap_height_to_block);
    let terrain_kind = if is_ocean {
        HeightfieldTerrainKind::Ocean
    } else if is_lake {
        HeightfieldTerrainKind::Lake
    } else if is_river_hint {
        HeightfieldTerrainKind::River
    } else if sample.dry_basin_mask > 0.5 {
        HeightfieldTerrainKind::DryBasin
    } else if sample.ridge_influence > 0.55 {
        HeightfieldTerrainKind::Ridge
    } else if sample.coast_mask > 0.45 {
        HeightfieldTerrainKind::Coast
    } else {
        HeightfieldTerrainKind::Land
    };

    HeightfieldColumn {
        position: sample.position,
        raw_surface_height_blocks,
        contour_guided_surface_height_blocks,
        constrained_surface_height_blocks,
        surface_height_blocks,
        surface_y,
        water_level_blocks,
        water_y,
        terrain_kind,
        macro_elevation: sample.macro_elevation,
        combined_macro_height: sample.combined_macro_height,
        ocean_mask: sample.ocean_mask,
        lake_mask: sample.lake_mask,
        dry_basin_mask: sample.dry_basin_mask,
        coast_mask: sample.coast_mask,
        ridge_influence: sample.ridge_influence,
        river_valley_strength: sample.river_valley_strength,
        river_flow_hint: sample.river_flow_hint,
        meso_delta_blocks,
        micro_relief_blocks,
    }
}

fn normalized_to_blocks(value: f32, config: HeightfieldConfig) -> f32 {
    let span = (config.normalized_max_height - config.normalized_min_height).max(f32::EPSILON);
    let t = ((value - config.normalized_min_height) / span).clamp(0.0, 1.0);
    config.min_height_blocks + (config.max_height_blocks - config.min_height_blocks) * t
}

fn resolve_contour_band_height(value: f32, contour: HeightfieldContourConfig) -> f32 {
    snap_to_contour_step(value, contour)
}

fn snap_to_contour_step(value: f32, contour: HeightfieldContourConfig) -> f32 {
    if contour.step_blocks <= 0.0 {
        return value;
    }
    let step = contour.step_blocks;
    (value / step).floor() * step
}

fn apply_shoreline_ramp(
    surface_height_blocks: f32,
    coast_mask: f32,
    config: HeightfieldConfig,
) -> f32 {
    let coast_t = coast_mask.clamp(0.0, 1.0);
    if coast_t <= 0.0 {
        return surface_height_blocks;
    }
    let inland_t = (1.0 - coast_t).clamp(0.0, 1.0);
    let max_land_height = config.sea_level_blocks + config.shore_ramp_blocks * inland_t.powf(1.65);
    surface_height_blocks
        .max(config.sea_level_blocks)
        .min(max_land_height)
}

fn apply_shoreline_contour_ceiling(
    surface_height_blocks: f32,
    water_distance_blocks: f32,
    sample_spacing_blocks: f32,
    config: HeightfieldConfig,
) -> f32 {
    if !water_distance_blocks.is_finite() || sample_spacing_blocks <= 0.0 {
        return surface_height_blocks;
    }
    let edge_distance = (water_distance_blocks - sample_spacing_blocks * 0.5).max(0.0);
    let step = config.contour.step_blocks.max(1.0);
    let allowed_steps_from_water = (edge_distance / sample_spacing_blocks).floor();
    let max_land_height = config.sea_level_blocks + allowed_steps_from_water * step;
    surface_height_blocks
        .max(config.sea_level_blocks)
        .min(max_land_height)
}

fn apply_neighbor_shoreline_continuity(
    columns: &mut [HeightfieldColumn],
    width: usize,
    height: usize,
    sample_spacing_blocks: f32,
    config: HeightfieldConfig,
) {
    if columns.is_empty() || width == 0 || height == 0 || config.shore_ramp_blocks <= 0.0 {
        return;
    }
    let water_distance =
        water_distance_field(columns, width, height, sample_spacing_blocks, config);
    for (index, column) in columns.iter_mut().enumerate() {
        if column.water_level_blocks.is_some() || water_distance[index] > config.shore_ramp_blocks {
            continue;
        }
        let clamped = apply_shoreline_contour_ceiling(
            column.constrained_surface_height_blocks,
            water_distance[index],
            sample_spacing_blocks,
            config,
        );
        if clamped < column.constrained_surface_height_blocks {
            column.constrained_surface_height_blocks = clamped;
            let final_surface_height_blocks = snap_to_contour_step(clamped, config.contour);
            column.surface_y = snap_height_to_block(final_surface_height_blocks);
            column.surface_height_blocks = column.surface_y as f32;
            if matches!(column.terrain_kind, HeightfieldTerrainKind::Land) {
                column.terrain_kind = HeightfieldTerrainKind::Coast;
            }
        }
    }
}

fn water_distance_field(
    columns: &[HeightfieldColumn],
    width: usize,
    height: usize,
    sample_spacing_blocks: f32,
    config: HeightfieldConfig,
) -> Vec<f32> {
    let mut distance = columns
        .iter()
        .map(|column| {
            if column.water_level_blocks.is_some() {
                0.0
            } else {
                f32::INFINITY
            }
        })
        .collect::<Vec<_>>();
    let diagonal = sample_spacing_blocks * std::f32::consts::SQRT_2;
    let limit = config.shore_ramp_blocks + diagonal;
    for _ in 0..2 {
        for z in 0..height {
            for x in 0..width {
                update_distance_from_neighbor(
                    &mut distance,
                    x,
                    z,
                    -1,
                    0,
                    sample_spacing_blocks,
                    width,
                    height,
                    limit,
                );
                update_distance_from_neighbor(
                    &mut distance,
                    x,
                    z,
                    0,
                    -1,
                    sample_spacing_blocks,
                    width,
                    height,
                    limit,
                );
                update_distance_from_neighbor(
                    &mut distance,
                    x,
                    z,
                    -1,
                    -1,
                    diagonal,
                    width,
                    height,
                    limit,
                );
                update_distance_from_neighbor(
                    &mut distance,
                    x,
                    z,
                    1,
                    -1,
                    diagonal,
                    width,
                    height,
                    limit,
                );
            }
        }
        for z in (0..height).rev() {
            for x in (0..width).rev() {
                update_distance_from_neighbor(
                    &mut distance,
                    x,
                    z,
                    1,
                    0,
                    sample_spacing_blocks,
                    width,
                    height,
                    limit,
                );
                update_distance_from_neighbor(
                    &mut distance,
                    x,
                    z,
                    0,
                    1,
                    sample_spacing_blocks,
                    width,
                    height,
                    limit,
                );
                update_distance_from_neighbor(
                    &mut distance,
                    x,
                    z,
                    1,
                    1,
                    diagonal,
                    width,
                    height,
                    limit,
                );
                update_distance_from_neighbor(
                    &mut distance,
                    x,
                    z,
                    -1,
                    1,
                    diagonal,
                    width,
                    height,
                    limit,
                );
            }
        }
    }
    distance
}

#[allow(clippy::too_many_arguments)]
fn update_distance_from_neighbor(
    distance: &mut [f32],
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
    let index = z * width + x;
    let neighbor = nz as usize * width + nx as usize;
    let candidate = distance[neighbor] + cost;
    if candidate < distance[index] && candidate <= limit {
        distance[index] = candidate;
    }
}

fn snap_height_to_block(value: f32) -> i32 {
    value.round() as i32
}

fn heightfield_stats(
    columns: &[HeightfieldColumn],
    config: HeightfieldConfig,
) -> HeightfieldTileStats {
    if columns.is_empty() {
        return HeightfieldTileStats::default();
    }

    let mut min = f32::MAX;
    let mut max = f32::MIN;
    let mut sum = 0.0;
    let mut water = 0usize;
    let mut ocean = 0usize;
    let mut lake = 0usize;
    let mut river = 0usize;
    let mut dry = 0usize;
    let mut ridge = 0usize;

    for column in columns {
        min = min.min(column.surface_height_blocks);
        max = max.max(column.surface_height_blocks);
        sum += column.surface_height_blocks;
        if column.has_water_column() {
            water += 1;
        }
        match column.terrain_kind {
            HeightfieldTerrainKind::Ocean => ocean += 1,
            HeightfieldTerrainKind::Lake => lake += 1,
            HeightfieldTerrainKind::River => river += 1,
            HeightfieldTerrainKind::DryBasin => dry += 1,
            HeightfieldTerrainKind::Ridge => ridge += 1,
            HeightfieldTerrainKind::Coast | HeightfieldTerrainKind::Land => {}
        }
    }

    HeightfieldTileStats {
        column_count: columns.len(),
        min_surface_height_blocks: min,
        max_surface_height_blocks: max,
        average_surface_height_blocks: sum / columns.len() as f32,
        contour_step_blocks: config.contour.step_blocks,
        contour_band_smoothing: config.contour.band_smoothing,
        max_raw_neighbor_delta_blocks: max_neighbor_delta(columns, |column| {
            column.raw_surface_height_blocks
        }),
        max_contour_guided_neighbor_delta_blocks: max_neighbor_delta(columns, |column| {
            column.contour_guided_surface_height_blocks
        }),
        max_constrained_neighbor_delta_blocks: max_neighbor_delta(columns, |column| {
            column.constrained_surface_height_blocks
        }),
        max_snapped_neighbor_delta_blocks: max_neighbor_delta(columns, |column| {
            column.surface_height_blocks
        }),
        max_visible_neighbor_delta_blocks: max_neighbor_delta(columns, |column| {
            column.visible_surface_height_blocks()
        }),
        max_shore_visible_neighbor_delta_blocks: max_shore_neighbor_delta(columns),
        water_column_count: water,
        ocean_column_count: ocean,
        lake_column_count: lake,
        river_hint_column_count: river,
        dry_basin_column_count: dry,
        ridge_column_count: ridge,
    }
}

fn max_shore_neighbor_delta(columns: &[HeightfieldColumn]) -> f32 {
    if columns.len() < 2 {
        return 0.0;
    }
    let width = infer_row_width(columns);
    let mut max_delta = 0.0f32;
    for (index, column) in columns.iter().enumerate() {
        if index + 1 < columns.len() && (index + 1) % width != 0 {
            let neighbor = &columns[index + 1];
            if column.water_level_blocks.is_some() || neighbor.water_level_blocks.is_some() {
                max_delta = max_delta.max(
                    (column.visible_surface_height_blocks()
                        - neighbor.visible_surface_height_blocks())
                    .abs(),
                );
            }
        }
        if index + width < columns.len() {
            let neighbor = &columns[index + width];
            if column.water_level_blocks.is_some() || neighbor.water_level_blocks.is_some() {
                max_delta = max_delta.max(
                    (column.visible_surface_height_blocks()
                        - neighbor.visible_surface_height_blocks())
                    .abs(),
                );
            }
        }
    }
    max_delta
}

fn max_neighbor_delta(
    columns: &[HeightfieldColumn],
    value: impl Fn(&HeightfieldColumn) -> f32,
) -> f32 {
    if columns.len() < 2 {
        return 0.0;
    }
    let width = infer_row_width(columns);
    let mut max_delta = 0.0f32;
    for (index, column) in columns.iter().enumerate() {
        if index + 1 < columns.len() && (index + 1) % width != 0 {
            max_delta = max_delta.max((value(column) - value(&columns[index + 1])).abs());
        }
        if index + width < columns.len() {
            max_delta = max_delta.max((value(column) - value(&columns[index + width])).abs());
        }
    }
    max_delta
}

fn infer_row_width(columns: &[HeightfieldColumn]) -> usize {
    if columns.len() < 2 {
        return columns.len().max(1);
    }
    let first_z = columns[0].position.z;
    columns
        .iter()
        .position(|column| (column.position.z - first_z).abs() > f32::EPSILON)
        .unwrap_or(columns.len())
        .max(1)
}

fn validate_heightfield_config(config: HeightfieldConfig) {
    assert!(config.sea_level_blocks.is_finite());
    assert!(config.min_height_blocks.is_finite());
    assert!(config.max_height_blocks.is_finite());
    assert!(config.normalized_min_height.is_finite());
    assert!(config.normalized_max_height.is_finite());
    assert!(config.river_water_threshold.is_finite());
    assert!(config.ocean_bed_blocks.is_finite());
    assert!(config.lake_bed_blocks.is_finite());
    assert!(config.min_height_blocks < config.max_height_blocks);
    assert!(config.normalized_min_height < config.normalized_max_height);
    assert!(config.ocean_bed_blocks <= 0.0);
    assert!(config.lake_bed_blocks <= 0.0);
    assert!(config.shore_ramp_blocks.is_finite());
    assert!(config.shore_min_land_blocks.is_finite());
    assert!(config.shore_ramp_blocks >= 0.0);
    assert!(config.shore_min_land_blocks >= 0.0);
    assert!(config.contour.step_blocks.is_finite());
    assert!(config.contour.band_smoothing.is_finite());
    assert!(config.contour.step_blocks > 0.0);
    assert!(config.contour.band_smoothing >= 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generation::macro_field::{MacroFieldTileConfig, MacroFieldTileStats};

    #[test]
    fn heightfield_tile_generation_is_deterministic() {
        let macro_tile = test_macro_tile();
        let config = HeightfieldConfig::default();

        let first = generate_heightfield_tile(&macro_tile, config);
        let second = generate_heightfield_tile(&macro_tile, config);

        assert_eq!(first, second);
    }

    #[test]
    fn dimensions_and_column_count_match_macro_tile() {
        let macro_tile = test_macro_tile();
        let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());

        assert_eq!(tile.width, macro_tile.config.width);
        assert_eq!(tile.height, macro_tile.config.height);
        assert_eq!(tile.columns.len(), macro_tile.samples.len());
        assert_eq!(tile.stats.column_count, macro_tile.samples.len());
        assert!(tile.column(0, 0).is_some());
        assert!(tile.column(tile.width, 0).is_none());
    }

    #[test]
    fn heights_are_finite_and_stats_are_ordered() {
        let tile = generate_heightfield_tile(&test_macro_tile(), HeightfieldConfig::default());

        assert!(tile.columns.iter().all(|column| {
            column.surface_height_blocks.is_finite()
                && column.raw_surface_height_blocks.is_finite()
                && column.contour_guided_surface_height_blocks.is_finite()
                && column.constrained_surface_height_blocks.is_finite()
                && column.meso_delta_blocks == 0.0
                && column.micro_relief_blocks == 0.0
        }));
        assert!(tile.stats.min_surface_height_blocks <= tile.stats.max_surface_height_blocks);
    }

    #[test]
    fn column_heights_are_snapped_to_integer_blocks() {
        let sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
        let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());

        assert_eq!(column.surface_height_blocks.fract(), 0.0);
        assert_eq!(column.surface_height_blocks, column.surface_y as f32);
    }

    #[test]
    fn default_contour_step_is_one_block_without_smoothing() {
        let contour = HeightfieldContourConfig::default();

        assert_eq!(contour.step_blocks, 1.0);
        assert_eq!(contour.band_smoothing, 0.0);
    }

    #[test]
    fn contour_guided_height_is_pure_lower_band_and_snaps() {
        let config = HeightfieldConfig::default();
        let sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
        let column = heightfield_column_from_sample(&sample, config);
        let lower = (column.raw_surface_height_blocks / config.contour.step_blocks).floor()
            * config.contour.step_blocks;

        assert_eq!(column.contour_guided_surface_height_blocks, lower);
        assert_eq!(column.surface_height_blocks.fract(), 0.0);
    }

    #[test]
    fn changing_contour_step_snaps_land_surface_to_step_multiples() {
        let sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
        let column = heightfield_column_from_sample(
            &sample,
            HeightfieldConfig {
                contour: HeightfieldContourConfig {
                    step_blocks: 4.0,
                    band_smoothing: 0.0,
                },
                ..HeightfieldConfig::default()
            },
        );

        assert_eq!(column.contour_guided_surface_height_blocks % 4.0, 0.0);
        assert_eq!(column.surface_height_blocks % 4.0, 0.0);
    }

    #[test]
    fn raw_continuous_height_is_recorded_but_final_uses_band_value() {
        let sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
        let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());

        assert_ne!(
            column.raw_surface_height_blocks,
            column.contour_guided_surface_height_blocks
        );
        assert_eq!(
            column.surface_height_blocks,
            column.contour_guided_surface_height_blocks
        );
    }

    #[test]
    fn coast_adjacent_land_ramps_from_sea_level() {
        let mut sample = sample(0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0);
        sample.coast_mask = 1.0;
        let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());

        assert!(column.surface_height_blocks >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS);
        assert!(
            column.surface_height_blocks
                <= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
                    + DEFAULT_HEIGHTFIELD_SHORE_MIN_LAND_BLOCKS
                    + 1.0
        );
    }

    #[test]
    fn water_columns_appear_for_ocean_and_lake_masks() {
        let tile = generate_heightfield_tile(&test_macro_tile(), HeightfieldConfig::default());

        assert!(tile.stats.ocean_column_count > 0);
        assert!(tile.stats.lake_column_count > 0);
        assert!(tile.stats.water_column_count > 0);
        assert!(
            tile.columns
                .iter()
                .filter(|column| matches!(
                    column.terrain_kind,
                    HeightfieldTerrainKind::Ocean | HeightfieldTerrainKind::Lake
                ))
                .all(|column| column.water_level_blocks.is_some())
        );
    }

    #[test]
    fn neighbor_shoreline_continuity_clamps_land_next_to_water() {
        let config = MacroFieldTileConfig::new(0.0, 0.0, 3, 1, 32.0);
        let samples = vec![
            sample(0.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0),
            sample(32.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
            sample(64.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
        ];
        let macro_tile = MacroFieldTile {
            config,
            samples,
            stats: MacroFieldTileStats::default(),
        };
        let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
        let coast = tile.column(1, 0).expect("coast column");
        let inland = tile.column(2, 0).expect("inland column");

        assert!(coast.raw_surface_height_blocks > 100.0);
        assert_eq!(
            coast.surface_height_blocks, DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
            "first land sample next to water should start at sea level in contour-step mode"
        );
        assert_eq!(
            inland.surface_height_blocks,
            DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS + DEFAULT_HEIGHTFIELD_CONTOUR_STEP_BLOCKS,
            "second land sample should rise by one contour step"
        );
        assert!(inland.surface_height_blocks >= coast.surface_height_blocks);
        assert!(
            tile.stats.max_shore_visible_neighbor_delta_blocks
                <= DEFAULT_HEIGHTFIELD_CONTOUR_STEP_BLOCKS,
            "water/shore visible top should not jump more than one contour step, got {}",
            tile.stats.max_shore_visible_neighbor_delta_blocks
        );
        assert!(
            tile.stats.max_constrained_neighbor_delta_blocks
                < tile.stats.max_raw_neighbor_delta_blocks,
            "shoreline continuity should reduce the raw neighbor jump"
        );
    }

    #[test]
    fn water_visible_top_uses_water_surface_not_bed() {
        let sample = sample(0.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0);
        let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());

        assert!(column.surface_height_blocks <= DEFAULT_HEIGHTFIELD_OCEAN_BED_BLOCKS);
        assert_eq!(
            column.water_level_blocks,
            Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS)
        );
        assert_eq!(
            column.visible_surface_height_blocks(),
            DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
        );
    }

    #[test]
    fn raw_and_constrained_heights_are_recorded_before_snap() {
        let mut sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
        sample.coast_mask = 0.65;
        let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());

        assert_ne!(
            column.raw_surface_height_blocks,
            column.surface_height_blocks
        );
        assert_ne!(
            column.constrained_surface_height_blocks,
            column.surface_height_blocks
        );
        assert_eq!(column.surface_height_blocks, column.surface_y as f32);
    }

    fn test_macro_tile() -> MacroFieldTile {
        let config = MacroFieldTileConfig::new(0.0, 0.0, 3, 2, 32.0);
        let samples = vec![
            sample(0.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0),
            sample(32.0, 0.0, -0.25, 0.0, 1.0, 0.0, 0.0),
            sample(64.0, 0.0, 0.05, 0.0, 0.0, 1.0, 0.0),
            sample(0.0, 32.0, 0.35, 0.0, 0.0, 0.0, 0.0),
            sample(32.0, 32.0, 0.7, 0.0, 0.0, 0.0, 0.8),
            sample(64.0, 32.0, 1.0, 0.0, 0.0, 0.0, 0.0),
        ];

        MacroFieldTile {
            config,
            samples,
            stats: MacroFieldTileStats::default(),
        }
    }

    fn sample(
        x: f32,
        z: f32,
        height: f32,
        ocean: f32,
        lake: f32,
        dry: f32,
        ridge: f32,
    ) -> MacroFieldSample {
        MacroFieldSample {
            position: WorldPlanePoint::new(x, z),
            nearest_site: None,
            surface_kind: None,
            macro_elevation: height,
            ocean_mask: ocean,
            coast_mask: 0.0,
            lake_mask: lake,
            dry_basin_mask: dry,
            ridge_influence: ridge,
            river_valley_strength: 0.0,
            river_distance_blocks: f32::INFINITY,
            river_flow_hint: 0.0,
            combined_macro_height: height,
        }
    }
}
