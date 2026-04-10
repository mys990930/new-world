use super::super::context::ColumnAtlasSample;
use super::super::noise::{
    DETAIL_RELIEF_SALT, RIDGE_RELIEF_SALT, ROLLING_RELIEF_SALT, centered_fbm, ridge_signal_fbm,
};

pub(super) fn surface_y(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
) -> f32 {
    let base = 8.0
        + sample.macro_elevation * 14.0
        + sample.continent_core_factor * 6.0
        + sample.ruggedness * 5.0
        + sample.mountain_mass * 5.0;
    let rolling = centered_fbm(seed, world_x, world_z, 88.0, 4, 2.0, 0.5, ROLLING_RELIEF_SALT) * 6.0;
    let shoulders = ridge_signal_fbm(seed, world_x, world_z, 56.0, 4, 2.0, 0.5, RIDGE_RELIEF_SALT) * 4.5;
    let detail = centered_fbm(seed, world_x, world_z, 16.0, 3, 2.0, 0.5, DETAIL_RELIEF_SALT) * 2.4;
    let river_carve = sample.riverine_factor * 3.2;

    (base + rolling + shoulders + detail - river_carve).clamp(6.0, 32.0)
}
