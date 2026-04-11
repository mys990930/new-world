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
        centered_fbm(seed, world_x, world_z, 88.0, 4, 2.0, 0.5, ROLLING_RELIEF_SALT) * 6.0;
    let shoulder_signal = ridge_signal_fbm(seed, world_x, world_z, 56.0, 4, 2.0, 0.5, RIDGE_RELIEF_SALT);
    let shoulders = shoulder_signal * 5.5;
    let escarpment_mask = clamp01(
        sample.ridge_factor * 0.95 + sample.ruggedness * 0.70 + sample.mountain_mass * 0.90 - 0.38,
    );
    let escarpments = shoulder_signal.powf(3.0) * (8.0 + sample.mountain_mass * 10.0) * escarpment_mask;
    let ravines = centered_fbm(
        seed,
        world_x,
        world_z,
        22.0,
        4,
        2.0,
        0.5,
        DETAIL_RELIEF_SALT.wrapping_add(17),
    )
    .min(0.0)
        * (4.0 + sample.ruggedness * 6.0);
    let detail = centered_fbm(seed, world_x, world_z, 16.0, 3, 2.0, 0.5, DETAIL_RELIEF_SALT) * 2.4;
    let river_carve = sample.riverine_factor * 3.2;

    (base + rolling + shoulders + escarpments + ravines + detail - river_carve).clamp(6.0, 40.0)
}
