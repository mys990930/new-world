use super::super::context::ColumnAtlasSample;
use super::super::noise::{
    DETAIL_RELIEF_SALT, OCEAN_FLOOR_SALT, ROLLING_RELIEF_SALT, centered_fbm, clamp01, lerp_f32,
};

pub(super) fn deep_ocean_surface_y(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
) -> f32 {
    let basin_signal =
        clamp01(sample.ocean_distance * 0.80 + (1.0 - sample.landness) * 0.20);
    let basin = lerp_f32(-18.0, -40.0, basin_signal);
    let rolling = centered_fbm(seed, world_x, world_z, 96.0, 4, 2.0, 0.5, OCEAN_FLOOR_SALT) * 5.0;
    let detail = centered_fbm(seed, world_x, world_z, 28.0, 3, 2.0, 0.5, DETAIL_RELIEF_SALT) * 1.8;
    let shelf_pull = sample.coast_factor * 3.0;

    (basin + rolling + detail + shelf_pull).clamp(-40.0, -12.0)
}

pub(super) fn shelf_surface_y(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
) -> f32 {
    let shelf_signal = clamp01(sample.ocean_distance * 0.72 + (1.0 - sample.coast_factor) * 0.12);
    let shelf = lerp_f32(-4.0, -18.0, shelf_signal);
    let rolling = centered_fbm(seed, world_x, world_z, 112.0, 3, 2.0, 0.5, ROLLING_RELIEF_SALT) * 3.6;
    let detail = centered_fbm(seed, world_x, world_z, 24.0, 3, 2.0, 0.5, DETAIL_RELIEF_SALT) * 1.6;
    let coastal_lift = sample.coast_factor * 2.4;

    (shelf + rolling + detail + coastal_lift).clamp(-18.0, -2.0)
}
