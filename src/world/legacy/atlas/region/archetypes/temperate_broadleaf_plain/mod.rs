use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};
use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::TemperateBroadleafPlain,
    biome_family: BiomeFamily::TemperateBroadleafForest,
    terrain_form_family: TerrainFormFamily::Plain,
    summary: "Broadleaf-dominant temperate lowland.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["hill_cluster", "shallow_basin"],
    seasonal_profile: SeasonalSurfaceProfile::TemperateFourSeason,
    water_response: WaterResponseHint::AllowsWetMargins,
    ecology_density: EcologyDensityHint::Dense,
};
