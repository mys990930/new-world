use super::super::macro_field::MacroFieldSample;

pub const DEFAULT_HEIGHTFIELD_PERLIN_AMPLITUDE_BLOCKS: f32 = 8.0;
pub const DEFAULT_HEIGHTFIELD_PERLIN_BASE_SCALE_BLOCKS: f32 = 72.0;
pub const DEFAULT_HEIGHTFIELD_PERLIN_OCTAVES: u8 = 4;
pub const DEFAULT_HEIGHTFIELD_PERLIN_PERSISTENCE: f32 = 0.5;
pub const DEFAULT_HEIGHTFIELD_PERLIN_LACUNARITY: f32 = 2.0;
pub const DEFAULT_HEIGHTFIELD_PERLIN_MAX_ABS_BLOCKS: f32 = 10.0;
const NEGATIVE_MACRO_HEIGHT_BLOCKS_PER_UNIT: f32 = 2048.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightfieldPerlinPlacement {
    AfterContourBeforeSnap,
    BeforeContour,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightfieldPerlinConfig {
    pub enabled: bool,
    pub seed: u64,
    pub generator_version: u32,
    pub amplitude_blocks: f32,
    pub base_scale_blocks: f32,
    pub octaves: u8,
    pub persistence: f32,
    pub lacunarity: f32,
    pub max_abs_blocks: f32,
    pub placement: HeightfieldPerlinPlacement,
}

impl HeightfieldPerlinConfig {
    pub fn disabled(seed: u64, generator_version: u32) -> Self {
        Self {
            enabled: false,
            seed,
            generator_version,
            ..Self::default()
        }
    }

    pub fn preview_enabled(seed: u64, generator_version: u32) -> Self {
        Self {
            enabled: true,
            seed,
            generator_version,
            placement: HeightfieldPerlinPlacement::BeforeContour,
            ..Self::default()
        }
    }
}

impl Default for HeightfieldPerlinConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            seed: 0,
            generator_version: 0,
            amplitude_blocks: DEFAULT_HEIGHTFIELD_PERLIN_AMPLITUDE_BLOCKS,
            base_scale_blocks: DEFAULT_HEIGHTFIELD_PERLIN_BASE_SCALE_BLOCKS,
            octaves: DEFAULT_HEIGHTFIELD_PERLIN_OCTAVES,
            persistence: DEFAULT_HEIGHTFIELD_PERLIN_PERSISTENCE,
            lacunarity: DEFAULT_HEIGHTFIELD_PERLIN_LACUNARITY,
            max_abs_blocks: DEFAULT_HEIGHTFIELD_PERLIN_MAX_ABS_BLOCKS,
            placement: HeightfieldPerlinPlacement::AfterContourBeforeSnap,
        }
    }
}

pub fn micro_relief_blocks(sample: &MacroFieldSample, config: HeightfieldPerlinConfig) -> f32 {
    if !config.enabled || config.amplitude_blocks <= 0.0 || config.max_abs_blocks <= 0.0 {
        return 0.0;
    }
    if sample.lake_mask > 0.5 || is_submerged_ocean_source(sample, config) {
        return 0.0;
    }
    if sample.river_valley_strength > 0.5 && sample.river_flow_hint > 0.0 {
        return 0.0;
    }

    let octaves = config.octaves.max(1);
    let mut frequency = 1.0 / config.base_scale_blocks.max(f32::EPSILON);
    let mut amplitude = 1.0;
    let mut amplitude_sum = 0.0;
    let mut value = 0.0;

    for octave in 0..octaves {
        value += perlin_2d(
            sample.position.x * frequency,
            sample.position.z * frequency,
            config.seed,
            config.generator_version,
            octave,
        ) * amplitude;
        amplitude_sum += amplitude;
        amplitude *= config.persistence;
        frequency *= config.lacunarity;
    }

    if amplitude_sum <= f32::EPSILON {
        return 0.0;
    }

    let normalized = (value / amplitude_sum).clamp(-1.0, 1.0);
    (normalized * config.amplitude_blocks).clamp(-config.max_abs_blocks, config.max_abs_blocks)
}

pub fn river_bed_relief_blocks(sample: &MacroFieldSample, config: HeightfieldPerlinConfig) -> f32 {
    if !config.enabled || config.amplitude_blocks <= 0.0 || config.max_abs_blocks <= 0.0 {
        return 0.0;
    }
    if sample.lake_mask > 0.5 || is_submerged_ocean_source(sample, config) {
        return 0.0;
    }
    if sample.river_valley_strength <= 0.5 || sample.river_flow_hint <= 0.0 {
        return 0.0;
    }

    let river_t = sample.river_valley_strength.clamp(0.0, 1.0);
    let bed_t = sample.river_bed_depth_hint.clamp(0.0, 1.0);
    let rough_t = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    let amplitude = (config.amplitude_blocks * (0.72 + rough_t * 0.58 + bed_t * 0.48))
        .min(config.max_abs_blocks * 1.35)
        .max(0.0)
        * river_t;
    if amplitude <= f32::EPSILON {
        return 0.0;
    }

    octave_noise_2d(sample.position.x, sample.position.z, config, 0.62, 0x71) * amplitude
}

