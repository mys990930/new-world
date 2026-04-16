use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::TemperatePlateau,
    biome_family: BiomeFamily::TemperateGrassland,
    terrain_form_family: TerrainFormFamily::Plateau,
    summary: "High but comparatively calm tableland that wants step transitions and edges.",
    regional_traits: &[
        "Plateau interior should stay readable and not collapse into mountain noise.",
        "Edges should invite escarpments and terraces rather than random roughness.",
    ],
    ecology_notes: &[
        "Supports grassland or sparse woodland depending on moisture.",
        "Wind exposure can thin soil and increase rock reveal on plateau rims.",
    ],
    allowed_meso_keys: &["escarpment_band", "upland_terrace", "shallow_basin"],
    seasonal_profile: SeasonalSurfaceProfile::TemperateFourSeason,
    water_response: WaterResponseHint::AvoidsStandingWater,
    ecology_density: EcologyDensityHint::Open,
};
