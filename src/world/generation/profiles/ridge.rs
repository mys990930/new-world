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
    let base = 14.0
        + sample.macro_elevation * 19.0
        + sample.continent_core_factor * 8.0
        + sample.ridge_factor * 16.0
        + sample.mountain_mass * 20.0
        + sample.alpine_factor * 11.0;
    let massif =
        centered_fbm(seed, world_x, world_z, 144.0, 4, 2.0, 0.5, ROLLING_RELIEF_SALT) * 8.0;
    let spine_signal =
        ridge_signal_fbm(seed, world_x, world_z, 72.0, 5, 2.0, 0.52, RIDGE_RELIEF_SALT);
    let spine = spine_signal * 11.5;
    let knife_edge = spine_signal.powf(4.8) * (10.0 + sample.mountain_mass * 12.0);
    let buttress_signal = ridge_signal_fbm(
        seed,
        world_x,
        world_z,
        132.0,
        4,
        2.0,
        0.5,
        RIDGE_RELIEF_SALT.wrapping_add(33),
    );
    let buttress = buttress_signal.powf(2.5) * 7.0;
    let cliff_mask = clamp01(
        sample.ridge_factor * 1.05 + sample.mountain_mass * 0.95 + sample.ruggedness * 0.55 - 0.42,
    );
    let fault_scars = centered_fbm(
        seed,
        world_x,
        world_z,
        44.0,
        4,
        2.0,
        0.5,
        DETAIL_RELIEF_SALT.wrapping_add(41),
    )
    .min(0.0)
        * (3.5 + cliff_mask * 6.0);
    let couloirs = centered_fbm(
        seed,
        world_x,
        world_z,
        34.0,
        4,
        2.0,
        0.5,
        DETAIL_RELIEF_SALT.wrapping_add(21),
    )
    .min(0.0)
        * (5.0 + cliff_mask * 7.0);
    let detail = centered_fbm(seed, world_x, world_z, 28.0, 3, 2.0, 0.5, DETAIL_RELIEF_SALT) * 1.2;
    let river_carve = sample.riverine_factor * 4.0;

    (base + massif + spine + knife_edge + buttress + fault_scars + couloirs + detail - river_carve)
        .clamp(12.0, 76.0)
}
