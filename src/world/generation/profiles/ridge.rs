use super::super::context::ColumnAtlasSample;
use super::super::noise::{
    DETAIL_RELIEF_SALT, RIDGE_RELIEF_SALT, ROLLING_RELIEF_SALT, SURFACE_JITTER_SALT, centered_fbm,
    centered_noise, ridge_signal_fbm,
};

pub(super) fn surface_y(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
) -> f32 {
    let base = 14.0
        + sample.macro_elevation * 16.0
        + sample.continent_core_factor * 8.0
        + sample.ridge_factor * 10.0
        + sample.mountain_mass * 14.0
        + sample.alpine_factor * 8.0;
    let massif = centered_fbm(seed, world_x, world_z, 120.0, 4, 2.0, 0.5, ROLLING_RELIEF_SALT) * 6.0;
    let spine = ridge_signal_fbm(seed, world_x, world_z, 52.0, 5, 2.0, 0.52, RIDGE_RELIEF_SALT) * 9.0;
    let detail = centered_fbm(seed, world_x, world_z, 14.0, 3, 2.0, 0.5, DETAIL_RELIEF_SALT) * 2.8;
    let jitter = centered_noise(seed, world_x, world_z, SURFACE_JITTER_SALT) * 1.2;
    let river_carve = sample.riverine_factor * 4.0;

    (base + massif + spine + detail + jitter - river_carve).clamp(12.0, 48.0)
}
