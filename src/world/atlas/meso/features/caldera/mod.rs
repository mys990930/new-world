use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "caldera",
    summary: "Caldera planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::VolcanicField,
    hydrology_coupling: MesoHydrologyCoupling::SpecialCase,
    terrain_effects: &[
        "Introduces volcanic relief accents without redefining macro mountain ownership.",
        "Should stay stubbed until a dedicated realization pass exists.",
    ],
    ecology_notes: &[
        "Later ecology can emphasize sparse pioneer cover and exposed mineral surfaces.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
