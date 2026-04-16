use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::TropicalRainforestHills,
    biome_family: BiomeFamily::TropicalRainforest,
    terrain_form_family: TerrainFormFamily::HillCountry,
    summary: "Warm, wet hills where ravines and shoulder ridges matter more than open plains.",
    regional_traits: &[
        "Base shape should already carry persistent hill structure.",
        "Hydrology should carve connected gullies instead of isolated wet bowls.",
    ],
    ecology_notes: &[
        "Supports dense canopy, hanging wetness, and steep-slope ecological variation later.",
        "Shade and cloud exposure can bias persistent moisture on selected slopes.",
    ],
    allowed_meso_keys: &["ravine", "hill_cluster", "escarpment_band"],
    seasonal_profile: SeasonalSurfaceProfile::TropicalWetDry,
    water_response: WaterResponseHint::EmbracesFloodplain,
    ecology_density: EcologyDensityHint::Dense,
};
