use super::context::ColumnAtlasSample;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TerrainProfile {
    DeepOcean,
    Shelf,
    Coast,
    Plain,
    Upland,
    Ridge,
}

pub(super) fn resolve_profile(
    sample: ColumnAtlasSample,
    land_threshold: f32,
) -> TerrainProfile {
    if sample.landness < land_threshold {
        let deep_ocean_signal =
            sample.ocean_distance * 0.72 + (1.0 - sample.landness) * 0.28;
        return if deep_ocean_signal > 0.38 {
            TerrainProfile::DeepOcean
        } else {
            TerrainProfile::Shelf
        };
    }

    let ridge_signal =
        sample.ridge_factor * 0.45 + sample.mountain_mass * 0.40 + sample.alpine_factor * 0.15;
    let upland_signal = sample.macro_elevation * 0.42
        + sample.ruggedness * 0.18
        + sample.continent_core_factor * 0.18
        + ridge_signal * 0.22;

    if sample.coast_factor > 0.52
        && sample.macro_elevation < 0.42
        && sample.mountain_mass < 0.42
    {
        TerrainProfile::Coast
    } else if ridge_signal > 0.55 || sample.alpine_factor > 0.48 {
        TerrainProfile::Ridge
    } else if upland_signal > 0.42 {
        TerrainProfile::Upland
    } else {
        TerrainProfile::Plain
    }
}
