use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::TropicalRainforestLowland,
    biome_family: BiomeFamily::TropicalRainforest,
    terrain_form_family: TerrainFormFamily::Plain,
    summary: "Warm, wet lowland with heavy cover potential and broad valley wetness.",
    regional_traits: &[
        "Surface should stay lowland-first rather than becoming accidental hills.",
        "Water corridors and wet pockets should be common but still connected to drainage logic.",
    ],
    ecology_notes: &[
        "Supports dense canopy and rich undergrowth in later ecology passes.",
        "Wet versus very wet phases should change saturation more than the archetype itself.",
    ],
    allowed_meso_keys: &["hill_cluster", "shallow_basin", "ravine"],
    seasonal_profile: SeasonalSurfaceProfile::TropicalWetDry,
    water_response: WaterResponseHint::EmbracesFloodplain,
    ecology_density: EcologyDensityHint::Dense,
};
