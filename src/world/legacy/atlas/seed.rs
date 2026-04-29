const HASH_K1: u64 = 0x9E37_79B9_7F4A_7C15;
const HASH_K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
const HASH_K3: u64 = 0x1656_67B1_9E37_79F9;

pub(crate) const SALT_CONTINENT_PRIMARY: u64 = 0xA711_A5C1_1000_0001;
pub(crate) const SALT_CONTINENT_SECONDARY: u64 = 0xA711_A5C1_1000_0002;
pub(crate) const SALT_COAST_ROUGHNESS: u64 = 0xA711_A5C1_2000_0001;
pub(crate) const SALT_RIDGE_PRIMARY: u64 = 0xA711_A5C1_3000_0001;
pub(crate) const SALT_RIDGE_SECONDARY: u64 = 0xA711_A5C1_3000_0002;
pub(crate) const SALT_MOUNTAIN_CLUSTER: u64 = 0xA711_A5C1_4000_0001;
pub(crate) const SALT_TEMPERATURE: u64 = 0xA711_A5C1_5000_0001;
pub(crate) const SALT_HUMIDITY: u64 = 0xA711_A5C1_6000_0001;
pub(crate) const SALT_WARP_X: u64 = 0xA711_A5C1_7000_0001;
pub(crate) const SALT_WARP_Z: u64 = 0xA711_A5C1_7000_0002;
pub(crate) const SALT_DETAIL: u64 = 0xA711_A5C1_8000_0001;

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(HASH_K1);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

pub(crate) fn lattice_hash(seed: u64, x: i64, z: i64, salt: u64) -> u64 {
    let x_bits = (x as u64).wrapping_mul(HASH_K2);
    let z_bits = (z as u64).wrapping_mul(HASH_K3);
    splitmix64(seed ^ salt ^ x_bits ^ z_bits)
}

pub(crate) fn hash01(seed: u64, x: i64, z: i64, salt: u64) -> f32 {
    let bits = lattice_hash(seed, x, z, salt) >> 11;
    let max = ((1_u64 << 53) - 1) as f64;
    (bits as f64 / max) as f32
}

fn smoothstep01(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(crate) fn value_noise(seed: u64, x: f64, z: f64, salt: u64) -> f32 {
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

pub(crate) fn fbm(
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
        let salt = salt.wrapping_add((octave as u64).wrapping_mul(HASH_K1));
        total += value_noise(seed, x * frequency, z * frequency, salt) * amplitude;
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

pub(crate) fn ridged_fbm(
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
        let salt = salt.wrapping_add((octave as u64).wrapping_mul(HASH_K2));
        let signal = value_noise(seed, x * frequency, z * frequency, salt);
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

pub(crate) fn domain_warp(
    seed: u64,
    x: f64,
    z: f64,
    warp_scale: f64,
    amplitude: f64,
) -> (f64, f64) {
    let dx = (fbm(
        seed,
        x * warp_scale,
        z * warp_scale,
        3,
        2.0,
        0.5,
        SALT_WARP_X,
    ) - 0.5) as f64
        * 2.0
        * amplitude;
    let dz = (fbm(
        seed,
        x * warp_scale,
        z * warp_scale,
        3,
        2.0,
        0.5,
        SALT_WARP_Z,
    ) - 0.5) as f64
        * 2.0
        * amplitude;
    (x + dx, z + dz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_noise_is_deterministic() {
        let a = value_noise(42, 12.5, -8.25, SALT_CONTINENT_PRIMARY);
        let b = value_noise(42, 12.5, -8.25, SALT_CONTINENT_PRIMARY);
        let c = value_noise(43, 12.5, -8.25, SALT_CONTINENT_PRIMARY);

        assert_eq!(a.to_bits(), b.to_bits());
        assert_ne!(a.to_bits(), c.to_bits());
    }
}
