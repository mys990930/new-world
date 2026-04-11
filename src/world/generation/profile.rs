use super::context::ColumnAtlasSample;
use super::noise::{clamp01, smoothstep_range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainProfile {
    DeepOcean,
    Shelf,
    Coast,
    Plain,
    Upland,
    Ridge,
}

impl TerrainProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeepOcean => "deep_ocean",
            Self::Shelf => "shelf",
            Self::Coast => "coast",
            Self::Plain => "plain",
            Self::Upland => "upland",
            Self::Ridge => "ridge",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SurfaceProfileBlend {
    pub deep_ocean: f32,
    pub shelf: f32,
    pub coast: f32,
    pub plain: f32,
    pub upland: f32,
    pub ridge: f32,
}

impl SurfaceProfileBlend {
    pub(super) fn normalize(self) -> Self {
        let total =
            self.deep_ocean + self.shelf + self.coast + self.plain + self.upland + self.ridge;
        if total <= f32::EPSILON {
            return Self {
                plain: 1.0,
                ..Self::default()
            };
        }

        Self {
            deep_ocean: self.deep_ocean / total,
            shelf: self.shelf / total,
            coast: self.coast / total,
            plain: self.plain / total,
            upland: self.upland / total,
            ridge: self.ridge / total,
        }
    }
}

impl Default for SurfaceProfileBlend {
    fn default() -> Self {
        Self {
            deep_ocean: 0.0,
            shelf: 0.0,
            coast: 0.0,
            plain: 0.0,
            upland: 0.0,
            ridge: 0.0,
        }
    }
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

    let ridge_signal = sample.ridge_factor * 0.60
        + sample.mountain_mass * 0.20
        + sample.ruggedness * 0.16
        + sample.alpine_factor * 0.20;
    let upland_signal = sample.macro_elevation * 0.42
        + sample.ruggedness * 0.18
        + sample.continent_core_factor * 0.18
        + ridge_signal * 0.22;

    if sample.coast_factor > 0.52
        && sample.macro_elevation < 0.42
        && sample.mountain_mass < 0.42
    {
        TerrainProfile::Coast
    } else if (sample.ridge_factor > 0.26 && sample.macro_elevation > 0.44)
        || ridge_signal > 0.22
        || sample.alpine_factor > 0.28
    {
        TerrainProfile::Ridge
    } else if upland_signal > 0.38 {
        TerrainProfile::Upland
    } else {
        TerrainProfile::Plain
    }
}

pub(super) fn surface_profile_blend(
    sample: ColumnAtlasSample,
    land_threshold: f32,
) -> SurfaceProfileBlend {
    let land = smoothstep_range(land_threshold - 0.08, land_threshold + 0.08, sample.landness);
    let ocean = 1.0 - land;
    let deep_ocean_signal = sample.ocean_distance * 0.72 + (1.0 - sample.landness) * 0.28;
    let deep_ocean = ocean * smoothstep_range(0.20, 0.48, deep_ocean_signal);
    let shelf = ocean * (0.18 + (1.0 - smoothstep_range(0.18, 0.42, deep_ocean_signal)) * 0.82);

    let coast_base = smoothstep_range(0.22, 0.72, sample.coast_factor)
        * (1.0 - smoothstep_range(0.18, 0.52, sample.macro_elevation))
        * (1.0 - smoothstep_range(0.18, 0.48, sample.mountain_mass));

    let ridge_signal = sample.ridge_factor * 0.60
        + sample.mountain_mass * 0.20
        + sample.ruggedness * 0.16
        + sample.alpine_factor * 0.20;
    let ridge_base = smoothstep_range(
        0.14,
        0.36,
        ridge_signal.max(sample.ridge_factor * 0.72 + sample.alpine_factor * 0.48),
    );
    let upland_signal = sample.macro_elevation * 0.42
        + sample.ruggedness * 0.18
        + sample.continent_core_factor * 0.18
        + ridge_signal * 0.22;
    let upland_base =
        smoothstep_range(0.18, 0.44, upland_signal) * (1.0 - smoothstep_range(0.52, 0.92, ridge_base));
    let plain_base = clamp01(
        0.28
            + (1.0 - coast_base) * 0.30
            + (1.0 - ridge_base) * 0.28
            + (1.0 - upland_base) * 0.24,
    );

    SurfaceProfileBlend {
        deep_ocean,
        shelf,
        coast: land * coast_base,
        plain: land * plain_base,
        upland: land * upland_base,
        ridge: land * ridge_base,
    }
    .normalize()
}
