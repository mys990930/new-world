use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::SemiDesertPediment,
    biome_family: BiomeFamily::SemiDesert,
    terrain_form_family: TerrainFormFamily::Pediment,
    summary: "Semi-desert slope apron and stripped rock low relief.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["dune_field", "ravine",],
    seasonal_profile: SeasonalSurfaceProfile::AridSparse,
    water_response: WaterResponseHint::AvoidsStandingWater,
    ecology_density: EcologyDensityHint::Barren,
};
