use super::super::context::ColumnAtlasSample;
use super::super::noise::{DETAIL_RELIEF_SALT, ROLLING_RELIEF_SALT, centered_fbm};

pub(super) fn surface_y(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
) -> f32 {
    let base = 4.0
        + sample.macro_elevation * 10.0
        + sample.continent_core_factor * 5.0
        - sample.coast_factor * 1.5;
    let rolling =
        centered_fbm(seed, world_x, world_z, 128.0, 4, 2.0, 0.5, ROLLING_RELIEF_SALT) * 4.0;
    let terraces = centered_fbm(seed, world_x, world_z, 72.0, 3, 2.0, 0.5, DETAIL_RELIEF_SALT) * 1.2;
    let detail = centered_fbm(
        seed,
        world_x,
        world_z,
        32.0,
        3,
        2.0,
        0.5,
        DETAIL_RELIEF_SALT.wrapping_add(9),
    ) * 0.5;
    let river_carve = sample.riverine_factor * (2.0 + (1.0 - sample.mountain_mass) * 2.5);

    (base + rolling + terraces + detail - river_carve).clamp(2.0, 20.0)
}
