use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::TemperateHills,
    biome_family: BiomeFamily::TemperateGrassland,
    terrain_form_family: TerrainFormFamily::HillCountry,
    summary: "Rolling inland upland where ridge-spur and ravine accents are expected.",
    regional_traits: &[
        "Baseline shape should already feel undulating before meso detail is added.",
        "Stream starts and slope transitions should feel more common than on plains.",
    ],
    ecology_notes: &[
        "Supports mosaics of grassland, shrubs, and later deciduous woodland belts.",
        "Cool-facing slopes can hold snow longer than adjacent flats.",
    ],
    allowed_meso_keys: &["hill_cluster", "ravine", "upland_terrace"],
    seasonal_profile: SeasonalSurfaceProfile::TemperateSnowCapable,
    water_response: WaterResponseHint::AllowsWetMargins,
    ecology_density: EcologyDensityHint::Open,
};
