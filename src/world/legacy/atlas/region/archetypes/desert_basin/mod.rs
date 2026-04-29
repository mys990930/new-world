use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};
use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::DesertBasin,
    biome_family: BiomeFamily::Desert,
    terrain_form_family: TerrainFormFamily::Basin,
    summary: "Dry enclosed basin with concentrated sediment floor.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["dune_field", "shallow_basin"],
    seasonal_profile: SeasonalSurfaceProfile::AridSparse,
    water_response: WaterResponseHint::AvoidsStandingWater,
    ecology_density: EcologyDensityHint::Barren,
};
