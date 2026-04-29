#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MesoCatalogStatus {
    LaunchCandidate,
    ExtendedCandidate,
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
        key: "rolling_hill_belt",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/rolling_hill_belt",
    },
    MesoCatalogEntry {
        key: "ridge_spur",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/ridge_spur",
    },
    MesoCatalogEntry {
        key: "summit_group",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/summit_group",
    },
    MesoCatalogEntry {
        key: "upland_knob_field",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/upland_knob_field",
    },
    MesoCatalogEntry {
        key: "mesa_island",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/mesa_island",
    },
    MesoCatalogEntry {
        key: "upland_terrace",
        status: MesoCatalogStatus::LaunchCandidate,
        module_path: "atlas/meso/features/upland_terrace",
    },
    MesoCatalogEntry {
        key: "escarpment_band",
        status: MesoCatalogStatus::LaunchCandidate,
        module_path: "atlas/meso/features/escarpment_band",
    },
    MesoCatalogEntry {
        key: "fault_scarp",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/fault_scarp",
    },
    MesoCatalogEntry {
        key: "shoulder_shelf",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/shoulder_shelf",
    },
    MesoCatalogEntry {
        key: "shallow_basin",
        status: MesoCatalogStatus::LaunchCandidate,
        module_path: "atlas/meso/features/shallow_basin",
    },
    MesoCatalogEntry {
        key: "closed_basin",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/closed_basin",
    },
    MesoCatalogEntry {
        key: "wet_basin",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/wet_basin",
    },
    MesoCatalogEntry {
        key: "sinkhole_field",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/sinkhole_field",
    },
    MesoCatalogEntry {
        key: "broad_valley",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/broad_valley",
    },
    MesoCatalogEntry {
        key: "narrow_valley",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/narrow_valley",
    },
    MesoCatalogEntry {
        key: "ravine",
        status: MesoCatalogStatus::LaunchCandidate,
        module_path: "atlas/meso/features/ravine",
    },
    MesoCatalogEntry {
        key: "canyon_reach",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/canyon_reach",
    },
    MesoCatalogEntry {
        key: "gorge_cut",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/gorge_cut",
    },
    MesoCatalogEntry {
        key: "cirque_basin",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/cirque_basin",
    },
    MesoCatalogEntry {
        key: "creek_corridor",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/creek_corridor",
    },
    MesoCatalogEntry {
        key: "secondary_channel_belt",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/secondary_channel_belt",
    },
    MesoCatalogEntry {
        key: "alluvial_fan",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/alluvial_fan",
    },
    MesoCatalogEntry {
        key: "terraced_floodplain",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/terraced_floodplain",
    },
    MesoCatalogEntry {
        key: "levee_strip",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/levee_strip",
    },
    MesoCatalogEntry {
        key: "oxbow_lowland",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/oxbow_lowland",
    },
    MesoCatalogEntry {
        key: "delta_lobe",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/delta_lobe",
    },
    MesoCatalogEntry {
        key: "distributary_fan",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/distributary_fan",
    },
    MesoCatalogEntry {
        key: "coastal_cliff_band",
        status: MesoCatalogStatus::LaunchCandidate,
        module_path: "atlas/meso/features/coastal_cliff_band",
    },
    MesoCatalogEntry {
        key: "rocky_headland_chain",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/rocky_headland_chain",
    },
    MesoCatalogEntry {
        key: "cove_breakup",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/cove_breakup",
    },
    MesoCatalogEntry {
        key: "barrier_spit",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/barrier_spit",
    },
    MesoCatalogEntry {
        key: "lagoon_rim",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/lagoon_rim",
    },
    MesoCatalogEntry {
        key: "tidal_flat_bench",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/tidal_flat_bench",
    },
    MesoCatalogEntry {
        key: "backshore_dune_field",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/backshore_dune_field",
    },
    MesoCatalogEntry {
        key: "wave_cut_shelf",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/wave_cut_shelf",
    },
    MesoCatalogEntry {
        key: "sea_stack_cluster",
        status: MesoCatalogStatus::Deferred,
        module_path: "atlas/meso/features/sea_stack_cluster",
    },
    MesoCatalogEntry {
        key: "fjord_wall_breakup",
        status: MesoCatalogStatus::Deferred,
        module_path: "atlas/meso/features/fjord_wall_breakup",
    },
    MesoCatalogEntry {
        key: "dune_field",
        status: MesoCatalogStatus::LaunchCandidate,
        module_path: "atlas/meso/features/dune_field",
    },
    MesoCatalogEntry {
        key: "linear_dune_belt",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/linear_dune_belt",
    },
    MesoCatalogEntry {
        key: "badlands_patch",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/badlands_patch",
    },
    MesoCatalogEntry {
        key: "yardang_band",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/yardang_band",
    },
    MesoCatalogEntry {
        key: "dry_gully_network",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/dry_gully_network",
    },
    MesoCatalogEntry {
        key: "pediment_steps",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/pediment_steps",
    },
    MesoCatalogEntry {
        key: "mesa_cluster",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/mesa_cluster",
    },
    MesoCatalogEntry {
        key: "eroded_butte_field",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/eroded_butte_field",
    },
    MesoCatalogEntry {
        key: "glacial_trough",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/glacial_trough",
    },
    MesoCatalogEntry {
        key: "moraine_belt",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/moraine_belt",
    },
    MesoCatalogEntry {
        key: "crevasse_belt",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/crevasse_belt",
    },
    MesoCatalogEntry {
        key: "icefall_breakup",
        status: MesoCatalogStatus::Deferred,
        module_path: "atlas/meso/features/icefall_breakup",
    },
    MesoCatalogEntry {
        key: "permafrost_pingo_field",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/permafrost_pingo_field",
    },
    MesoCatalogEntry {
        key: "frost_heave_plain",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/frost_heave_plain",
    },
    MesoCatalogEntry {
        key: "snow_basin",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/snow_basin",
    },
    MesoCatalogEntry {
        key: "glacial_bench",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/glacial_bench",
    },
    MesoCatalogEntry {
        key: "crater",
        status: MesoCatalogStatus::LaunchCandidate,
        module_path: "atlas/meso/features/crater",
    },
    MesoCatalogEntry {
        key: "caldera",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/caldera",
    },
    MesoCatalogEntry {
        key: "lava_field",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/lava_field",
    },
    MesoCatalogEntry {
        key: "cinder_cone_cluster",
        status: MesoCatalogStatus::Deferred,
        module_path: "atlas/meso/features/cinder_cone_cluster",
    },
    MesoCatalogEntry {
        key: "fissure_ridge",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/fissure_ridge",
    },
    MesoCatalogEntry {
        key: "volcanic_terrace",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/volcanic_terrace",
    },
    MesoCatalogEntry {
        key: "marsh_flat",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/marsh_flat",
    },
    MesoCatalogEntry {
        key: "peaty_hollow",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/peaty_hollow",
    },
    MesoCatalogEntry {
        key: "spring_basin",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/spring_basin",
    },
    MesoCatalogEntry {
        key: "wet_meadow_bowl",
        status: MesoCatalogStatus::ExtendedCandidate,
        module_path: "atlas/meso/features/wet_meadow_bowl",
    },
    MesoCatalogEntry {
        key: "natural_arch",
        status: MesoCatalogStatus::Deferred,
        module_path: "atlas/meso/features/natural_arch",
    },
];

pub fn meso_catalog_entries() -> &'static [MesoCatalogEntry] {
    MESO_CATALOG
}
