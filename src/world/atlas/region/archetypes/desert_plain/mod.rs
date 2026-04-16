use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::DesertPlain,
    biome_family: BiomeFamily::Desert,
    terrain_form_family: TerrainFormFamily::Plain,
    summary: "Dry exposed plain where sediment ownership dominates and water is exceptional.",
    regional_traits: &[
        "Relief should stay readable through broad dunes, fans, and arid runoff traces.",
        "Vegetated cover should be sparse and strongly tied to hydrology exceptions.",
    ],
    ecology_notes: &[
        "Supports sparse scrub, exposed sediment, and harsh heat response later.",
        "Wet season events should create short-lived green pulses rather than permanent cover.",
    ],
    allowed_meso_keys: &["dune_field", "crater", "ravine"],
    seasonal_profile: SeasonalSurfaceProfile::AridSparse,
    water_response: WaterResponseHint::AvoidsStandingWater,
    ecology_density: EcologyDensityHint::Sparse,
};
