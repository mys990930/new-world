use super::super::graph::WorldPlanePoint;
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

pub(super) fn heightfield_value_noise_2d(
    position: WorldPlanePoint,
    scale_blocks: f32,
    salt: u64,
) -> f32 {
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

pub(super) fn signed_lattice_noise(x: i32, z: i32, salt: u64) -> f32 {
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

pub(super) fn smoothstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
