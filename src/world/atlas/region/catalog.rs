use super::RegionArchetype;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionCatalogStatus {
    LaunchCandidate,
    ExtendedCandidate,
    Deferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionCatalogEntry {
    pub archetype: RegionArchetype,
    pub status: RegionCatalogStatus,
    pub module_path: &'static str,
}

pub const REGION_CATALOG: &'static [RegionCatalogEntry] = &[
    RegionCatalogEntry { archetype: RegionArchetype::OceanicShelf, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/oceanic_shelf" },
    RegionCatalogEntry { archetype: RegionArchetype::SandyBeachPlain, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/sandy_beach_plain" },
    RegionCatalogEntry { archetype: RegionArchetype::CoastalCliffland, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/coastal_cliffland" },
    RegionCatalogEntry { archetype: RegionArchetype::ColdWetLowland, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/cold_wet_lowland" },
    RegionCatalogEntry { archetype: RegionArchetype::TemperatePlain, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/temperate_plain" },
    RegionCatalogEntry { archetype: RegionArchetype::TemperateHills, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/temperate_hills" },
    RegionCatalogEntry { archetype: RegionArchetype::TemperatePlateau, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/temperate_plateau" },
    RegionCatalogEntry { archetype: RegionArchetype::SteppePlain, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/steppe_plain" },
    RegionCatalogEntry { archetype: RegionArchetype::DesertPlain, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/desert_plain" },
    RegionCatalogEntry { archetype: RegionArchetype::DesertDuneField, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/desert_dune_field" },
    RegionCatalogEntry { archetype: RegionArchetype::SavannaPlain, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/savanna_plain" },
    RegionCatalogEntry { archetype: RegionArchetype::TropicalRainforestLowland, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/tropical_rainforest_lowland" },
    RegionCatalogEntry { archetype: RegionArchetype::TropicalRainforestHills, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/tropical_rainforest_hills" },
    RegionCatalogEntry { archetype: RegionArchetype::GlaciatedAlpine, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/glaciated_alpine" },
    RegionCatalogEntry { archetype: RegionArchetype::TundraPlain, status: RegionCatalogStatus::LaunchCandidate, module_path: "atlas/region/archetypes/tundra_plain" },
    RegionCatalogEntry { archetype: RegionArchetype::RockyShoreCoast, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/rocky_shore_coast" },
    RegionCatalogEntry { archetype: RegionArchetype::BarrierCoast, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/barrier_coast" },
    RegionCatalogEntry { archetype: RegionArchetype::LagoonCoast, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/lagoon_coast" },
    RegionCatalogEntry { archetype: RegionArchetype::EstuaryLowland, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/estuary_lowland" },
    RegionCatalogEntry { archetype: RegionArchetype::CoastalDelta, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/coastal_delta" },
    RegionCatalogEntry { archetype: RegionArchetype::MangroveLagoon, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/mangrove_lagoon" },
    RegionCatalogEntry { archetype: RegionArchetype::MangroveDelta, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/mangrove_delta" },
    RegionCatalogEntry { archetype: RegionArchetype::MarshFloodplain, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/marsh_floodplain" },
    RegionCatalogEntry { archetype: RegionArchetype::SwampLowland, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/swamp_lowland" },
    RegionCatalogEntry { archetype: RegionArchetype::FloodedForestAlluvialLowland, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/flooded_forest_alluvial_lowland" },
    RegionCatalogEntry { archetype: RegionArchetype::FloodedForestFloodplain, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/flooded_forest_floodplain" },
    RegionCatalogEntry { archetype: RegionArchetype::TemperateRollingPlain, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/temperate_rolling_plain" },
    RegionCatalogEntry { archetype: RegionArchetype::TemperateBasin, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/temperate_basin" },
    RegionCatalogEntry { archetype: RegionArchetype::TemperateBroadValley, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/temperate_broad_valley" },
    RegionCatalogEntry { archetype: RegionArchetype::TemperateEscarpmentUpland, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/temperate_escarpment_upland" },
    RegionCatalogEntry { archetype: RegionArchetype::TemperateBroadleafPlain, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/temperate_broadleaf_plain" },
    RegionCatalogEntry { archetype: RegionArchetype::TemperateMixedHills, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/temperate_mixed_hills" },
    RegionCatalogEntry { archetype: RegionArchetype::BorealPlain, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/boreal_plain" },
    RegionCatalogEntry { archetype: RegionArchetype::BorealHills, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/boreal_hills" },
    RegionCatalogEntry { archetype: RegionArchetype::BorealWetLowland, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/boreal_wet_lowland" },
    RegionCatalogEntry { archetype: RegionArchetype::SteppeHills, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/steppe_hills" },
    RegionCatalogEntry { archetype: RegionArchetype::SemiDesertPediment, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/semi_desert_pediment" },
    RegionCatalogEntry { archetype: RegionArchetype::DryShrublandBadlands, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/dry_shrubland_badlands" },
    RegionCatalogEntry { archetype: RegionArchetype::DryShrublandKarst, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/dry_shrubland_karst" },
    RegionCatalogEntry { archetype: RegionArchetype::MediterraneanShrublandHills, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/mediterranean_shrubland_hills" },
    RegionCatalogEntry { archetype: RegionArchetype::DesertBasin, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/desert_basin" },
    RegionCatalogEntry { archetype: RegionArchetype::DesertMesaCountry, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/desert_mesa_country" },
    RegionCatalogEntry { archetype: RegionArchetype::SavannaHills, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/savanna_hills" },
    RegionCatalogEntry { archetype: RegionArchetype::TropicalDryForestHills, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/tropical_dry_forest_hills" },
    RegionCatalogEntry { archetype: RegionArchetype::MonsoonFloodplain, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/monsoon_floodplain" },
    RegionCatalogEntry { archetype: RegionArchetype::SubalpineWoodedFront, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/subalpine_wooded_front" },
    RegionCatalogEntry { archetype: RegionArchetype::AlpineMeadowMountain, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/alpine_meadow_mountain" },
    RegionCatalogEntry { archetype: RegionArchetype::PolarBarrensPlain, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/polar_barrens_plain" },
    RegionCatalogEntry { archetype: RegionArchetype::MonsoonDelta, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/monsoon_delta" },
    RegionCatalogEntry { archetype: RegionArchetype::CrevassedIcefield, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/crevassed_icefield" },
    RegionCatalogEntry { archetype: RegionArchetype::GlacialValley, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/glacial_valley" },
    RegionCatalogEntry { archetype: RegionArchetype::DesertAlluvialFan, status: RegionCatalogStatus::ExtendedCandidate, module_path: "atlas/region/archetypes/desert_alluvial_fan" },
    RegionCatalogEntry { archetype: RegionArchetype::FjordCoast, status: RegionCatalogStatus::Deferred, module_path: "atlas/region/archetypes/fjord_coast" },
    RegionCatalogEntry { archetype: RegionArchetype::BorealRidgeCountry, status: RegionCatalogStatus::Deferred, module_path: "atlas/region/archetypes/boreal_ridge_country" },
    RegionCatalogEntry { archetype: RegionArchetype::MonsoonPlateau, status: RegionCatalogStatus::Deferred, module_path: "atlas/region/archetypes/monsoon_plateau" },
    RegionCatalogEntry { archetype: RegionArchetype::AlpineRavineCountry, status: RegionCatalogStatus::Deferred, module_path: "atlas/region/archetypes/alpine_ravine_country" },
];

pub fn region_catalog_entries() -> &'static [RegionCatalogEntry] {
    REGION_CATALOG
}
