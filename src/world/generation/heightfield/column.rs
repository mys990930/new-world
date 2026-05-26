use super::super::macro_field::MacroFieldSample;
use super::super::macro_map::MacroSurfaceKind;
use super::mapping::{
    contour_config_for_sample, normalized_to_blocks, resolve_contour_band_height,
    snap_height_to_block, snap_to_contour_step,
};
use super::water::{lake_bed_height_blocks, lake_water_level_blocks, ocean_bed_height_blocks};
use super::{
    HeightfieldColumn, HeightfieldConfig, HeightfieldPerlinPlacement, HeightfieldTerrainKind,
    perlin, validate_heightfield_config,
};

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
    let is_land_side_coast = matches!(
        sample.surface_kind,
        Some(MacroSurfaceKind::CoastLand | MacroSurfaceKind::CoastIsland)
    );
    let surface_height_blocks = match config.perlin.placement {
        HeightfieldPerlinPlacement::BeforeContour => contour_guided_surface_height_blocks,
        HeightfieldPerlinPlacement::AfterContourBeforeSnap => {
            contour_guided_surface_height_blocks + meso_delta_blocks + micro_relief_blocks
        }
    };
    let lake_water_level_blocks = is_lake.then(|| lake_water_level_blocks(sample, config));
    let is_dry_basin = sample.dry_basin_mask > 0.5;
    let ocean_bed_relief_blocks = if is_ocean && surface_height_blocks < config.sea_level_blocks {
        let max_raise_blocks = (config.sea_level_blocks - surface_height_blocks - 1.0).max(0.0);
        perlin::ocean_bed_relief_blocks(sample, config.perlin)
            .clamp(-config.perlin.max_abs_blocks, max_raise_blocks)
    } else {
        0.0
    };
    let surface_height_blocks = if is_ocean {
        ocean_bed_height_blocks(surface_height_blocks + ocean_bed_relief_blocks, config)
    } else if is_lake {
        lake_water_level_blocks
            .map(|water| lake_bed_height_blocks(surface_height_blocks, water, config))
            .unwrap_or(surface_height_blocks)
    } else {
        surface_height_blocks
    };
    let constrained_surface_height_blocks =
        surface_height_blocks.clamp(config.min_height_blocks, config.max_height_blocks);
    let final_surface_height_blocks =
        snap_to_contour_step(constrained_surface_height_blocks, contour);
    let surface_y = snap_height_to_block(final_surface_height_blocks);
    let surface_height_blocks = surface_y as f32;
    let ocean_water_level_blocks = (is_ocean && surface_height_blocks < config.sea_level_blocks)
        .then_some(snap_height_to_block(config.sea_level_blocks) as f32);
    let coast_water_level_blocks = (is_land_side_coast
        && surface_height_blocks < config.sea_level_blocks)
        .then_some(snap_height_to_block(config.sea_level_blocks) as f32);
    let water_level_blocks = if is_lake {
        Some(
            snap_height_to_block(lake_water_level_blocks.unwrap_or(config.sea_level_blocks)) as f32,
        )
    } else if is_ocean {
        ocean_water_level_blocks
    } else if is_land_side_coast {
        coast_water_level_blocks
    } else {
        None
    };
    let water_y = water_level_blocks.map(snap_height_to_block);
    let terrain_kind = if is_lake {
        HeightfieldTerrainKind::Lake
    } else if is_ocean {
        HeightfieldTerrainKind::Ocean
    } else if is_dry_basin {
        HeightfieldTerrainKind::DryBasin
    } else if is_land_side_coast {
        HeightfieldTerrainKind::Coast
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
        river_core_water_height_blocks: None,
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
        river_core_strength: 0.0,
        river_shoulder_strength: 0.0,
        river_valley_strength: 0.0,
        river_distance_blocks: 0.0,
        river_flow_hint: 0.0,
        river_core_depth_blocks: 0.0,
        river_bank_roughness_hint: 0.0,
        river_gravel_hint: 0.0,
        river_cutbank_hint: 0.0,
        meso_delta_blocks,
        micro_relief_blocks,
    }
}

#[cfg(test)]
#[path = "column_tests.rs"]
mod tests;
