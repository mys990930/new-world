use super::super::context::ColumnAtlasSample;
use super::super::noise::{DETAIL_RELIEF_SALT, ROLLING_RELIEF_SALT, centered_fbm};

pub(super) fn surface_y(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
) -> f32 {
    let shoreline_rise = sample.ocean_distance * 4.8 + sample.continent_core_factor * 2.2;
    let coastal_rise = 0.6
        + shoreline_rise
        + sample.macro_elevation * 4.2
        + sample.continent_core_factor * 2.0
        - sample.coast_factor * 1.1;
    let beach_roll =
        centered_fbm(seed, world_x, world_z, 112.0, 4, 2.0, 0.5, ROLLING_RELIEF_SALT) * 1.6;
    let longshore =
        centered_fbm(seed, world_x, world_z, 220.0, 3, 2.0, 0.5, ROLLING_RELIEF_SALT.wrapping_add(13)) * 0.8;
    let detail = centered_fbm(seed, world_x, world_z, 24.0, 3, 2.0, 0.5, DETAIL_RELIEF_SALT) * 0.7;
    let river_carve = sample.riverine_factor * 0.3;

    (coastal_rise + beach_roll + longshore + detail - river_carve).clamp(0.0, 10.0)
}
