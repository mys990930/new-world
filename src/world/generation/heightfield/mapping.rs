use super::super::macro_field::MacroFieldSample;
use super::{HeightfieldConfig, HeightfieldContourConfig};

pub(super) fn normalized_to_blocks(value: f32, config: HeightfieldConfig) -> f32 {
    if value >= 0.0 {
        let t = (value / config.normalized_max_height.max(f32::EPSILON)).clamp(0.0, 1.0);
        config.sea_level_blocks + (config.max_height_blocks - config.sea_level_blocks) * t
    } else {
        let t = (value / config.normalized_min_height.min(-f32::EPSILON)).clamp(0.0, 1.0);
        config.sea_level_blocks + (config.min_height_blocks - config.sea_level_blocks) * t
    }
}

pub(super) fn resolve_contour_band_height(value: f32, contour: HeightfieldContourConfig) -> f32 {
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

pub(super) fn contour_config_for_sample(
    sample: &MacroFieldSample,
    config: HeightfieldConfig,
) -> HeightfieldContourConfig {
    let mut contour = config.contour;
    if sample.river_core_strength >= config.river_water_threshold * 0.5
        && sample.river_flow_hint > 0.0
    {
        contour.min_gap_blocks = contour
            .min_gap_blocks
            .min(contour.river_min_gap_blocks.max(0.0));
    }
    contour
}

pub(super) fn snap_to_contour_step(value: f32, contour: HeightfieldContourConfig) -> f32 {
    if contour.step_blocks <= 0.0 {
        return value;
    }
    let step = contour.step_blocks;
    (value / step).floor() * step
}

pub(super) fn snap_height_to_block(value: f32) -> i32 {
    value.floor() as i32
}
