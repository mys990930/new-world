use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "crevasse_belt",
    summary: "Crevasse Belt planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::CryoSurface,
    hydrology_coupling: MesoHydrologyCoupling::SpecialCase,
    terrain_effects: &[
        "Adds cold-climate landform accents tied to freeze, snow, or ice processes.",
        "Should stay stubbed until a dedicated realization pass exists.",
    ],
    ecology_notes: &[
        "Later ecology can bias snow persistence, frost-tolerant cover, or exposed ice margins.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
