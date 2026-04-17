use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "closed_basin",
    summary: "Closed Basin planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::BasinFloor,
    hydrology_coupling: MesoHydrologyCoupling::PrefersClosedBasin,
    terrain_effects: &[
        "Adds localized lowland or depression structure inside broader regional terrain.",
        "Prefers internally drained or weakly connected depressions.",
    ],
    ecology_notes: &[
        "Later ecology can thicken wetland, pond-edge, or dense grass cover in these pockets.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
