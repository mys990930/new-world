use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::GlacialValley,
    biome_family: BiomeFamily::PolarIce,
    terrain_form_family: TerrainFormFamily::GlacialValley,
    summary: "Long glacial trough descending from ice uplands.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["ravine", "upland_terrace",],
    seasonal_profile: SeasonalSurfaceProfile::AlpineSnowPersistent,
    water_response: WaterResponseHint::GlacialMeltDriven,
    ecology_density: EcologyDensityHint::Barren,
};
