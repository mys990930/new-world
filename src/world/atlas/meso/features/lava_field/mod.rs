use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "lava_field",
    summary: "Lava Field planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::VolcanicField,
    hydrology_coupling: MesoHydrologyCoupling::None,
    terrain_effects: &[
        "Introduces volcanic relief accents without redefining macro mountain ownership.",
        "Should be resolved mainly through archetype context and local relief budget.",
    ],
    ecology_notes: &[
        "Later ecology can emphasize sparse pioneer cover and exposed mineral surfaces.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
