use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::ColdMountainUpland,
    biome_family: BiomeFamily::Alpine,
    terrain_form_family: TerrainFormFamily::Mountain,
    summary: "Cold high upland with persistent snow logic and glacial carve expectations.",
    regional_traits: &[
        "Major relief belongs to the skeleton and base prototype, not local randomness.",
        "Snow persistence and meltwater corridors should both matter strongly.",
    ],
    ecology_notes: &[
        "Vegetation remains sparse and strongly elevation-limited.",
        "Short thaw windows should expose rock and wet melt paths rather than grassland.",
    ],
    allowed_meso_keys: &["upland_terrace", "ravine", "crater"],
    seasonal_profile: SeasonalSurfaceProfile::AlpineSnowPersistent,
    water_response: WaterResponseHint::GlacialMeltDriven,
    ecology_density: EcologyDensityHint::Barren,
};
