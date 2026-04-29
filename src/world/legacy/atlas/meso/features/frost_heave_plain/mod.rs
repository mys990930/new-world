use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "frost_heave_plain",
    summary: "Frost Heave Plain planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::CryoSurface,
    hydrology_coupling: MesoHydrologyCoupling::None,
    terrain_effects: &[
        "Adds cold-climate landform accents tied to freeze, snow, or ice processes.",
        "Should be resolved mainly through archetype context and local relief budget.",
    ],
    ecology_notes: &[
        "Later ecology can bias snow persistence, frost-tolerant cover, or exposed ice margins.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
