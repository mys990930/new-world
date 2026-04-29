use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};
use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::MonsoonFloodplain,
    biome_family: BiomeFamily::MonsoonForest,
    terrain_form_family: TerrainFormFamily::Floodplain,
    summary: "Monsoon-fed lowland with strong wet-dry pulses.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["shallow_basin", "ravine"],
    seasonal_profile: SeasonalSurfaceProfile::TropicalWetDry,
    water_response: WaterResponseHint::EmbracesFloodplain,
    ecology_density: EcologyDensityHint::Dense,
};
