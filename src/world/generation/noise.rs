const HASH_K1: u64 = 0x9E37_79B9_7F4A_7C15;
const HASH_K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const HASH_K3: u64 = 0x1656_67B1_9E37_79F9;

pub(super) const STONE_DEPTH_SALT: u64 = 0x9511_1100_0000_0002;
pub(super) const MATERIAL_BLEND_SALT: u64 = 0x9511_1100_0000_0003;
pub(super) const OCEAN_FLOOR_SALT: u64 = 0x9511_1100_0000_0101;
pub(super) const ROLLING_RELIEF_SALT: u64 = 0x9511_1100_0000_0102;
pub(super) const DETAIL_RELIEF_SALT: u64 = 0x9511_1100_0000_0103;
pub(super) const RIDGE_RELIEF_SALT: u64 = 0x9511_1100_0000_0104;

pub(super) fn centered_fbm(
    seed: u64,
    world_x: i32,
    world_z: i32,
    scale_blocks: f64,
    octaves: u32,
    lacunarity: f64,
    gain: f32,
    salt: u64,
) -> f32 {
    fbm(
        seed,
        world_x as f64 / scale_blocks,
        world_z as f64 / scale_blocks,
        octaves,
        lacunarity,
        gain,
        salt,
    ) * 2.0
        - 1.0
}

pub(super) fn ridge_signal_fbm(
    seed: u64,
    world_x: i32,
    world_z: i32,
    scale_blocks: f64,
    octaves: u32,
    lacunarity: f64,
    gain: f32,
    salt: u64,
) -> f32 {
    ridged_fbm(
        seed,
        world_x as f64 / scale_blocks,
        world_z as f64 / scale_blocks,
        octaves,
        lacunarity,
        gain,
        salt,
    )
}

pub(super) fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

pub(super) fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

pub(super) fn smoothstep_range(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }

    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn hash01_2d(seed: u64, x: i32, z: i32, salt: u64) -> f32 {
    noise01_2d(seed, x, z, salt)
}

pub(super) fn hash01_3d(seed: u64, x: i32, y: i32, z: i32, salt: u64) -> f32 {
    let mut value = seed ^ salt;
    value ^= (x as u64).wrapping_mul(HASH_K1);
    value ^= (y as u64).wrapping_mul(0xC6BC_2796_92B5_CC83);
    value ^= (z as u64).wrapping_mul(HASH_K2);
    let bits = splitmix64(value) >> 11;
    let max = ((1_u64 << 53) - 1) as f64;
    (bits as f64 / max) as f32
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(HASH_K1);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn lattice_hash(seed: u64, x: i64, z: i64, salt: u64) -> u64 {
    let x_bits = (x as u64).wrapping_mul(HASH_K2);
    let z_bits = (z as u64).wrapping_mul(HASH_K3);
    splitmix64(seed ^ salt ^ x_bits ^ z_bits)
}

fn noise01_2d(seed: u64, x: i32, z: i32, salt: u64) -> f32 {
    hash01(seed, x as i64, z as i64, salt)
}

fn hash01(seed: u64, x: i64, z: i64, salt: u64) -> f32 {
    let bits = lattice_hash(seed, x, z, salt) >> 11;
    let max = ((1_u64 << 53) - 1) as f64;
    (bits as f64 / max) as f32
}

fn smoothstep01(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn value_noise(seed: u64, x: f64, z: f64, salt: u64) -> f32 {
    let x0 = x.floor() as i64;
    let z0 = z.floor() as i64;
    let tx = smoothstep01(x - x0 as f64);
    let tz = smoothstep01(z - z0 as f64);

    let n00 = hash01(seed, x0, z0, salt) as f64;
    let n10 = hash01(seed, x0 + 1, z0, salt) as f64;
    let n01 = hash01(seed, x0, z0 + 1, salt) as f64;
    let n11 = hash01(seed, x0 + 1, z0 + 1, salt) as f64;

    let nx0 = n00 + (n10 - n00) * tx;
    let nx1 = n01 + (n11 - n01) * tx;
    (nx0 + (nx1 - nx0) * tz) as f32
}

fn fbm(
    seed: u64,
    x: f64,
    z: f64,
    octaves: u32,
    lacunarity: f64,
    gain: f32,
    salt: u64,
) -> f32 {
    let mut amplitude = 1.0_f32;
    let mut frequency = 1.0_f64;
    let mut total = 0.0_f32;
    let mut amplitude_sum = 0.0_f32;

    for octave in 0..octaves {
        let octave_salt = salt.wrapping_add((octave as u64).wrapping_mul(HASH_K1));
        total += value_noise(seed, x * frequency, z * frequency, octave_salt) * amplitude;
        amplitude_sum += amplitude;
        amplitude *= gain;
        frequency *= lacunarity;
    }

    if amplitude_sum == 0.0 {
        0.0
    } else {
        total / amplitude_sum
    }
}

fn ridged_fbm(
    seed: u64,
    x: f64,
    z: f64,
    octaves: u32,
    lacunarity: f64,
    gain: f32,
    salt: u64,
) -> f32 {
    let mut amplitude = 1.0_f32;
    let mut frequency = 1.0_f64;
    let mut total = 0.0_f32;
    let mut amplitude_sum = 0.0_f32;

    for octave in 0..octaves {
        let octave_salt = salt.wrapping_add((octave as u64).wrapping_mul(HASH_K2));
        let signal = value_noise(seed, x * frequency, z * frequency, octave_salt);
        let ridged = 1.0 - (signal * 2.0 - 1.0).abs();
        total += ridged * amplitude;
        amplitude_sum += amplitude;
        amplitude *= gain;
        frequency *= lacunarity;
    }

    if amplitude_sum == 0.0 {
        0.0
    } else {
        total / amplitude_sum
    }
}
