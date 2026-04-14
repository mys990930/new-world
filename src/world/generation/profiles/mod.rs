mod coast;
mod ocean;
mod plain;
mod ridge;
mod upland;

use super::context::ColumnAtlasSample;
use super::profile::{TerrainProfile, surface_profile_blend};

pub(super) fn surface_height_for_profile(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    profile: TerrainProfile,
) -> f32 {
    match profile {
        TerrainProfile::DeepOcean => ocean::deep_ocean_surface_y(seed, world_x, world_z, sample),
        TerrainProfile::Shelf => ocean::shelf_surface_y(seed, world_x, world_z, sample),
        TerrainProfile::Coast => coast::surface_y(seed, world_x, world_z, sample),
        TerrainProfile::Plain => plain::surface_y(seed, world_x, world_z, sample),
        TerrainProfile::Upland => upland::surface_y(seed, world_x, world_z, sample),
        TerrainProfile::Ridge => ridge::surface_y(seed, world_x, world_z, sample),
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn surface_y_for_profile(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    profile: TerrainProfile,
) -> i32 {
    surface_height_for_profile(seed, world_x, world_z, sample, profile).round() as i32
}

pub(super) fn surface_height_for_sample(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    land_threshold: f32,
) -> f32 {
    let blend = surface_profile_blend(sample, land_threshold);
    blend.deep_ocean
        * surface_height_for_profile(seed, world_x, world_z, sample, TerrainProfile::DeepOcean)
        + blend.shelf
            * surface_height_for_profile(seed, world_x, world_z, sample, TerrainProfile::Shelf)
        + blend.coast
            * surface_height_for_profile(seed, world_x, world_z, sample, TerrainProfile::Coast)
        + blend.plain
            * surface_height_for_profile(seed, world_x, world_z, sample, TerrainProfile::Plain)
        + blend.upland
            * surface_height_for_profile(seed, world_x, world_z, sample, TerrainProfile::Upland)
        + blend.ridge
            * surface_height_for_profile(seed, world_x, world_z, sample, TerrainProfile::Ridge)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn surface_y_for_sample(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    land_threshold: f32,
) -> i32 {
    surface_height_for_sample(seed, world_x, world_z, sample, land_threshold).round() as i32
}
