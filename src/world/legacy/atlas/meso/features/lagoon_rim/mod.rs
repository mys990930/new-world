use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "lagoon_rim",
    summary: "Lagoon Rim planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::CoastalSediment,
    hydrology_coupling: MesoHydrologyCoupling::RequiresCoast,
    terrain_effects: &[
        "Adds shoreline sediment or lagoon-adjacent landform accents.",
        "Only valid when coastal context has already been resolved.",
    ],
    ecology_notes: &[
        "Later ecology can bias beach grass, mangrove fringe, or lagoon-edge cover here.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
