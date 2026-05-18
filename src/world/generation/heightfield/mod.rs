use rayon::prelude::*;

use super::graph::WorldPlanePoint;
use super::macro_field::{MacroFieldSample, MacroFieldTile};

pub mod perlin;

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
    HeightfieldPerlinConfig, HeightfieldPerlinPlacement,
    DEFAULT_HEIGHTFIELD_PERLIN_AMPLITUDE_BLOCKS, DEFAULT_HEIGHTFIELD_PERLIN_BASE_SCALE_BLOCKS,
    DEFAULT_HEIGHTFIELD_PERLIN_LACUNARITY, DEFAULT_HEIGHTFIELD_PERLIN_MAX_ABS_BLOCKS,
    DEFAULT_HEIGHTFIELD_PERLIN_OCTAVES, DEFAULT_HEIGHTFIELD_PERLIN_PERSISTENCE,
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
    pub river_water_height_blocks: Option<f32>,
    pub terrain_kind: HeightfieldTerrainKind,
    pub macro_elevation: f32,
    pub combined_macro_height: f32,
    pub ocean_mask: f32,
    pub lake_mask: f32,
    pub dry_basin_mask: f32,
    pub coast_mask: f32,
    pub ridge_influence: f32,
    pub terrain_ruggedness: f32,
    pub river_valley_strength: f32,
    pub river_flow_hint: f32,
    pub river_bed_depth_blocks: f32,
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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HeightfieldTileStats {
    pub column_count: usize,
    pub min_surface_height_blocks: f32,
    pub max_surface_height_blocks: f32,
    pub average_surface_height_blocks: f32,
    pub contour_step_blocks: f32,
    pub contour_min_gap_blocks: f32,
    pub contour_river_min_gap_blocks: f32,
    pub contour_band_smoothing: f32,
    pub max_raw_neighbor_delta_blocks: f32,
    pub max_contour_guided_neighbor_delta_blocks: f32,
    pub max_constrained_neighbor_delta_blocks: f32,
    pub max_snapped_neighbor_delta_blocks: f32,
    pub max_visible_neighbor_delta_blocks: f32,
    pub max_shore_visible_neighbor_delta_blocks: f32,
    pub min_ocean_visible_surface_blocks: f32,
    pub max_ocean_visible_surface_blocks: f32,
    pub min_land_near_water_surface_blocks: f32,
    pub max_river_water_neighbor_delta_blocks: f32,
    pub river_uphill_flow_neighbor_count: usize,
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
    apply_river_water_descent(
        &mut columns,
        macro_tile.config.width as usize,
        macro_tile.config.height as usize,
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
    let contour = contour_config_for_sample(sample, config);
    let meso_delta_blocks = 0.0;
    let micro_relief_blocks = perlin::micro_relief_blocks(sample, config.perlin);
    let contour_source_height_blocks = match config.perlin.placement {
        HeightfieldPerlinPlacement::BeforeContour => {
            raw_surface_height_blocks + meso_delta_blocks + micro_relief_blocks
        }
        HeightfieldPerlinPlacement::AfterContourBeforeSnap => raw_surface_height_blocks,
    };
    let contour_guided_surface_height_blocks =
        resolve_contour_band_height(contour_source_height_blocks, contour);
    let is_ocean = sample.ocean_mask > 0.5;
    let is_lake = sample.lake_mask > 0.5;
    let has_river_bed_hint = sample.river_valley_strength >= config.river_water_threshold
        && sample.river_flow_hint > 0.0;
    let is_river_hint = has_river_bed_hint && !is_ocean && !is_lake;
    let river_bed_depth_blocks = if has_river_bed_hint {
        river_bed_depth_blocks(sample)
    } else {
        0.0
    };
    let surface_height_blocks = match config.perlin.placement {
        HeightfieldPerlinPlacement::BeforeContour => contour_guided_surface_height_blocks,
        HeightfieldPerlinPlacement::AfterContourBeforeSnap => {
            contour_guided_surface_height_blocks + meso_delta_blocks + micro_relief_blocks
        }
    };
    let lake_water_level_blocks = is_lake.then(|| lake_water_level_blocks(sample, config));
    let is_dry_basin = sample.dry_basin_mask > 0.5;
    let river_bed_base_height_blocks = surface_height_blocks - river_bed_depth_blocks;
    let river_bed_relief_blocks = if is_river_hint {
        perlin::river_bed_relief_blocks(sample, config.perlin).clamp(
            -river_bed_depth_blocks * 0.75,
            (river_water_depth_blocks(sample) - 1.0).max(0.0),
        )
    } else {
        0.0
    };
    let river_bed_variation_blocks = if has_river_bed_hint {
        let flow = sample.river_flow_hint.clamp(0.0, 1.0);
        let downcut_limit = river_bed_depth_blocks * 0.15 * smoothstep01((flow - 0.10) / 0.30);
        deterministic_river_bed_variation_blocks(sample).clamp(
            -downcut_limit,
            (river_water_depth_blocks(sample) - 1.0).max(0.0),
        )
    } else {
        0.0
    };
    let river_bank_relief_blocks = if !is_river_hint
        && !is_ocean
        && !is_lake
        && sample.river_flow_hint > 0.0
        && sample.river_valley_strength > config.river_water_threshold * 0.18
    {
        (deterministic_river_bank_variation_blocks(sample)
            + perlin::river_bank_relief_blocks(sample, config.perlin))
        .clamp(-14.0, 14.0)
    } else {
        0.0
    };
    let ocean_bed_relief_blocks = if is_ocean && surface_height_blocks < config.sea_level_blocks {
        let max_raise_blocks = (config.sea_level_blocks - surface_height_blocks - 1.0).max(0.0);
        perlin::ocean_bed_relief_blocks(sample, config.perlin)
            .clamp(-config.perlin.max_abs_blocks, max_raise_blocks)
    } else {
        0.0
    };
    let river_water_level_blocks = if is_river_hint {
        let base_constrained =
            river_bed_base_height_blocks.clamp(config.min_height_blocks, config.max_height_blocks);
        let base_snapped = snap_to_contour_step(base_constrained, contour);
        let base_y = snap_height_to_block(base_snapped) as f32;
        Some(snap_height_to_block(
            (base_y + river_water_depth_blocks(sample)).max(config.sea_level_blocks),
        ) as f32)
    } else {
        None
    };
    let surface_height_blocks = if (is_ocean || is_lake) && has_river_bed_hint {
        let water = lake_water_level_blocks.unwrap_or(config.sea_level_blocks);
        (water - river_bed_depth_blocks + river_bed_variation_blocks).min(water - 1.0)
    } else if is_ocean {
        ocean_bed_height_blocks(surface_height_blocks + ocean_bed_relief_blocks, config)
    } else if is_lake {
        lake_water_level_blocks
            .map(|water| lake_bed_height_blocks(surface_height_blocks, water, config))
            .unwrap_or(surface_height_blocks)
    } else if is_river_hint {
        river_bed_base_height_blocks + river_bed_relief_blocks + river_bed_variation_blocks
    } else {
        (surface_height_blocks + river_bank_relief_blocks).max(config.sea_level_blocks)
    };
    let constrained_surface_height_blocks =
        surface_height_blocks.clamp(config.min_height_blocks, config.max_height_blocks);
    let final_surface_height_blocks =
        snap_to_contour_step(constrained_surface_height_blocks, contour);
    let surface_y = snap_height_to_block(final_surface_height_blocks);
    let surface_height_blocks = surface_y as f32;
    let ocean_water_level_blocks = (is_ocean && surface_height_blocks < config.sea_level_blocks)
        .then_some(snap_height_to_block(config.sea_level_blocks) as f32);
    let water_level_blocks = if is_ocean {
        ocean_water_level_blocks
    } else if is_lake {
        Some(
            snap_height_to_block(lake_water_level_blocks.unwrap_or(config.sea_level_blocks)) as f32,
        )
    } else if is_river_hint {
        river_water_level_blocks
    } else {
        None
    };
    let water_y = water_level_blocks.map(snap_height_to_block);
    let river_water_height_blocks =
        has_river_bed_hint.then_some(water_y.unwrap_or(surface_y) as f32);
    let terrain_kind = if is_ocean {
        HeightfieldTerrainKind::Ocean
    } else if is_lake {
        HeightfieldTerrainKind::Lake
    } else if is_river_hint {
        HeightfieldTerrainKind::River
    } else if is_dry_basin {
        HeightfieldTerrainKind::DryBasin
    } else if sample.ridge_influence > 0.55 {
        HeightfieldTerrainKind::Ridge
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
        river_water_height_blocks,
        terrain_kind,
        macro_elevation: sample.macro_elevation,
        combined_macro_height: sample.combined_macro_height,
        ocean_mask: sample.ocean_mask,
        lake_mask: sample.lake_mask,
        dry_basin_mask: sample.dry_basin_mask,
        coast_mask: sample.coast_mask,
        ridge_influence: sample.ridge_influence,
        terrain_ruggedness: sample
            .biome_context
            .map(|context| context.ruggedness.clamp(0.0, 1.0))
            .unwrap_or(0.0),
        river_valley_strength: sample.river_valley_strength,
        river_flow_hint: sample.river_flow_hint,
        river_bed_depth_blocks,
        river_bank_roughness_hint: sample.river_bank_roughness_hint,
        river_gravel_hint: sample.river_gravel_hint,
        river_cutbank_hint: sample.river_cutbank_hint,
        meso_delta_blocks,
        micro_relief_blocks,
    }
}

fn river_bed_depth_blocks(sample: &MacroFieldSample) -> f32 {
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    let bed = sample.river_bed_depth_hint.clamp(0.0, 1.0);
    let rough = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    (bed * 40.0)
        .max(1.0 + flow * 1.5 + rough * 0.4)
        .clamp(1.0, 40.0)
}

fn river_water_depth_blocks(sample: &MacroFieldSample) -> f32 {
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    let bed_depth = river_bed_depth_blocks(sample);
    (bed_depth * (0.78 + flow * 0.17)).clamp(1.0, bed_depth.max(1.0))
}

fn deterministic_river_bed_variation_blocks(sample: &MacroFieldSample) -> f32 {
    let valley = sample.river_valley_strength.clamp(0.0, 1.0);
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    if valley <= 0.0 || flow <= 0.0 {
        return 0.0;
    }

    let rough = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    let gravel = sample.river_gravel_hint.clamp(0.0, 1.0);
    let broad = heightfield_value_noise_2d(sample.position, 31.0, 0xB4D0_0001);
    let small = heightfield_value_noise_2d(
        WorldPlanePoint::new(sample.position.x + 17.0, sample.position.z - 29.0),
        13.0,
        0xB4D0_0002,
    );
    let flow_scale = lerp(0.25, 1.0, smoothstep01(flow));
    let amplitude =
        (0.35 + rough * 0.75 + gravel * 0.45) * (1.12 - flow * 0.32) * valley * flow_scale;

    (broad * 0.68 + small * 0.32).clamp(-1.0, 1.0) * amplitude
}

fn deterministic_river_bank_variation_blocks(sample: &MacroFieldSample) -> f32 {
    let valley = sample.river_valley_strength.clamp(0.0, 1.0);
    let flow = sample.river_flow_hint.clamp(0.0, 1.0);
    if valley <= 0.0 || flow <= 0.0 {
        return 0.0;
    }

    let rough = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    let gravel = sample.river_gravel_hint.clamp(0.0, 1.0);
    let bank = (1.0 - valley).clamp(0.0, 1.0);
    let shoulder = (bank * 1.6).clamp(0.0, 1.0);
    let broad = heightfield_value_noise_2d(sample.position, 47.0, 0xBA11_0001);
    let medium = heightfield_value_noise_2d(
        WorldPlanePoint::new(sample.position.x - 23.0, sample.position.z + 11.0),
        19.0,
        0xBA11_0002,
    );
    let amplitude = (0.45 + rough * 1.4 + gravel * 0.55) * shoulder * (1.0 - flow * 0.22);

    (broad * 0.6 + medium * 0.4).clamp(-1.0, 1.0) * amplitude
}

fn lake_water_level_blocks(sample: &MacroFieldSample, config: HeightfieldConfig) -> f32 {
    let source_level = normalized_to_blocks(sample.macro_elevation, config);
    let shoreline_margin = config.lake_bed_blocks.abs().max(2.0) * 8.0;

    (source_level - shoreline_margin)
        .max(config.sea_level_blocks + 1.0)
        .clamp(config.min_height_blocks, config.max_height_blocks)
}

fn ocean_bed_height_blocks(source_bed_height_blocks: f32, _config: HeightfieldConfig) -> f32 {
    source_bed_height_blocks
}

fn lake_bed_height_blocks(
    raw_bed_height_blocks: f32,
    water_level_blocks: f32,
    config: HeightfieldConfig,
) -> f32 {
    let shallow_gap = config.lake_bed_blocks.abs().max(1.0);
    let max_depth = shallow_gap * 12.0;
    raw_bed_height_blocks.max(water_level_blocks - max_depth)
}

fn normalized_to_blocks(value: f32, config: HeightfieldConfig) -> f32 {
    if value >= 0.0 {
        let t = (value / config.normalized_max_height.max(f32::EPSILON)).clamp(0.0, 1.0);
        config.sea_level_blocks + (config.max_height_blocks - config.sea_level_blocks) * t
    } else {
        let t = (value / config.normalized_min_height.min(-f32::EPSILON)).clamp(0.0, 1.0);
        config.sea_level_blocks + (config.min_height_blocks - config.sea_level_blocks) * t
    }
}

fn resolve_contour_band_height(value: f32, contour: HeightfieldContourConfig) -> f32 {
    if contour.step_blocks <= 0.0 {
        return value;
    }
    let step = contour.step_blocks;
    let stride = step + contour.min_gap_blocks.max(0.0);
    if value >= 0.0 {
        (value / stride).floor() * step
    } else {
        -((-value / stride).floor() * step)
    }
}

fn contour_config_for_sample(
    sample: &MacroFieldSample,
    config: HeightfieldConfig,
) -> HeightfieldContourConfig {
    let mut contour = config.contour;
    if sample.river_valley_strength >= config.river_water_threshold * 0.5
        && sample.river_flow_hint > 0.0
    {
        contour.min_gap_blocks = contour
            .min_gap_blocks
            .min(contour.river_min_gap_blocks.max(0.0));
    }
    contour
}

fn snap_to_contour_step(value: f32, contour: HeightfieldContourConfig) -> f32 {
    if contour.step_blocks <= 0.0 {
        return value;
    }
    let step = contour.step_blocks;
    (value / step).floor() * step
}

fn apply_river_water_descent(columns: &mut [HeightfieldColumn], width: usize, height: usize) {
    if columns.is_empty() || width == 0 || height == 0 {
        return;
    }

    let mut water_y = columns
        .iter()
        .map(|column| column.water_y)
        .collect::<Vec<_>>();
    for _ in 0..(width + height).max(1) {
        let mut changed = false;
        for index in 0..columns.len() {
            if !matches!(columns[index].terrain_kind, HeightfieldTerrainKind::River) {
                continue;
            }
            let Some(current) = water_y[index] else {
                continue;
            };
            let mut allowed = current;
            for neighbor in neighbor_indices(index, width, height) {
                let Some(neighbor_water) = water_y[neighbor] else {
                    continue;
                };
                if matches!(
                    columns[neighbor].terrain_kind,
                    HeightfieldTerrainKind::River
                ) && columns[index].river_flow_hint > columns[neighbor].river_flow_hint + 0.01
                {
                    allowed = allowed.min(neighbor_water);
                } else {
                    allowed = allowed.min(neighbor_water.saturating_add(1));
                }
            }
            if allowed < current {
                water_y[index] = Some(allowed);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    for (index, column) in columns.iter_mut().enumerate() {
        if !matches!(column.terrain_kind, HeightfieldTerrainKind::River) {
            continue;
        }
        let Some(water) = water_y[index] else {
            continue;
        };
        column.water_y = Some(water);
        column.water_level_blocks = Some(water as f32);
        column.river_water_height_blocks = Some(water as f32);
        if column.surface_y >= water {
            column.surface_y = water.saturating_sub(1);
            column.surface_height_blocks = column.surface_y as f32;
            column.constrained_surface_height_blocks = column.surface_height_blocks;
        }
    }
}

fn neighbor_indices(index: usize, width: usize, height: usize) -> impl Iterator<Item = usize> {
    let x = index % width;
    let z = index / width;
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
    neighbors
        .into_iter()
        .flatten()
        .filter(move |neighbor| *neighbor < width * height)
}

fn is_standing_water(column: HeightfieldColumn) -> bool {
    matches!(
        column.terrain_kind,
        HeightfieldTerrainKind::Ocean | HeightfieldTerrainKind::Lake
    )
}

fn snap_height_to_block(value: f32) -> i32 {
    value.floor() as i32
}

fn heightfield_value_noise_2d(position: WorldPlanePoint, scale_blocks: f32, salt: u64) -> f32 {
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

fn smoothstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
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
    let mut min_ocean_visible = f32::INFINITY;
    let mut max_ocean_visible = f32::NEG_INFINITY;
    let mut found_ocean = false;

    for column in columns {
        min = min.min(column.surface_height_blocks);
        max = max.max(column.surface_height_blocks);
        sum += column.surface_height_blocks;
        if column.has_water_column() {
            water += 1;
        }
        match column.terrain_kind {
            HeightfieldTerrainKind::Ocean => {
                ocean += 1;
                found_ocean = true;
                let visible = column.visible_surface_height_blocks();
                min_ocean_visible = min_ocean_visible.min(visible);
                max_ocean_visible = max_ocean_visible.max(visible);
            }
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
        contour_min_gap_blocks: config.contour.min_gap_blocks,
        contour_river_min_gap_blocks: config.contour.river_min_gap_blocks,
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
        min_ocean_visible_surface_blocks: found_ocean.then_some(min_ocean_visible).unwrap_or(0.0),
        max_ocean_visible_surface_blocks: found_ocean.then_some(max_ocean_visible).unwrap_or(0.0),
        min_land_near_water_surface_blocks: min_land_near_standing_water(columns),
        max_river_water_neighbor_delta_blocks: max_river_water_neighbor_delta(columns),
        river_uphill_flow_neighbor_count: river_uphill_flow_neighbor_count(columns),
        water_column_count: water,
        ocean_column_count: ocean,
        lake_column_count: lake,
        river_hint_column_count: river,
        dry_basin_column_count: dry,
        ridge_column_count: ridge,
    }
}

fn min_land_near_standing_water(columns: &[HeightfieldColumn]) -> f32 {
    if columns.len() < 2 {
        return 0.0;
    }
    let width = infer_row_width(columns);
    let height = columns.len().div_ceil(width);
    let mut min_land = f32::INFINITY;
    for (index, column) in columns.iter().enumerate() {
        if is_standing_water(*column) {
            continue;
        }
        if neighbor_indices(index, width, height)
            .any(|neighbor| is_standing_water(columns[neighbor]))
        {
            min_land = min_land.min(column.surface_height_blocks);
        }
    }
    min_land.is_finite().then_some(min_land).unwrap_or(0.0)
}

fn max_river_water_neighbor_delta(columns: &[HeightfieldColumn]) -> f32 {
    if columns.len() < 2 {
        return 0.0;
    }
    let width = infer_row_width(columns);
    let height = columns.len().div_ceil(width);
    let mut max_delta = 0.0f32;
    for (index, column) in columns.iter().enumerate() {
        if !matches!(column.terrain_kind, HeightfieldTerrainKind::River) {
            continue;
        }
        let Some(water) = column.river_water_height_blocks else {
            continue;
        };
        for neighbor in neighbor_indices(index, width, height) {
            let neighbor_column = columns[neighbor];
            if matches!(
                neighbor_column.terrain_kind,
                HeightfieldTerrainKind::River
                    | HeightfieldTerrainKind::Ocean
                    | HeightfieldTerrainKind::Lake
            ) {
                if let Some(neighbor_water) = neighbor_column.water_level_blocks {
                    max_delta = max_delta.max((water - neighbor_water).abs());
                }
            }
        }
    }
    max_delta
}

fn river_uphill_flow_neighbor_count(columns: &[HeightfieldColumn]) -> usize {
    if columns.len() < 2 {
        return 0;
    }
    let width = infer_row_width(columns);
    let height = columns.len().div_ceil(width);
    let mut count = 0usize;
    for (index, column) in columns.iter().enumerate() {
        if !matches!(column.terrain_kind, HeightfieldTerrainKind::River) {
            continue;
        }
        let Some(water) = column.river_water_height_blocks else {
            continue;
        };
        for neighbor in neighbor_indices(index, width, height).filter(|neighbor| *neighbor > index)
        {
            let neighbor_column = columns[neighbor];
            if !matches!(neighbor_column.terrain_kind, HeightfieldTerrainKind::River) {
                continue;
            }
            let Some(neighbor_water) = neighbor_column.river_water_height_blocks else {
                continue;
            };
            if neighbor_column.river_flow_hint > column.river_flow_hint + 0.01
                && neighbor_water > water
            {
                count += 1;
            } else if column.river_flow_hint > neighbor_column.river_flow_hint + 0.01
                && water > neighbor_water
            {
                count += 1;
            }
        }
    }
    count
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
            if is_standing_water(*column) || is_standing_water(*neighbor) {
                max_delta = max_delta.max(
                    (column.visible_surface_height_blocks()
                        - neighbor.visible_surface_height_blocks())
                    .abs(),
                );
            }
        }
        if index + width < columns.len() {
            let neighbor = &columns[index + width];
            if is_standing_water(*column) || is_standing_water(*neighbor) {
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
        assert_eq!(contour.min_gap_blocks, 0.0);
        assert_eq!(contour.river_min_gap_blocks, 0.0);
        assert_eq!(contour.band_smoothing, 0.0);
    }

    #[test]
    fn default_perlin_is_disabled_and_keeps_micro_relief_zero() {
        let config = HeightfieldConfig::default();
        let column =
            heightfield_column_from_sample(&sample(17.0, 29.0, 0.25, 0.0, 0.0, 0.0, 0.0), config);

        assert!(!config.perlin.enabled);
        assert_eq!(column.micro_relief_blocks, 0.0);
    }

    #[test]
    fn enabled_perlin_produces_bounded_land_micro_relief() {
        let config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let column =
            heightfield_column_from_sample(&sample(17.0, 29.0, 0.25, 0.0, 0.0, 0.0, 0.0), config);

        assert_ne!(column.micro_relief_blocks, 0.0);
        assert!(
            column.micro_relief_blocks.abs() <= config.perlin.max_abs_blocks,
            "micro relief should stay bounded: {}",
            column.micro_relief_blocks
        );
    }

    #[test]
    fn preview_perlin_applies_micro_relief_before_contour_band() {
        let config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let column =
            heightfield_column_from_sample(&sample(17.0, 29.0, 0.25, 0.0, 0.0, 0.0, 0.0), config);
        let expected = resolve_contour_band_height(
            column.raw_surface_height_blocks + column.micro_relief_blocks,
            config.contour,
        );

        assert_eq!(
            config.perlin.placement,
            HeightfieldPerlinPlacement::BeforeContour
        );
        assert_eq!(column.contour_guided_surface_height_blocks, expected);
        assert_eq!(
            column.surface_height_blocks, expected,
            "preview Perlin should perturb the contour source rather than stack after the band"
        );
    }

    #[test]
    fn enabled_perlin_keeps_micro_relief_zero_for_water_and_river_columns() {
        let config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let ocean =
            heightfield_column_from_sample(&sample(17.0, 29.0, -0.2, 1.0, 0.0, 0.0, 0.0), config);
        let lake =
            heightfield_column_from_sample(&sample(17.0, 29.0, -0.2, 0.0, 1.0, 0.0, 0.0), config);
        let river =
            heightfield_column_from_sample(&sample_with_river(17.0, 29.0, 0.25, 0.75), config);

        assert_eq!(ocean.micro_relief_blocks, 0.0);
        assert_eq!(lake.micro_relief_blocks, 0.0);
        assert_eq!(river.micro_relief_blocks, 0.0);
    }

    #[test]
    fn enabled_perlin_can_perturb_river_bed_without_moving_water_surface() {
        let mut sample = sample_with_river(37.0, -91.0, 0.25, 0.82);
        sample.river_valley_strength = 1.0;
        sample.river_bed_depth_hint = 0.55;
        sample.river_bank_roughness_hint = 0.75;
        let disabled = heightfield_column_from_sample(&sample, HeightfieldConfig::default());
        let enabled_config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let enabled = heightfield_column_from_sample(&sample, enabled_config);

        assert_ne!(
            perlin::river_bed_relief_blocks(&sample, enabled_config.perlin),
            0.0
        );
        assert_ne!(
            enabled.constrained_surface_height_blocks, disabled.constrained_surface_height_blocks,
            "river bed Perlin should perturb the terrain bed before integer snapping"
        );
        assert_eq!(
            enabled.water_level_blocks, disabled.water_level_blocks,
            "river bed Perlin should not move the river water surface"
        );
        assert_eq!(enabled.micro_relief_blocks, 0.0);
    }

    #[test]
    fn enabled_perlin_can_perturb_river_bank_surface() {
        let mut sample = sample(91.0, -37.0, 0.18, 0.0, 0.0, 0.0, 0.0);
        sample.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.45;
        sample.river_flow_hint = 0.68;
        sample.river_bank_roughness_hint = 0.9;
        sample.river_gravel_hint = 0.65;
        let disabled = heightfield_column_from_sample(&sample, HeightfieldConfig::default());
        let enabled_config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let enabled = heightfield_column_from_sample(&sample, enabled_config);

        assert_ne!(
            perlin::river_bank_relief_blocks(&sample, enabled_config.perlin),
            0.0
        );
        assert_eq!(enabled.water_level_blocks, None);
        assert_eq!(enabled.terrain_kind, HeightfieldTerrainKind::Land);
        assert_ne!(
            enabled.constrained_surface_height_blocks, disabled.constrained_surface_height_blocks,
            "river bank Perlin should perturb visible bank terrain before snapping"
        );
    }

    #[test]
    fn enabled_perlin_can_perturb_ocean_bed_without_moving_water_surface() {
        let sample = sample(53.0, -79.0, -0.18, 1.0, 0.0, 0.0, 0.0);
        let disabled = heightfield_column_from_sample(&sample, HeightfieldConfig::default());
        let enabled_config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let enabled = heightfield_column_from_sample(&sample, enabled_config);

        assert_ne!(
            perlin::ocean_bed_relief_blocks(&sample, enabled_config.perlin),
            0.0
        );
        assert_eq!(enabled.micro_relief_blocks, 0.0);
        assert_eq!(
            enabled.water_level_blocks, disabled.water_level_blocks,
            "ocean bed Perlin should not move the sea surface"
        );
        assert_ne!(
            enabled.constrained_surface_height_blocks, disabled.constrained_surface_height_blocks,
            "ocean bed Perlin should perturb terrain bed before snapping"
        );
    }

    #[test]
    fn enabled_perlin_uses_land_micro_relief_for_ocean_owned_above_sea_terrain() {
        let sample = sample(37.0, -91.0, 0.18, 1.0, 0.0, 0.0, 0.0);
        let config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let column = heightfield_column_from_sample(&sample, config);

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
        assert_eq!(column.water_level_blocks, None);
        assert_ne!(
            column.micro_relief_blocks, 0.0,
            "ocean-owned above-sea terrain should use the same micro relief map as land"
        );
    }

    #[test]
    fn sea_level_ocean_owned_border_uses_land_micro_relief() {
        let sample = sample(53.0, -79.0, 0.0, 1.0, 0.0, 0.0, 0.0);
        let config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let column = heightfield_column_from_sample(&sample, config);
        let mut land_sample = sample;
        land_sample.ocean_mask = 0.0;
        let land_column = heightfield_column_from_sample(&land_sample, config);

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
        assert_eq!(
            column.micro_relief_blocks, land_column.micro_relief_blocks,
            "exact sea-level ocean-owned border should reuse the ordinary land micro relief map"
        );
        assert_ne!(column.micro_relief_blocks, 0.0);
        assert_eq!(
            perlin::ocean_bed_relief_blocks(&sample, config.perlin),
            0.0,
            "ocean bed Perlin should not move an exact sea-level source column"
        );
    }

    #[test]
    fn shallow_ocean_owned_border_band_uses_land_micro_relief() {
        let shallow_ocean = sample(53.0, -79.0, -0.003, 1.0, 0.0, 0.0, 0.0);
        let config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let column = heightfield_column_from_sample(&shallow_ocean, config);
        let mut land_sample = shallow_ocean;
        land_sample.ocean_mask = 0.0;
        let land_column = heightfield_column_from_sample(&land_sample, config);
        let deep_ocean =
            heightfield_column_from_sample(&sample(53.0, -79.0, -0.02, 1.0, 0.0, 0.0, 0.0), config);

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
        assert_eq!(
            column.micro_relief_blocks, land_column.micro_relief_blocks,
            "shallow ocean-owned border band should reuse ordinary land micro relief"
        );
        assert_ne!(column.micro_relief_blocks, 0.0);
        assert_eq!(
            deep_ocean.micro_relief_blocks, 0.0,
            "deeper submerged ocean should stay protected from land micro relief"
        );
        assert_eq!(
            column.water_level_blocks,
            Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS),
            "land micro relief must not directly move the sea-level water surface"
        );
    }

    #[test]
    fn enabled_perlin_is_deterministic_for_same_world_position_and_config() {
        let config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let sample = sample(17.0, 29.0, 0.25, 0.0, 0.0, 0.0, 0.0);
        let first = heightfield_column_from_sample(&sample, config);
        let second = heightfield_column_from_sample(&sample, config);

        assert_eq!(first.micro_relief_blocks, second.micro_relief_blocks);
        assert_eq!(first.surface_height_blocks, second.surface_height_blocks);
    }

    #[test]
    fn contour_guided_height_is_pure_lower_band_and_snaps() {
        let config = HeightfieldConfig::default();
        let sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
        let column = heightfield_column_from_sample(&sample, config);
        let lower = (column.raw_surface_height_blocks / config.contour.step_blocks).floor()
            * config.contour.step_blocks;
        let expected =
            resolve_contour_band_height(column.raw_surface_height_blocks, config.contour);

        assert!(
            column.contour_guided_surface_height_blocks <= lower,
            "gap policy should not raise the lower contour band"
        );
        assert_eq!(column.contour_guided_surface_height_blocks, expected);
        assert_eq!(column.surface_height_blocks.fract(), 0.0);
    }

    #[test]
    fn default_contour_snap_preserves_raw_block_scale() {
        let config = HeightfieldConfig::default();
        let below_one =
            heightfield_column_from_sample(&sample(0.0, 0.0, 0.00048, 0.0, 0.0, 0.0, 0.0), config);
        let at_one =
            heightfield_column_from_sample(&sample(0.0, 0.0, 0.00049, 0.0, 0.0, 0.0, 0.0), config);
        let below_two =
            heightfield_column_from_sample(&sample(0.0, 0.0, 0.00096, 0.0, 0.0, 0.0, 0.0), config);
        let at_two =
            heightfield_column_from_sample(&sample(0.0, 0.0, 0.00098, 0.0, 0.0, 0.0, 0.0), config);

        assert!(
            below_one.raw_surface_height_blocks < 1.0,
            "test input should sit just below the first block"
        );
        assert_eq!(
            below_one.surface_height_blocks, 0.0,
            "raw 0.0..0.999 should remain visible y=0"
        );
        assert_eq!(
            at_one.surface_height_blocks, 1.0,
            "raw 1.0..1.999 should become visible y=1"
        );
        assert_eq!(
            below_two.surface_height_blocks, 1.0,
            "default contour snap must not halve raw block scale"
        );
        assert_eq!(
            at_two.surface_height_blocks, 2.0,
            "raw 2.0..2.999 should become visible y=2"
        );
    }

    #[test]
    fn default_river_corridor_uses_same_gap_but_can_cut_bed() {
        let config = HeightfieldConfig::default();
        let land =
            heightfield_column_from_sample(&sample(0.0, 0.0, 0.00110, 0.0, 0.0, 0.0, 0.0), config);
        let river =
            heightfield_column_from_sample(&sample_with_river(0.0, 0.0, 0.00110, 0.75), config);

        assert_eq!(
            config.contour.min_gap_blocks, config.contour.river_min_gap_blocks,
            "default launch slice uses the same zero-block gap for land and river corridors"
        );
        assert!(
            river.surface_height_blocks < land.surface_height_blocks,
            "river bed hint should cut the bed below the surrounding land: land={} river={}",
            land.surface_height_blocks,
            river.surface_height_blocks
        );
        assert!(
            river.water_level_blocks.is_some(),
            "river corridor should keep water separate from the carved bed"
        );
    }

    #[test]
    fn river_corridor_gap_can_still_override_general_land_gap() {
        let config = HeightfieldConfig {
            contour: HeightfieldContourConfig {
                min_gap_blocks: 4.0,
                river_min_gap_blocks: 1.0,
                ..HeightfieldContourConfig::default()
            },
            ..HeightfieldConfig::default()
        };
        let land =
            heightfield_column_from_sample(&sample(0.0, 0.0, 0.00110, 0.0, 0.0, 0.0, 0.0), config);
        let river =
            heightfield_column_from_sample(&sample_with_river(0.0, 0.0, 0.00110, 0.75), config);

        assert_eq!(
            land.surface_height_blocks, 0.0,
            "wider configured land gap should still hold ordinary terrain back"
        );
        assert!(
            river.surface_height_blocks < land.surface_height_blocks,
            "river corridors keep the smaller gap and then apply river bed carve: land={} river={}",
            land.surface_height_blocks,
            river.surface_height_blocks
        );
    }

    #[test]
    fn ordinary_land_visible_surface_preserves_raw_neighbor_jump() {
        let config = MacroFieldTileConfig::new(0.0, 0.0, 4, 1, 1.0);
        let samples = vec![
            sample(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            sample(1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
            sample(2.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
            sample(3.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
        ];
        let macro_tile = MacroFieldTile {
            config,
            samples,
            stats: MacroFieldTileStats::default(),
        };
        let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
        let heights = tile
            .columns
            .iter()
            .map(|column| column.surface_height_blocks)
            .collect::<Vec<_>>();

        assert_eq!(
            tile.column(1, 0)
                .expect("steep land")
                .raw_surface_height_blocks,
            DEFAULT_HEIGHTFIELD_MAX_BLOCKS
        );
        assert_eq!(
            heights,
            vec![
                0.0,
                DEFAULT_HEIGHTFIELD_MAX_BLOCKS,
                DEFAULT_HEIGHTFIELD_MAX_BLOCKS,
                DEFAULT_HEIGHTFIELD_MAX_BLOCKS
            ]
        );
        assert!(
            tile.stats.max_visible_neighbor_delta_blocks > 1.0,
            "ordinary land should preserve raw-scale jumps when the source field jumps"
        );
    }

    #[test]
    fn launch_relief_scale_uses_experimental_large_block_domain() {
        let high = heightfield_column_from_sample(
            &sample(
                0.0,
                0.0,
                DEFAULT_HEIGHTFIELD_NORMALIZED_MAX,
                0.0,
                0.0,
                0.0,
                0.0,
            ),
            HeightfieldConfig::default(),
        );

        assert_eq!(DEFAULT_HEIGHTFIELD_NORMALIZED_MIN, -0.5);
        assert_eq!(DEFAULT_HEIGHTFIELD_NORMALIZED_MAX, 1.0);
        assert_eq!(DEFAULT_HEIGHTFIELD_MAX_BLOCKS, 2048.0);
        assert_eq!(DEFAULT_HEIGHTFIELD_MIN_BLOCKS, -1024.0);
        assert_eq!(
            high.raw_surface_height_blocks, DEFAULT_HEIGHTFIELD_MAX_BLOCKS,
            "macro relief should use the experimental heightfield block-domain resolution"
        );
    }

    #[test]
    fn experimental_interest_range_maps_to_large_block_span() {
        let config = HeightfieldConfig::default();
        let low =
            heightfield_column_from_sample(&sample(0.0, 0.0, -0.25, 0.0, 0.0, 0.0, 0.0), config);
        let high =
            heightfield_column_from_sample(&sample(0.0, 0.0, 0.75, 0.0, 0.0, 0.0, 0.0), config);

        assert_eq!(low.raw_surface_height_blocks, -512.0);
        assert_eq!(high.raw_surface_height_blocks, 1536.0);
    }

    #[test]
    fn experimental_effective_range_saturates_outside_limits() {
        let config = HeightfieldConfig::default();
        let low =
            heightfield_column_from_sample(&sample(0.0, 0.0, -0.75, 0.0, 0.0, 0.0, 0.0), config);
        let high =
            heightfield_column_from_sample(&sample(0.0, 0.0, 1.25, 0.0, 0.0, 0.0, 0.0), config);

        assert_eq!(
            low.raw_surface_height_blocks,
            DEFAULT_HEIGHTFIELD_MIN_BLOCKS
        );
        assert_eq!(
            high.raw_surface_height_blocks,
            DEFAULT_HEIGHTFIELD_MAX_BLOCKS
        );
    }

    #[test]
    fn signed_macro_zero_maps_to_sea_level_before_contour_snap() {
        let column = heightfield_column_from_sample(
            &sample(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            HeightfieldConfig::default(),
        );

        assert_eq!(
            column.raw_surface_height_blocks,
            DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
        );
        assert_eq!(
            column.surface_height_blocks,
            DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
        );
    }

    #[test]
    fn small_positive_coastal_macro_height_starts_near_sea_level() {
        let column = heightfield_column_from_sample(
            &sample(0.0, 0.0, 0.0008, 0.0, 0.0, 0.0, 0.0),
            HeightfieldConfig::default(),
        );

        assert!(
            column.raw_surface_height_blocks <= 2.0,
            "signed macro height just above sea level should not become a high terrace: {}",
            column.raw_surface_height_blocks
        );
        assert!(
            column.surface_height_blocks <= 2.0,
            "signed macro height just above sea level should snap to the first few contour steps: {}",
            column.surface_height_blocks
        );
    }

    #[test]
    fn changing_contour_step_snaps_land_surface_to_step_multiples() {
        let sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
        let column = heightfield_column_from_sample(
            &sample,
            HeightfieldConfig {
                contour: HeightfieldContourConfig {
                    step_blocks: 4.0,
                    min_gap_blocks: 0.0,
                    river_min_gap_blocks: 0.0,
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
    fn coast_mask_alone_does_not_smooth_contour_terrace() {
        let mut sample = sample(0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0);
        sample.coast_mask = 1.0;
        let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());
        let expected = resolve_contour_band_height(
            column.raw_surface_height_blocks,
            HeightfieldConfig::default().contour,
        );

        assert_eq!(column.surface_height_blocks, expected);
        assert_eq!(
            column.terrain_kind,
            HeightfieldTerrainKind::Land,
            "coast_mask is preserved as data but should not create a heightfield-specific terrain kind"
        );
        assert_eq!(
            column.water_y, None,
            "coast_mask alone should not create a water column"
        );
    }

    #[test]
    fn coast_mask_negative_non_ocean_land_floors_without_water() {
        let mut coast = sample(0.0, 0.0, -0.25, 0.0, 0.0, 0.0, 0.0);
        coast.coast_mask = 0.35;
        let column = heightfield_column_from_sample(&coast, HeightfieldConfig::default());

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Land);
        assert_eq!(column.surface_y, 0);
        assert_eq!(column.water_y, None);
        assert_eq!(
            column.visible_surface_height_blocks(),
            DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
        );
    }

    #[test]
    fn non_coast_near_zero_land_keeps_ordinary_contour_behavior() {
        let land = sample(0.0, 0.0, 0.0004, 0.0, 0.0, 0.0, 0.0);
        let column = heightfield_column_from_sample(&land, HeightfieldConfig::default());

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Land);
        assert_eq!(column.surface_y, 0);
        assert_eq!(column.water_y, None);
    }

    #[test]
    fn coast_near_zero_land_uses_ordinary_contour_behavior() {
        let mut coast = sample(0.0, 0.0, 0.0004, 0.0, 0.0, 0.0, 0.0);
        coast.coast_mask = 1.0;
        let column = heightfield_column_from_sample(&coast, HeightfieldConfig::default());

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Land);
        assert_eq!(column.surface_y, 0);
        assert_eq!(column.water_y, None);
        assert_eq!(column.coast_mask, 1.0);
    }

    #[test]
    fn inland_negative_non_water_land_still_floors_at_sea_level() {
        let inland = sample(0.0, 0.0, -0.25, 0.0, 0.0, 0.0, 0.0);
        let column = heightfield_column_from_sample(&inland, HeightfieldConfig::default());

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Land);
        assert_eq!(column.surface_y, 0);
        assert_eq!(column.water_y, None);
    }

    #[test]
    fn water_columns_appear_for_submerged_ocean_and_lake_masks() {
        let tile = generate_heightfield_tile(&test_macro_tile(), HeightfieldConfig::default());

        assert!(tile.stats.ocean_column_count > 0);
        assert!(tile.stats.lake_column_count > 0);
        assert!(tile.stats.water_column_count > 0);
        assert!(tile
            .columns
            .iter()
            .filter(
                |column| matches!(column.terrain_kind, HeightfieldTerrainKind::Lake)
                    || (matches!(column.terrain_kind, HeightfieldTerrainKind::Ocean)
                        && column.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS)
            )
            .all(|column| column.water_level_blocks.is_some()));
        assert!(tile
            .columns
            .iter()
            .filter(
                |column| matches!(column.terrain_kind, HeightfieldTerrainKind::Ocean)
                    && column.surface_height_blocks >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
            )
            .all(|column| column.water_level_blocks.is_none()));
    }

    #[test]
    fn ocean_owned_positive_terrain_bed_has_no_water_column() {
        let column = heightfield_column_from_sample(
            &sample(0.0, 0.0, 0.01, 1.0, 0.0, 0.0, 0.0),
            HeightfieldConfig::default(),
        );

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
        assert!(
            column.surface_height_blocks >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
            "positive ocean-owned terrain bed should stay above sea level: {}",
            column.surface_height_blocks
        );
        assert_eq!(
            column.water_level_blocks, None,
            "water should only cover ocean-owned terrain when the final bed is below sea level"
        );
        assert_eq!(column.water_y, None);
    }

    #[test]
    fn ocean_owned_negative_terrain_bed_has_sea_level_water() {
        let column = heightfield_column_from_sample(
            &sample(0.0, 0.0, -0.01, 1.0, 0.0, 0.0, 0.0),
            HeightfieldConfig::default(),
        );

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
        assert!(
            column.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
            "negative ocean-owned terrain bed should stay below sea level: {}",
            column.surface_height_blocks
        );
        assert_eq!(
            column.water_level_blocks,
            Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS)
        );
        assert_eq!(column.water_y, Some(0));
    }

    #[test]
    fn ocean_owned_positive_bed_with_perlin_has_no_water_column() {
        let sample = sample(53.0, -79.0, 0.01, 1.0, 0.0, 0.0, 0.0);
        let config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let column = heightfield_column_from_sample(&sample, config);
        let mut land_sample = sample;
        land_sample.ocean_mask = 0.0;
        let land_column = heightfield_column_from_sample(&land_sample, config);

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
        assert_eq!(
            column.micro_relief_blocks, land_column.micro_relief_blocks,
            "above-sea ocean-owned dry terrain should use the same micro relief map as land"
        );
        assert_ne!(column.micro_relief_blocks, 0.0);
        assert_eq!(column.water_level_blocks, None);
        assert_eq!(column.water_y, None);
    }

    #[test]
    fn ocean_owned_positive_bed_uses_land_perlin_not_ocean_bed_perlin() {
        let sample = sample(53.0, -79.0, 0.01, 1.0, 0.0, 0.0, 0.0);
        let enabled_config = HeightfieldConfig {
            perlin: HeightfieldPerlinConfig::preview_enabled(42, 1),
            ..HeightfieldConfig::default()
        };
        let disabled = heightfield_column_from_sample(&sample, HeightfieldConfig::default());
        let enabled = heightfield_column_from_sample(&sample, enabled_config);
        let mut land_sample = sample;
        land_sample.ocean_mask = 0.0;
        let land_enabled = heightfield_column_from_sample(&land_sample, enabled_config);

        assert_eq!(
            perlin::ocean_bed_relief_blocks(&sample, enabled_config.perlin),
            0.0,
            "ocean bed Perlin should be bed-only and stay inactive for above-sea ocean beds"
        );
        assert_eq!(disabled.water_level_blocks, None);
        assert_eq!(enabled.water_level_blocks, None);
        assert_eq!(enabled.water_y, None);
        assert_eq!(
            enabled.micro_relief_blocks, land_enabled.micro_relief_blocks,
            "above-sea ocean-owned dry terrain should reuse land micro relief"
        );
        assert_ne!(
            enabled.micro_relief_blocks, 0.0,
            "land-style Perlin should be present even if integer contour snapping keeps this column in the same band"
        );
    }

    #[test]
    fn water_columns_do_not_appear_for_ocean_owned_above_sea_beds() {
        let config = MacroFieldTileConfig::new(0.0, 0.0, 2, 1, 32.0);
        let macro_tile = MacroFieldTile {
            config,
            samples: vec![
                sample(0.0, 0.0, 0.01, 1.0, 0.0, 0.0, 0.0),
                sample(32.0, 0.0, -0.01, 1.0, 0.0, 0.0, 0.0),
            ],
            stats: MacroFieldTileStats::default(),
        };

        let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());

        assert_eq!(tile.column(0, 0).expect("above sea ocean").water_y, None);
        assert_eq!(tile.column(1, 0).expect("below sea ocean").water_y, Some(0));
        assert_eq!(tile.stats.water_column_count, 1);
    }

    #[test]
    fn ocean_water_surface_is_sea_level_while_bed_preserves_source_height() {
        let tile = generate_heightfield_tile(&test_macro_tile(), HeightfieldConfig::default());

        assert!(tile.stats.ocean_column_count > 0);
        assert_eq!(
            tile.stats.min_ocean_visible_surface_blocks,
            DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
        );
        assert_eq!(
            tile.stats.max_ocean_visible_surface_blocks,
            DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
        );
        assert!(tile
            .columns
            .iter()
            .filter(|column| matches!(column.terrain_kind, HeightfieldTerrainKind::Ocean))
            .all(|column| {
                column.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
                    && column.water_level_blocks == Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS)
                    && column.visible_surface_height_blocks()
                        == DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
            }));
    }

    #[test]
    fn ocean_negative_source_bed_stays_below_sea_level_with_water_to_zero() {
        let ocean = sample(0.0, 0.0, -0.25, 1.0, 0.0, 0.0, 0.0);
        let column = heightfield_column_from_sample(&ocean, HeightfieldConfig::default());

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
        assert!(
            column.surface_y < 0,
            "ocean bed should preserve negative source terrain instead of clamping to sea level: {}",
            column.surface_y
        );
        assert_eq!(column.water_y, Some(0));
        assert_eq!(
            column.visible_surface_height_blocks(),
            DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS
        );
    }

    #[test]
    fn ocean_nonnegative_source_bed_is_preserved_without_fallback() {
        let column = heightfield_column_from_sample(
            &sample(0.0, 0.0, 0.01, 1.0, 0.0, 0.0, 0.0),
            HeightfieldConfig::default(),
        );

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
        assert_eq!(
            column.water_level_blocks, None,
            "ocean-owned terrain above sea level should not create a water column"
        );
        assert!(
            column.surface_height_blocks >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
            "heightfield should preserve source ocean bed directly instead of inventing a below-sea fallback: {}",
            column.surface_height_blocks
        );
    }

    #[test]
    fn lake_water_surface_uses_lake_bed_depth_instead_of_absolute_sea_level() {
        let mut lake = sample(0.0, 0.0, 0.05, 0.0, 1.0, 0.0, 0.0);
        lake.combined_macro_height = 0.04;
        let column = heightfield_column_from_sample(&lake, HeightfieldConfig::default());

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Lake);
        assert!(
            column.water_level_blocks.expect("lake water") > DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
            "inland lake water should derive from the lake source elevation, not y=0"
        );
        assert!(
            column.surface_height_blocks < column.water_level_blocks.expect("lake water"),
            "lake bed should sit below the water surface"
        );
        assert!(
            column.water_level_blocks.expect("lake water") < column.raw_surface_height_blocks + 8.0,
            "lake water should stay tied to the carved bed instead of riding high on the source terrain"
        );
        assert_eq!(
            column.visible_surface_height_blocks(),
            column.water_level_blocks.expect("lake water")
        );
    }

    #[test]
    fn lake_bed_preserves_local_relief_instead_of_flattening_to_one_plane() {
        let mut shallow = sample(0.0, 0.0, 0.05, 0.0, 1.0, 0.0, 0.0);
        shallow.combined_macro_height = 0.047;
        let mut deep = sample(1.0, 0.0, 0.05, 0.0, 1.0, 0.0, 0.0);
        deep.combined_macro_height = 0.038;

        let shallow = heightfield_column_from_sample(&shallow, HeightfieldConfig::default());
        let deep = heightfield_column_from_sample(&deep, HeightfieldConfig::default());

        assert_eq!(shallow.water_y, deep.water_y);
        assert!(
            deep.surface_height_blocks < shallow.surface_height_blocks,
            "lake bed should preserve U-shaped bed variation instead of a completely flat floor: shallow={} deep={}",
            shallow.surface_height_blocks,
            deep.surface_height_blocks
        );
        assert!(
            shallow.water_y.expect("lake water") - deep.surface_y <= 24,
            "lake bed carve should stay depth-capped instead of making an abrupt shaft"
        );
    }

    #[test]
    fn coast_adjacent_land_preserves_contour_band_without_extra_clamp() {
        let config = MacroFieldTileConfig::new(0.0, 0.0, 3, 1, 32.0);
        let samples = vec![
            sample(0.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0),
            sample(32.0, 0.0, 0.00049, 0.0, 0.0, 0.0, 0.0),
            sample(64.0, 0.0, 0.00098, 0.0, 0.0, 0.0, 0.0),
        ];
        let macro_tile = MacroFieldTile {
            config,
            samples,
            stats: MacroFieldTileStats::default(),
        };
        let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
        let coast = tile.column(1, 0).expect("coast column");
        let inland = tile.column(2, 0).expect("inland column");

        assert_eq!(coast.surface_height_blocks, 1.0);
        assert_eq!(inland.surface_height_blocks, 2.0);
        assert!(coast.surface_height_blocks > DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS);
        assert!(inland.surface_height_blocks > coast.surface_height_blocks);
        assert_eq!(
            tile.stats.max_shore_visible_neighbor_delta_blocks,
            DEFAULT_HEIGHTFIELD_CONTOUR_STEP_BLOCKS,
            "standing-water shoreline should only reflect the ordinary contour step"
        );
    }

    #[test]
    fn water_visible_top_uses_water_surface_not_bed() {
        let sample = sample(0.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0);
        let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());

        assert!(
            column.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
            "standing ocean bed should stay below the visible water surface: {}",
            column.surface_height_blocks
        );
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
    fn final_visible_heights_are_integer_blocks() {
        let tile = generate_heightfield_tile(&test_macro_tile(), HeightfieldConfig::default());

        assert!(tile.columns.iter().all(|column| {
            column.visible_surface_height_blocks().fract() == 0.0
                && column
                    .water_level_blocks
                    .is_none_or(|water| water.fract() == 0.0)
                && column
                    .river_water_height_blocks
                    .is_none_or(|water| water.fract() == 0.0)
        }));
    }

    #[test]
    fn river_water_steps_down_to_standing_water_without_large_jumps() {
        let config = MacroFieldTileConfig::new(0.0, 0.0, 5, 1, 32.0);
        let samples = vec![
            sample_with_river(0.0, 0.0, 1.0, 0.22),
            sample_with_river(32.0, 0.0, 0.85, 0.42),
            sample_with_river(64.0, 0.0, 0.70, 0.62),
            sample_with_river(96.0, 0.0, 0.55, 0.82),
            sample(128.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0),
        ];
        let macro_tile = MacroFieldTile {
            config,
            samples,
            stats: MacroFieldTileStats::default(),
        };
        let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
        let river_water = (0..4)
            .map(|x| {
                tile.column(x, 0)
                    .expect("river column")
                    .river_water_height_blocks
                    .expect("river water")
            })
            .collect::<Vec<_>>();

        assert_eq!(
            tile.column(4, 0).expect("ocean").water_level_blocks,
            Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS)
        );
        assert!(
            river_water.windows(2).all(|pair| pair[0] >= pair[1]),
            "river water should not climb as display flow increases: {river_water:?}"
        );
        assert!(
            river_water
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() <= 1.0),
            "river water should descend by at most one block per neighboring sample: {river_water:?}"
        );
        assert!(
            tile.stats.max_river_water_neighbor_delta_blocks <= 1.0,
            "river water neighbor delta should be constrained: {}",
            tile.stats.max_river_water_neighbor_delta_blocks
        );
        assert_eq!(tile.stats.river_uphill_flow_neighbor_count, 0);
    }

    #[test]
    fn headwater_river_bed_stays_shallow_and_nearly_filled() {
        let mut headwater = sample_with_river(0.0, 0.0, 0.004, 0.05);
        headwater.river_bed_depth_hint = 3.0 / 40.0;
        let column = heightfield_column_from_sample(&headwater, HeightfieldConfig::default());

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::River);
        assert!(
            column.river_bed_depth_blocks <= 3.0,
            "headwater stream carve should stay within the small V-cut range: {}",
            column.river_bed_depth_blocks
        );
        assert!(
            column.water_y.expect("river water") - column.surface_y <= 2,
            "shallow headwater water should sit close to the surrounding bed instead of exposing a deep trench: bed={} water={:?}",
            column.surface_y,
            column.water_y
        );
    }

    #[test]
    fn river_bed_depth_scales_with_flow_hint() {
        let mut headwater = sample_with_river(0.0, 0.0, 0.02, 0.05);
        headwater.river_bed_depth_hint = 3.0 / 40.0;
        let mut lower = sample_with_river(1.0, 0.0, 0.02, 0.85);
        lower.river_bed_depth_hint = 18.0 / 40.0;

        let headwater = heightfield_column_from_sample(&headwater, HeightfieldConfig::default());
        let lower = heightfield_column_from_sample(&lower, HeightfieldConfig::default());

        assert!(
            lower.river_bed_depth_blocks > headwater.river_bed_depth_blocks * 3.0,
            "downstream river bed depth should follow river-plan Q scale: headwater={} lower={}",
            headwater.river_bed_depth_blocks,
            lower.river_bed_depth_blocks
        );
        assert!(
            lower.water_y.expect("lower water") - lower.surface_y
                > headwater.water_y.expect("headwater water") - headwater.surface_y,
            "larger Q should allow deeper water while keeping the surface separate from bed"
        );
    }

    #[test]
    fn deterministic_river_bed_variation_does_not_move_water_surface() {
        let mut left = sample_with_river(37.0, -91.0, 0.20, 0.62);
        left.river_bed_depth_hint = 0.46;
        left.river_bank_roughness_hint = 0.80;
        left.river_gravel_hint = 0.55;
        let mut right = left;
        right.position = WorldPlanePoint::new(53.0, -91.0);

        let left_variation = deterministic_river_bed_variation_blocks(&left);
        let right_variation = deterministic_river_bed_variation_blocks(&right);
        let left_column = heightfield_column_from_sample(&left, HeightfieldConfig::default());
        let right_column = heightfield_column_from_sample(&right, HeightfieldConfig::default());

        assert_ne!(
            left_variation, right_variation,
            "river bed variation should be deterministic but not uniform across neighboring bed samples"
        );
        assert_eq!(
            left_column.water_y, right_column.water_y,
            "bed variation must not perturb the river water surface"
        );
    }

    #[test]
    fn deterministic_river_bank_variation_is_available_by_default() {
        let mut bank = sample_with_river(41.0, 19.0, 0.24, 0.42);
        bank.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD * 0.35;
        bank.river_bank_roughness_hint = 0.90;
        bank.river_gravel_hint = 0.60;

        assert_ne!(
            deterministic_river_bank_variation_blocks(&bank),
            0.0,
            "river shoulders should have deterministic default relief even when optional Perlin is disabled"
        );
    }

    #[test]
    fn river_bed_hint_can_cut_below_sea_level_at_ocean_mouth() {
        let config = MacroFieldTileConfig::new(0.0, 0.0, 2, 1, 1.0);
        let mut river = sample_with_river(0.0, 0.0, 0.0, 0.95);
        river.river_bed_depth_hint = 0.85;
        river.river_bank_roughness_hint = 0.2;
        let samples = vec![river, sample(1.0, 0.0, -0.8, 1.0, 0.0, 0.0, 0.0)];
        let macro_tile = MacroFieldTile {
            config,
            samples,
            stats: MacroFieldTileStats::default(),
        };

        let tile = generate_heightfield_tile(&macro_tile, HeightfieldConfig::default());
        let river = tile.column(0, 0).expect("river mouth");
        let ocean = tile.column(1, 0).expect("ocean");

        assert_eq!(ocean.visible_surface_height_blocks(), 0.0);
        assert_eq!(river.terrain_kind, HeightfieldTerrainKind::River);
        assert!(
            river.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
            "river bed should stay carved below sea level near the mouth: {}",
            river.surface_height_blocks
        );
        assert!(
            river.water_level_blocks.unwrap_or_default() >= DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
            "river water surface should remain distinct from the carved bed"
        );
        assert!(
            river.river_bed_depth_blocks > 0.0,
            "heightfield should preserve the river bed depth diagnostic"
        );
    }

    #[test]
    fn selected_river_bed_hint_can_cut_ocean_column_below_sea_level() {
        let mut mouth = sample_with_river(0.0, 0.0, -0.25, 0.98);
        mouth.ocean_mask = 1.0;
        mouth.river_bed_depth_hint = 0.9;

        let column = heightfield_column_from_sample(&mouth, HeightfieldConfig::default());

        assert_eq!(column.terrain_kind, HeightfieldTerrainKind::Ocean);
        assert_eq!(
            column.water_level_blocks,
            Some(DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS),
            "ocean mouth water surface should stay at sea level"
        );
        assert!(
            column.surface_height_blocks < DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
            "selected river bed hint should carve the ocean-mouth bed below y=0: {}",
            column.surface_height_blocks
        );
        assert_eq!(
            column.visible_surface_height_blocks(),
            DEFAULT_HEIGHTFIELD_SEA_LEVEL_BLOCKS,
            "visible standing-water top remains the sea surface, not the carved bed"
        );
    }

    #[test]
    fn raw_and_contour_heights_are_recorded_before_snap() {
        let mut sample = sample(0.0, 0.0, 0.123, 0.0, 0.0, 0.0, 0.0);
        sample.coast_mask = 0.65;
        let column = heightfield_column_from_sample(&sample, HeightfieldConfig::default());

        assert_ne!(
            column.raw_surface_height_blocks,
            column.surface_height_blocks
        );
        assert_eq!(
            column.constrained_surface_height_blocks,
            column.contour_guided_surface_height_blocks
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
            biome_context: None,
            biome: None,
            macro_elevation: height,
            ocean_mask: ocean,
            coast_mask: 0.0,
            lake_mask: lake,
            dry_basin_mask: dry,
            ridge_influence: ridge,
            river_valley_strength: 0.0,
            river_distance_blocks: f32::INFINITY,
            river_flow_hint: 0.0,
            river_bed_depth_hint: 0.0,
            river_bank_roughness_hint: 0.0,
            river_gravel_hint: 0.0,
            river_cutbank_hint: 0.0,
            combined_macro_height: height,
        }
    }

    fn sample_with_river(x: f32, z: f32, height: f32, flow_hint: f32) -> MacroFieldSample {
        let mut sample = sample(x, z, height, 0.0, 0.0, 0.0, 0.0);
        sample.river_valley_strength = DEFAULT_HEIGHTFIELD_RIVER_WATER_THRESHOLD;
        sample.river_flow_hint = flow_hint;
        sample
    }
}
