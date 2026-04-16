pub mod coastal_cliff_band;
pub mod crater;
pub mod dune_field;
pub mod escarpment_band;
pub mod hill_cluster;
pub mod ravine;
pub mod shallow_basin;
pub mod upland_terrace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MesoPlacementFamily {
    InteriorLandform,
    RidgeShoulder,
    BasinFloor,
    CoastalEdge,
    AridExposure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MesoHydrologyCoupling {
    None,
    AvoidPrimaryCorridor,
    SupportsFloodplain,
    RequiresCoast,
    PrefersAridRunoff,
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
    shallow_basin::DEF,
    escarpment_band::DEF,
    upland_terrace::DEF,
    ravine::DEF,
    coastal_cliff_band::DEF,
    dune_field::DEF,
    crater::DEF,
];

pub fn meso_feature_defs() -> &'static [MesoFeatureDef] {
    MESO_FEATURE_DEFS
}

pub fn meso_feature_def(key: &str) -> Option<&'static MesoFeatureDef> {
    MESO_FEATURE_DEFS.iter().find(|def| def.key == key)
}
