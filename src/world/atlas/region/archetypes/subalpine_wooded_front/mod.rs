use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::SubalpineWoodedFront,
    biome_family: BiomeFamily::SubalpineWoodland,
    terrain_form_family: TerrainFormFamily::MountainFront,
    summary: "Cold wooded mountain-front transition.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["upland_terrace", "ravine", "escarpment_band",],
    seasonal_profile: SeasonalSurfaceProfile::AlpineSnowPersistent,
    water_response: WaterResponseHint::GlacialMeltDriven,
    ecology_density: EcologyDensityHint::Open,
};
