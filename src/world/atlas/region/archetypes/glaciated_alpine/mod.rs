use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::GlaciatedAlpine,
    biome_family: BiomeFamily::PolarIce,
    terrain_form_family: TerrainFormFamily::Icefield,
    summary: "Persistent ice and snow upland with glacial control.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["upland_terrace", "ravine", "crater",],
    seasonal_profile: SeasonalSurfaceProfile::AlpineSnowPersistent,
    water_response: WaterResponseHint::GlacialMeltDriven,
    ecology_density: EcologyDensityHint::Barren,
};
