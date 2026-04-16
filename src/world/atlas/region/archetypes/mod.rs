use super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub mod coastal_cliffland;
pub mod cold_wet_lowland;
pub mod desert_plain;
pub mod glaciated_alpine;
pub mod temperate_hills;
pub mod temperate_plain;
pub mod temperate_plateau;
pub mod tropical_rainforest_hills;
pub mod tropical_rainforest_lowland;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcologyDensityHint {
    Barren,
    Sparse,
    Open,
    Dense,
    Saturated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterResponseHint {
    AvoidsStandingWater,
    AllowsWetMargins,
    EmbracesFloodplain,
    GlacialMeltDriven,
    CoastalSprayExposed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeasonalSurfaceProfile {
    TemperateFourSeason,
    TemperateSnowCapable,
    TropicalWetDry,
    AridSparse,
    ColdFreezeThaw,
    AlpineSnowPersistent,
    CoastalTemperate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionArchetypeDef {
    pub id: RegionArchetype,
    pub biome_family: BiomeFamily,
    pub terrain_form_family: TerrainFormFamily,
    pub summary: &'static str,
    pub regional_traits: &'static [&'static str],
    pub ecology_notes: &'static [&'static str],
    pub allowed_meso_keys: &'static [&'static str],
    pub seasonal_profile: SeasonalSurfaceProfile,
    pub water_response: WaterResponseHint,
    pub ecology_density: EcologyDensityHint,
}

pub const REGION_ARCHETYPE_DEFS: &[RegionArchetypeDef] = &[
    temperate_plain::DEF,
    temperate_plateau::DEF,
    temperate_hills::DEF,
    tropical_rainforest_lowland::DEF,
    tropical_rainforest_hills::DEF,
    desert_plain::DEF,
    cold_wet_lowland::DEF,
    glaciated_alpine::DEF,
    coastal_cliffland::DEF,
];

pub fn region_archetype_defs() -> &'static [RegionArchetypeDef] {
    REGION_ARCHETYPE_DEFS
}

pub fn region_archetype_def(id: RegionArchetype) -> Option<&'static RegionArchetypeDef> {
    REGION_ARCHETYPE_DEFS.iter().find(|def| def.id == id)
}
