mod coast;
mod ocean;
mod plain;
mod ridge;
mod upland;

use super::context::ColumnAtlasSample;
use super::profile::TerrainProfile;

pub(super) fn surface_y_for_profile(
    seed: u64,
    world_x: i32,
    world_z: i32,
    sample: ColumnAtlasSample,
    profile: TerrainProfile,
) -> i32 {
    let surface = match profile {
        TerrainProfile::DeepOcean => ocean::deep_ocean_surface_y(seed, world_x, world_z, sample),
        TerrainProfile::Shelf => ocean::shelf_surface_y(seed, world_x, world_z, sample),
        TerrainProfile::Coast => coast::surface_y(seed, world_x, world_z, sample),
        TerrainProfile::Plain => plain::surface_y(seed, world_x, world_z, sample),
        TerrainProfile::Upland => upland::surface_y(seed, world_x, world_z, sample),
        TerrainProfile::Ridge => ridge::surface_y(seed, world_x, world_z, sample),
    };

    surface.round() as i32
}
