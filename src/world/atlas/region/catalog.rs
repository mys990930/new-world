use super::RegionArchetype;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionCatalogStatus {
    Draft,
    LaunchCandidate,
    Deferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionCatalogEntry {
    pub archetype: RegionArchetype,
    pub status: RegionCatalogStatus,
    pub module_path: &'static str,
}

pub const REGION_CATALOG: &[RegionCatalogEntry] = &[
    RegionCatalogEntry {
        archetype: RegionArchetype::TemperatePlain,
        status: RegionCatalogStatus::LaunchCandidate,
        module_path: "atlas/region/archetypes/temperate_plain",
    },
    RegionCatalogEntry {
        archetype: RegionArchetype::TemperatePlateau,
        status: RegionCatalogStatus::LaunchCandidate,
        module_path: "atlas/region/archetypes/temperate_plateau",
    },
    RegionCatalogEntry {
        archetype: RegionArchetype::TemperateHills,
        status: RegionCatalogStatus::LaunchCandidate,
        module_path: "atlas/region/archetypes/temperate_hills",
    },
    RegionCatalogEntry {
        archetype: RegionArchetype::TropicalRainforestLowland,
        status: RegionCatalogStatus::LaunchCandidate,
        module_path: "atlas/region/archetypes/tropical_rainforest_lowland",
    },
    RegionCatalogEntry {
        archetype: RegionArchetype::TropicalRainforestHills,
        status: RegionCatalogStatus::LaunchCandidate,
        module_path: "atlas/region/archetypes/tropical_rainforest_hills",
    },
    RegionCatalogEntry {
        archetype: RegionArchetype::DesertPlain,
        status: RegionCatalogStatus::LaunchCandidate,
        module_path: "atlas/region/archetypes/desert_plain",
    },
    RegionCatalogEntry {
        archetype: RegionArchetype::WetLowland,
        status: RegionCatalogStatus::LaunchCandidate,
        module_path: "atlas/region/archetypes/cold_wet_lowland",
    },
    RegionCatalogEntry {
        archetype: RegionArchetype::ColdMountainUpland,
        status: RegionCatalogStatus::Deferred,
        module_path: "atlas/region/archetypes/glaciated_alpine",
    },
    RegionCatalogEntry {
        archetype: RegionArchetype::CoastalBeach,
        status: RegionCatalogStatus::Deferred,
        module_path: "atlas/region/archetypes/coastal_cliffland",
    },
];

pub fn region_catalog_entries() -> &'static [RegionCatalogEntry] {
    REGION_CATALOG
}
