use super::super::macro_field::MacroFieldSample;
use super::HeightfieldConfig;
use super::mapping::normalized_to_blocks;

pub(super) fn lake_water_level_blocks(sample: &MacroFieldSample, config: HeightfieldConfig) -> f32 {
    let source_level = normalized_to_blocks(sample.macro_elevation, config);
    let shoreline_margin = config.lake_bed_blocks.abs().max(2.0) * 8.0;

    (source_level - shoreline_margin)
        .max(config.sea_level_blocks + 1.0)
        .clamp(config.min_height_blocks, config.max_height_blocks)
}

pub(super) fn ocean_bed_height_blocks(
    source_bed_height_blocks: f32,
    _config: HeightfieldConfig,
) -> f32 {
    source_bed_height_blocks
}

pub(super) fn lake_bed_height_blocks(
    raw_bed_height_blocks: f32,
    water_level_blocks: f32,
    config: HeightfieldConfig,
) -> f32 {
    let shallow_gap = config.lake_bed_blocks.abs().max(1.0);
    let max_depth = shallow_gap * 12.0;
    raw_bed_height_blocks.max(water_level_blocks - max_depth)
}
