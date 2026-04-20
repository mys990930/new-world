use super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub mod oceanic_shelf;
pub mod sandy_beach_plain;
pub mod coastal_cliffland;
pub mod cold_wet_lowland;
pub mod temperate_plain;
pub mod temperate_hills;
pub mod temperate_plateau;
pub mod steppe_plain;
pub mod desert_plain;
pub mod desert_dune_field;
pub mod savanna_plain;
pub mod tropical_rainforest_lowland;
pub mod tropical_rainforest_hills;
pub mod glaciated_alpine;
pub mod tundra_plain;
pub mod rocky_shore_coast;
pub mod barrier_coast;
pub mod lagoon_coast;
pub mod estuary_lowland;
pub mod coastal_delta;
pub mod mangrove_lagoon;
pub mod mangrove_delta;
pub mod marsh_floodplain;
pub mod swamp_lowland;
pub mod flooded_forest_alluvial_lowland;
pub mod flooded_forest_floodplain;
pub mod temperate_rolling_plain;
pub mod temperate_basin;
pub mod temperate_broad_valley;
pub mod temperate_escarpment_upland;
pub mod temperate_broadleaf_plain;
pub mod temperate_mixed_hills;
pub mod boreal_plain;
pub mod boreal_hills;
pub mod boreal_wet_lowland;
pub mod steppe_hills;
pub mod semi_desert_pediment;
pub mod dry_shrubland_badlands;
pub mod dry_shrubland_karst;
pub mod mediterranean_shrubland_hills;
pub mod desert_basin;
pub mod desert_mesa_country;
pub mod savanna_hills;
pub mod tropical_dry_forest_hills;
pub mod monsoon_floodplain;
pub mod subalpine_wooded_front;
pub mod alpine_meadow_mountain;
pub mod polar_barrens_plain;
pub mod monsoon_delta;
pub mod crevassed_icefield;
pub mod glacial_valley;
pub mod desert_alluvial_fan;
pub mod fjord_coast;
pub mod boreal_ridge_country;
pub mod monsoon_plateau;
pub mod alpine_ravine_country;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrototypeArchetypeHint {
    pub macro_height_bonus_delta: f32,
    pub wet_flatten_delta: f32,
    pub low_freq_amp_scale: f32,
    pub mid_freq_amp_scale: f32,
    pub terrace_amp_scale: f32,
    pub relief_base_scale: f32,
    pub relief_gain_scale: f32,
    pub corridor_depth_scale: f32,
    pub floodplain_width_scale: f32,
    pub ridge_lift_scale: f32,
    pub ridge_shoulder_lift_scale: f32,
    pub ridge_preservation_scale: f32,
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

pub const REGION_ARCHETYPE_DEFS: &'static [RegionArchetypeDef] = &[
    oceanic_shelf::DEF,
    sandy_beach_plain::DEF,
    coastal_cliffland::DEF,
    cold_wet_lowland::DEF,
    temperate_plain::DEF,
    temperate_hills::DEF,
    temperate_plateau::DEF,
    steppe_plain::DEF,
    desert_plain::DEF,
    desert_dune_field::DEF,
    savanna_plain::DEF,
    tropical_rainforest_lowland::DEF,
    tropical_rainforest_hills::DEF,
    glaciated_alpine::DEF,
    tundra_plain::DEF,
    rocky_shore_coast::DEF,
    barrier_coast::DEF,
    lagoon_coast::DEF,
    estuary_lowland::DEF,
    coastal_delta::DEF,
    mangrove_lagoon::DEF,
    mangrove_delta::DEF,
    marsh_floodplain::DEF,
    swamp_lowland::DEF,
    flooded_forest_alluvial_lowland::DEF,
    flooded_forest_floodplain::DEF,
    temperate_rolling_plain::DEF,
    temperate_basin::DEF,
    temperate_broad_valley::DEF,
    temperate_escarpment_upland::DEF,
    temperate_broadleaf_plain::DEF,
    temperate_mixed_hills::DEF,
    boreal_plain::DEF,
    boreal_hills::DEF,
    boreal_wet_lowland::DEF,
    steppe_hills::DEF,
    semi_desert_pediment::DEF,
    dry_shrubland_badlands::DEF,
    dry_shrubland_karst::DEF,
    mediterranean_shrubland_hills::DEF,
    desert_basin::DEF,
    desert_mesa_country::DEF,
    savanna_hills::DEF,
    tropical_dry_forest_hills::DEF,
    monsoon_floodplain::DEF,
    subalpine_wooded_front::DEF,
    alpine_meadow_mountain::DEF,
    polar_barrens_plain::DEF,
    monsoon_delta::DEF,
    crevassed_icefield::DEF,
    glacial_valley::DEF,
    desert_alluvial_fan::DEF,
    fjord_coast::DEF,
    boreal_ridge_country::DEF,
    monsoon_plateau::DEF,
    alpine_ravine_country::DEF,
];

pub fn region_archetype_defs() -> &'static [RegionArchetypeDef] {
    REGION_ARCHETYPE_DEFS
}

pub fn region_archetype_def(id: RegionArchetype) -> Option<&'static RegionArchetypeDef> {
    REGION_ARCHETYPE_DEFS.iter().find(|def| def.id == id)
}

pub fn region_archetype_prototype_hint(
    id: RegionArchetype,
) -> Option<&'static PrototypeArchetypeHint> {
    match id {
        RegionArchetype::TemperatePlain => Some(&temperate_plain::PROTOTYPE_HINT),
        RegionArchetype::GlaciatedAlpine => Some(&glaciated_alpine::PROTOTYPE_HINT),
        _ => None,
    }
}
