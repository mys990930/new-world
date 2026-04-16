use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::DesertPlain,
    biome_family: BiomeFamily::Desert,
    terrain_form_family: TerrainFormFamily::Plain,
    summary: "Dry exposed desert floor dominated by sediment ownership.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["dune_field", "ravine", "crater",],
    seasonal_profile: SeasonalSurfaceProfile::AridSparse,
    water_response: WaterResponseHint::AvoidsStandingWater,
    ecology_density: EcologyDensityHint::Barren,
};
