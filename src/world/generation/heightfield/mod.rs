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
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HeightfieldTileStats {
    pub column_count: usize,
    pub min_surface_height_blocks: f32,
    pub max_surface_height_blocks: f32,
    pub average_surface_height_blocks: f32,
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
    let columns = macro_tile
        .samples
        .par_iter()
        .map(|sample| heightfield_column_from_sample(sample, config))
        .collect::<Vec<_>>();
    let stats = heightfield_stats(&columns);

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
    let base_surface = normalized_to_blocks(sample.combined_macro_height, config);
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
    let mut surface_height_blocks = base_surface + meso_delta_blocks + micro_relief_blocks;
    if let Some(bed_ceiling) = water_bed_ceiling {
        surface_height_blocks = surface_height_blocks.min(bed_ceiling);
    }
    let surface_height_blocks =
        surface_height_blocks.clamp(config.min_height_blocks, config.max_height_blocks);
    let surface_y = surface_height_blocks.floor() as i32;
    let is_river_hint = sample.river_valley_strength >= config.river_water_threshold
        && sample.river_flow_hint > 0.0
        && !is_ocean
        && !is_lake;
    let water_level_blocks = if is_ocean || is_lake {
        Some(config.sea_level_blocks)
    } else if is_river_hint {
        Some(surface_height_blocks + 1.0)
    } else {
        None
    };
    let water_y = water_level_blocks.map(|water| water.ceil() as i32);
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

fn heightfield_stats(columns: &[HeightfieldColumn]) -> HeightfieldTileStats {
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
        water_column_count: water,
        ocean_column_count: ocean,
        lake_column_count: lake,
        river_hint_column_count: river,
        dry_basin_column_count: dry,
        ridge_column_count: ridge,
    }
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
                && column.meso_delta_blocks == 0.0
                && column.micro_relief_blocks == 0.0
        }));
        assert!(tile.stats.min_surface_height_blocks <= tile.stats.max_surface_height_blocks);
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
