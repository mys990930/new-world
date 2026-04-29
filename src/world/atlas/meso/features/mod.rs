pub mod alluvial_fan;
pub mod backshore_dune_field;
pub mod badlands_patch;
pub mod barrier_spit;
pub mod broad_valley;
pub mod caldera;
pub mod canyon_reach;
pub mod cinder_cone_cluster;
pub mod cirque_basin;
pub mod closed_basin;
pub mod coastal_cliff_band;
pub mod cove_breakup;
pub mod crater;
pub mod creek_corridor;
pub mod crevasse_belt;
pub mod delta_lobe;
pub mod distributary_fan;
pub mod dry_gully_network;
pub mod dune_field;
pub mod eroded_butte_field;
pub mod escarpment_band;
pub mod fault_scarp;
pub mod fissure_ridge;
pub mod fjord_wall_breakup;
pub mod frost_heave_plain;
pub mod glacial_bench;
pub mod glacial_trough;
pub mod gorge_cut;
pub mod hill_cluster;
pub mod icefall_breakup;
pub mod lagoon_rim;
pub mod lava_field;
pub mod levee_strip;
pub mod linear_dune_belt;
pub mod marsh_flat;
pub mod mesa_cluster;
pub mod mesa_island;
pub mod moraine_belt;
pub mod narrow_valley;
pub mod natural_arch;
pub mod oxbow_lowland;
pub mod peaty_hollow;
pub mod pediment_steps;
pub mod permafrost_pingo_field;
pub mod ravine;
pub mod ridge_spur;
pub mod rocky_headland_chain;
pub mod rolling_hill_belt;
pub mod sea_stack_cluster;
pub mod secondary_channel_belt;
pub mod shallow_basin;
pub mod shoulder_shelf;
pub mod sinkhole_field;
pub mod snow_basin;
pub mod spring_basin;
pub mod summit_group;
pub mod terraced_floodplain;
pub mod tidal_flat_bench;
pub mod upland_knob_field;
pub mod upland_terrace;
pub mod volcanic_terrace;
pub mod wave_cut_shelf;
pub mod wet_basin;
pub mod wet_meadow_bowl;
pub mod yardang_band;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MesoPlacementFamily {
    InteriorLandform,
    RidgeShoulder,
    BasinFloor,
    ValleyFloor,
    FloodplainMargin,
    RiverCorridor,
    CoastalEdge,
    CoastalSediment,
    AridExposure,
    CryoSurface,
    VolcanicField,
    SpecialThreeDimensional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MesoHydrologyCoupling {
    None,
    AvoidPrimaryCorridor,
    SupportsFloodplain,
    RequiresRiverCorridor,
    RequiresCoast,
    PrefersAridRunoff,
    PrefersClosedBasin,
    RequiresSeasonalWetness,
    RequiresCryoDrainage,
    SpecialCase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MesoFeatureDef {
    pub key: &'static str,
    pub summary: &'static str,
    pub placement_family: MesoPlacementFamily,
    pub hydrology_coupling: MesoHydrologyCoupling,
    pub terrain_effects: &'static [&'static str],
    pub ecology_notes: &'static [&'static str],
}

pub const MESO_FEATURE_DEFS: &[MesoFeatureDef] = &[
    hill_cluster::DEF,
    rolling_hill_belt::DEF,
    ridge_spur::DEF,
    summit_group::DEF,
    upland_knob_field::DEF,
    mesa_island::DEF,
    upland_terrace::DEF,
    escarpment_band::DEF,
    fault_scarp::DEF,
    shoulder_shelf::DEF,
    shallow_basin::DEF,
    closed_basin::DEF,
    wet_basin::DEF,
    sinkhole_field::DEF,
    broad_valley::DEF,
    narrow_valley::DEF,
    ravine::DEF,
    canyon_reach::DEF,
    gorge_cut::DEF,
    cirque_basin::DEF,
    creek_corridor::DEF,
    secondary_channel_belt::DEF,
    alluvial_fan::DEF,
    terraced_floodplain::DEF,
    levee_strip::DEF,
    oxbow_lowland::DEF,
    delta_lobe::DEF,
    distributary_fan::DEF,
    coastal_cliff_band::DEF,
    rocky_headland_chain::DEF,
    cove_breakup::DEF,
    barrier_spit::DEF,
    lagoon_rim::DEF,
    tidal_flat_bench::DEF,
    backshore_dune_field::DEF,
    wave_cut_shelf::DEF,
    sea_stack_cluster::DEF,
    fjord_wall_breakup::DEF,
    dune_field::DEF,
    linear_dune_belt::DEF,
    badlands_patch::DEF,
    yardang_band::DEF,
    dry_gully_network::DEF,
    pediment_steps::DEF,
    mesa_cluster::DEF,
    eroded_butte_field::DEF,
    glacial_trough::DEF,
    moraine_belt::DEF,
    crevasse_belt::DEF,
    icefall_breakup::DEF,
    permafrost_pingo_field::DEF,
    frost_heave_plain::DEF,
    snow_basin::DEF,
    glacial_bench::DEF,
    crater::DEF,
    caldera::DEF,
    lava_field::DEF,
    cinder_cone_cluster::DEF,
    fissure_ridge::DEF,
    volcanic_terrace::DEF,
    marsh_flat::DEF,
    peaty_hollow::DEF,
    spring_basin::DEF,
    wet_meadow_bowl::DEF,
    natural_arch::DEF,
];

pub fn meso_feature_defs() -> &'static [MesoFeatureDef] {
    MESO_FEATURE_DEFS
}

pub fn meso_feature_def(key: &str) -> Option<&'static MesoFeatureDef> {
    MESO_FEATURE_DEFS.iter().find(|def| def.key == key)
}
