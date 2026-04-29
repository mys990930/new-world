use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "cirque_basin",
    summary: "Cirque Basin planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::CryoSurface,
    hydrology_coupling: MesoHydrologyCoupling::RequiresCryoDrainage,
    terrain_effects: &[
        "Adds cold-climate landform accents tied to freeze, snow, or ice processes.",
        "Needs glacial or cold-drainage context rather than ordinary river logic.",
    ],
    ecology_notes: &[
        "Later ecology can bias snow persistence, frost-tolerant cover, or exposed ice margins.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
