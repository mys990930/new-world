use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::TropicalRainforestHills,
    biome_family: BiomeFamily::TropicalRainforest,
    terrain_form_family: TerrainFormFamily::HillCountry,
    summary: "Warm wet hill country with ravines and shoulder ridges.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["hill_cluster", "ravine", "escarpment_band",],
    seasonal_profile: SeasonalSurfaceProfile::TropicalWetDry,
    water_response: WaterResponseHint::AllowsWetMargins,
    ecology_density: EcologyDensityHint::Dense,
};
