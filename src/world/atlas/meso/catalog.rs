#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MesoCatalogStatus {
    Planned,
    LaunchCandidate,
    Deferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MesoCatalogEntry {
    pub key: &'static str,
    pub status: MesoCatalogStatus,
    pub module_path: &'static str,
}

pub const MESO_CATALOG: &[MesoCatalogEntry] = &[
    MesoCatalogEntry {
        key: "hill_cluster",
        status: MesoCatalogStatus::LaunchCandidate,
        module_path: "atlas/meso/features/hill_cluster",
    },
    MesoCatalogEntry {
        key: "shallow_basin",
        status: MesoCatalogStatus::LaunchCandidate,
        module_path: "atlas/meso/features/shallow_basin",
    },
    MesoCatalogEntry {
        key: "escarpment_band",
        status: MesoCatalogStatus::LaunchCandidate,
        module_path: "atlas/meso/features/escarpment_band",
    },
    MesoCatalogEntry {
        key: "upland_terrace",
        status: MesoCatalogStatus::LaunchCandidate,
        module_path: "atlas/meso/features/upland_terrace",
    },
    MesoCatalogEntry {
        key: "ravine",
        status: MesoCatalogStatus::Deferred,
        module_path: "atlas/meso/features/ravine",
    },
    MesoCatalogEntry {
        key: "coastal_cliff_band",
        status: MesoCatalogStatus::Deferred,
        module_path: "atlas/meso/features/coastal_cliff_band",
    },
    MesoCatalogEntry {
        key: "dune_field",
        status: MesoCatalogStatus::Deferred,
        module_path: "atlas/meso/features/dune_field",
    },
    MesoCatalogEntry {
        key: "crater",
        status: MesoCatalogStatus::Deferred,
        module_path: "atlas/meso/features/crater",
    },
];

pub fn meso_catalog_entries() -> &'static [MesoCatalogEntry] {
    MESO_CATALOG
}
