use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::MediterraneanShrublandHills,
    biome_family: BiomeFamily::MediterraneanShrubland,
    terrain_form_family: TerrainFormFamily::HillCountry,
    summary: "Seasonally dry shrubland hills.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["hill_cluster", "ravine", "escarpment_band",],
    seasonal_profile: SeasonalSurfaceProfile::TemperateFourSeason,
    water_response: WaterResponseHint::AvoidsStandingWater,
    ecology_density: EcologyDensityHint::Sparse,
};
