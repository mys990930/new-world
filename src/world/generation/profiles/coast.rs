use super::super::context::ColumnAtlasSample;
use super::super::noise::{DETAIL_RELIEF_SALT, ROLLING_RELIEF_SALT, centered_fbm};

pub(super) fn surface_y(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
) -> f32 {
    let coastal_rise = 1.5
        + sample.macro_elevation * 6.0
        + sample.continent_core_factor * 4.0
        - sample.coast_factor * 2.5;
    let beach_roll = centered_fbm(seed, world_x, world_z, 84.0, 3, 2.0, 0.5, ROLLING_RELIEF_SALT) * 3.0;
    let detail = centered_fbm(seed, world_x, world_z, 20.0, 3, 2.0, 0.5, DETAIL_RELIEF_SALT) * 1.5;
    let river_carve = sample.riverine_factor * 1.5;

    (coastal_rise + beach_roll + detail - river_carve).clamp(-1.0, 10.0)
}
