use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};
use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::FjordCoast,
    biome_family: BiomeFamily::RockyCoast,
    terrain_form_family: TerrainFormFamily::FjordCoast,
    summary: "Cold drowned valley coast and steep marine walls.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["coastal_cliff_band", "ravine"],
    seasonal_profile: SeasonalSurfaceProfile::CoastalTemperate,
    water_response: WaterResponseHint::CoastalSprayExposed,
    ecology_density: EcologyDensityHint::Barren,
};
