use rayon::prelude::*;

use super::graph::WorldPlanePoint;
use super::macro_field::MacroFieldTile;

mod column;
mod mapping;
pub mod perlin;
mod river;
mod stats;
mod water;

pub use self::stats::HeightfieldTileStats;
pub use column::heightfield_column_from_sample;
pub const DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS: f32 = 0.0;
pub const DEFAULT_HEIGHTFIELD_MIN_BLOCKS: f32 = -1024.0;
pub const DEFAULT_HEIGHTFIELD_MAX_BLOCKS: f32 = 2048.0;
pub const DEFAULT_HEIGHTFIELD_NORMALIZED_MIN: f32 = -0.5;
pub const DEFAULT_HEIGHTFIELD_NORMALIZED_MAX: f32 = 1.0;
pub const DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD: f32 = 0.88;
pub const DEFAULT_HEIGHTFIELD_OCEAN_BED_BLOCKS: f32 = -12.0;
pub const DEFAULT_HEIGHTFIELD_LAKE_BED_BLOCKS: f32 = -2.0;
pub const DEFAULT_HEIGHTFIELD_CONTOUR_STEP_BLOCKS: f32 = 1.0;
pub const DEFAULT_HEIGHTFIELD_CONTOUR_MIN_GAP_BLOCKS: f32 = 0.0;
pub const DEFAULT_HEIGHTFIELD_RIVER_CONTOUR_MIN_GAP_BLOCKS: f32 = 0.0;
pub const DEFAULT_HEIGHTFIELD_CONTOUR_BAND_SMOOTHING: f32 = 0.0;

pub use perlin::{
    DEFAULT_HEIGHTFIELD_PERLIN_AMPLITUDE_BLOCKS, DEFAULT_HEIGHTFIELD_PERLIN_BASE_SCALE_BLOCKS,
    DEFAULT_HEIGHTFIELD_PERLIN_LACUNARITY, DEFAULT_HEIGHTFIELD_PERLIN_MAX_ABS_BLOCKS,
    DEFAULT_HEIGHTFIELD_PERLIN_OCTAVES, DEFAULT_HEIGHTFIELD_PERLIN_PERSISTENCE,
    HeightfieldPerlinConfig, HeightfieldPerlinPlacement,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightfieldContourConfig {
    pub step_blocks: f32,
    pub min_gap_blocks: f32,
    pub river_min_gap_blocks: f32,
    pub band_smoothing: f32,
}

impl Default for HeightfieldContourConfig {
    fn default() -> Self {
        Self {
            step_blocks: DEFAULT_HEIGHTFIELD_CONTOUR_STEP_BLOCKS,
            min_gap_blocks: DEFAULT_HEIGHTFIELD_CONTOUR_MIN_GAP_BLOCKS,
            river_min_gap_blocks: DEFAULT_HEIGHTFIELD_RIVER_CONTOUR_MIN_GAP_BLOCKS,
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
    pub contour: HeightfieldContourConfig,
    pub perlin: HeightfieldPerlinConfig,
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
            contour: HeightfieldContourConfig::default(),
            perlin: HeightfieldPerlinConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightfieldTerrainKind {
    Ocean,
    Lake,
    RiverCore,
    RiverBed,
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
    pub river_core_water_height_blocks: Option<f32>,
    pub terrain_kind: HeightfieldTerrainKind,
    pub macro_elevation: f32,
    pub combined_macro_height: f32,
    pub ocean_mask: f32,
    pub lake_mask: f32,
    pub dry_basin_mask: f32,
    pub coast_mask: f32,
    pub ridge_influence: f32,
    pub terrain_ruggedness: f32,
    pub river_core_strength: f32,
    pub river_shoulder_strength: f32,
    pub river_valley_strength: f32,
    pub river_distance_blocks: f32,
    pub river_flow_hint: f32,
    pub river_core_depth_blocks: f32,
    pub river_bank_roughness_hint: f32,
    pub river_gravel_hint: f32,
    pub river_cutbank_hint: f32,
    pub meso_delta_blocks: f32,
    pub micro_relief_blocks: f32,
}

impl HeightfieldColumn {
    pub fn has_water_column(&self) -> bool {
        self.water_level_blocks.is_some()
    }

    pub fn visible_surface_height_blocks(&self) -> f32 {
        self.water_level_blocks
            .map(|water| water.max(self.surface_height_blocks))
            .unwrap_or(self.surface_height_blocks)
    }
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
    river::apply_river_water_descent(
        &mut columns,
        macro_tile.config.width as usize,
        macro_tile.config.height as usize,
        config.sea_level_blocks,
    );
    let stats = stats::heightfield_stats(&columns, config);

    HeightfieldTile {
        width: macro_tile.config.width,
        height: macro_tile.config.height,
        sample_spacing_blocks: macro_tile.config.sample_spacing_blocks,
        columns,
        stats,
        config,
    }
}

pub(super) fn validate_heightfield_config(config: HeightfieldConfig) {
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
    assert!(config.contour.step_blocks.is_finite());
    assert!(config.contour.min_gap_blocks.is_finite());
    assert!(config.contour.river_min_gap_blocks.is_finite());
    assert!(config.contour.band_smoothing.is_finite());
    assert!(config.contour.step_blocks > 0.0);
    assert!(config.contour.min_gap_blocks >= 0.0);
    assert!(config.contour.river_min_gap_blocks >= 0.0);
    assert!(config.contour.band_smoothing >= 0.0);
    assert!(config.perlin.amplitude_blocks.is_finite());
    assert!(config.perlin.base_scale_blocks.is_finite());
    assert!(config.perlin.persistence.is_finite());
    assert!(config.perlin.lacunarity.is_finite());
    assert!(config.perlin.max_abs_blocks.is_finite());
    assert!(config.perlin.amplitude_blocks >= 0.0);
    assert!(config.perlin.base_scale_blocks > 0.0);
    assert!(config.perlin.persistence >= 0.0);
    assert!(config.perlin.lacunarity >= 1.0);
    assert!(config.perlin.max_abs_blocks >= 0.0);
}