pub fn river_bank_relief_blocks(sample: &MacroFieldSample, config: HeightfieldPerlinConfig) -> f32 {
    if !config.enabled || config.amplitude_blocks <= 0.0 || config.max_abs_blocks <= 0.0 {
        return 0.0;
    }
    if sample.lake_mask > 0.5 || is_submerged_ocean_source(sample, config) {
        return 0.0;
    }
    if sample.river_flow_hint <= 0.0 || sample.river_valley_strength <= 0.12 {
        return 0.0;
    }

    let valley_t = sample.river_valley_strength.clamp(0.0, 1.0);
    let bank_t = (1.0 - valley_t).clamp(0.0, 1.0);
    let rough_t = sample.river_bank_roughness_hint.clamp(0.0, 1.0);
    let gravel_t = sample.river_gravel_hint.clamp(0.0, 1.0);
    let shoulder_t = (bank_t * 1.55).clamp(0.0, 1.0);
    let amplitude = (config.amplitude_blocks * (0.55 + rough_t * 0.45 + gravel_t * 0.35))
        .min(config.max_abs_blocks * 1.15)
        .max(0.0)
        * shoulder_t;
    if amplitude <= f32::EPSILON {
        return 0.0;
    }

    octave_noise_2d(sample.position.x, sample.position.z, config, 0.48, 0xB4) * amplitude
}

pub fn ocean_bed_relief_blocks(sample: &MacroFieldSample, config: HeightfieldPerlinConfig) -> f32 {
    if !config.enabled || config.amplitude_blocks <= 0.0 || config.max_abs_blocks <= 0.0 {
        return 0.0;
    }
    if sample.ocean_mask <= 0.5 || sample.lake_mask > 0.5 {
        return 0.0;
    }
    if sample.river_valley_strength > 0.5 && sample.river_flow_hint > 0.0 {
        return 0.0;
    }

    let depth_t = (-sample.combined_macro_height).max(0.0).clamp(0.0, 1.0);
    let coast_fade = smoothstep01((depth_t / 0.08).clamp(0.0, 1.0));
    let amplitude = (config.amplitude_blocks * (0.25 + depth_t * 0.35))
        .min(config.max_abs_blocks * 0.65)
        .max(0.0)
        * coast_fade;
    if amplitude <= f32::EPSILON {
        return 0.0;
    }

    octave_noise_2d(sample.position.x, sample.position.z, config, 1.15, 0xD3) * amplitude
}

pub(super) fn octave_noise_2d(
    x_blocks: f32,
    z_blocks: f32,
    config: HeightfieldPerlinConfig,
    scale_multiplier: f32,
    salt_octave: u8,
) -> f32 {
    if !config.enabled || config.amplitude_blocks <= 0.0 || config.max_abs_blocks <= 0.0 {
        return 0.0;
    }

    let octaves = config.octaves.max(1);
    let mut frequency = 1.0 / (config.base_scale_blocks * scale_multiplier).max(f32::EPSILON);
    let mut amplitude = 1.0;
    let mut amplitude_sum = 0.0;
    let mut value = 0.0;

    for octave in 0..octaves {
        value += perlin_2d(
            x_blocks * frequency,
            z_blocks * frequency,
            config.seed,
            config.generator_version,
            octave.wrapping_add(salt_octave),
        ) * amplitude;
        amplitude_sum += amplitude;
        amplitude *= config.persistence;
        frequency *= config.lacunarity;
    }

    if amplitude_sum <= f32::EPSILON {
        0.0
    } else {
        (value / amplitude_sum).clamp(-1.0, 1.0)
    }
}

fn is_submerged_ocean_source(sample: &MacroFieldSample, config: HeightfieldPerlinConfig) -> bool {
    sample.ocean_mask > 0.5
        && sample.combined_macro_height < -shallow_ocean_border_band_normalized(config)
}

fn shallow_ocean_border_band_normalized(config: HeightfieldPerlinConfig) -> f32 {
    config.max_abs_blocks.max(0.0) / NEGATIVE_MACRO_HEIGHT_BLOCKS_PER_UNIT
}

fn perlin_2d(x: f32, z: f32, seed: u64, generator_version: u32, octave: u8) -> f32 {
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let x1 = x0 + 1;
    let z1 = z0 + 1;
    let tx = x - x0 as f32;
    let tz = z - z0 as f32;
    let sx = fade(tx);
    let sz = fade(tz);

    let n00 = gradient_dot(x0, z0, tx, tz, seed, generator_version, octave);
    let n10 = gradient_dot(x1, z0, tx - 1.0, tz, seed, generator_version, octave);
    let n01 = gradient_dot(x0, z1, tx, tz - 1.0, seed, generator_version, octave);
    let n11 = gradient_dot(x1, z1, tx - 1.0, tz - 1.0, seed, generator_version, octave);

    let ix0 = lerp(n00, n10, sx);
    let ix1 = lerp(n01, n11, sx);
    lerp(ix0, ix1, sz).clamp(-1.0, 1.0)
}

fn gradient_dot(
    grid_x: i32,
    grid_z: i32,
    dx: f32,
    dz: f32,
    seed: u64,
    generator_version: u32,
    octave: u8,
) -> f32 {
    const GRADIENTS: [(f32, f32); 8] = [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (0.70710677, 0.70710677),
        (-0.70710677, 0.70710677),
        (0.70710677, -0.70710677),
        (-0.70710677, -0.70710677),
    ];
    let hash = lattice_hash(grid_x, grid_z, seed, generator_version, octave);
    let gradient = GRADIENTS[hash as usize % GRADIENTS.len()];
    gradient.0 * dx + gradient.1 * dz
}

fn lattice_hash(grid_x: i32, grid_z: i32, seed: u64, generator_version: u32, octave: u8) -> u64 {
    let mut value = seed
        ^ ((generator_version as u64) << 32)
        ^ ((octave as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    value ^= (grid_x as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= (grid_z as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn fade(value: f32) -> f32 {
    value * value * value * (value * (value * 6.0 - 15.0) + 10.0)
}

fn smoothstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
