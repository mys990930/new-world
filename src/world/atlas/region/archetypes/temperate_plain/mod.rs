use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::TemperatePlain,
    biome_family: BiomeFamily::TemperateGrassland,
    terrain_form_family: TerrainFormFamily::Plain,
    summary: "Balanced inland grassland plain with readable room for low-relief meso accents.",
    regional_traits: &[
        "Broad, traversable land with modest macro relief.",
        "River corridors should read clearly because the surrounding terrain is restrained.",
    ],
    ecology_notes: &[
        "Supports open grassland, scattered shrubs, and later open woodland transitions.",
        "Seasonal browning and snow-capable cover are both plausible.",
    ],
    allowed_meso_keys: &["hill_cluster", "shallow_basin", "ravine"],
    seasonal_profile: SeasonalSurfaceProfile::TemperateSnowCapable,
    water_response: WaterResponseHint::AllowsWetMargins,
    ecology_density: EcologyDensityHint::Open,
};
