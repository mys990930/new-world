use super::super::context::ColumnAtlasSample;
use super::super::noise::{
    DETAIL_RELIEF_SALT, RIDGE_RELIEF_SALT, ROLLING_RELIEF_SALT, centered_fbm, clamp01,
    ridge_signal_fbm,
};

pub(super) fn surface_y(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
) -> f32 {
    let base = 8.0
        + sample.macro_elevation * 15.0
        + sample.continent_core_factor * 6.0
        + sample.ruggedness * 6.0
        + sample.mountain_mass * 8.0;
    let rolling =
        centered_fbm(seed, world_x, world_z, 112.0, 4, 2.0, 0.5, ROLLING_RELIEF_SALT) * 4.8;
    let shoulder_signal =
        ridge_signal_fbm(seed, world_x, world_z, 72.0, 4, 2.0, 0.5, RIDGE_RELIEF_SALT);
    let shoulders = shoulder_signal * 4.2;
    let escarpment_mask = clamp01(
        sample.ridge_factor * 0.95 + sample.ruggedness * 0.70 + sample.mountain_mass * 0.90 - 0.38,
    );
    let escarpments =
        shoulder_signal.powf(3.0) * (6.0 + sample.mountain_mass * 8.0) * escarpment_mask;
    let ravines = centered_fbm(
        seed,
        world_x,
        world_z,
        36.0,
        4,
        2.0,
        0.5,
        DETAIL_RELIEF_SALT.wrapping_add(17),
    )
    .min(0.0)
        * (2.5 + sample.ruggedness * 4.0);
    let detail = centered_fbm(seed, world_x, world_z, 28.0, 3, 2.0, 0.5, DETAIL_RELIEF_SALT) * 1.0;
    let river_carve = sample.riverine_factor * 3.2;

    (base + rolling + shoulders + escarpments + ravines + detail - river_carve).clamp(6.0, 40.0)
}
