use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "crater",
    summary: "Crater planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::VolcanicField,
    hydrology_coupling: MesoHydrologyCoupling::None,
    terrain_effects: &[
        "Introduces volcanic relief accents without redefining macro mountain ownership.",
        "Should be resolved mainly through archetype context and local relief budget.",
    ],
    ecology_notes: &[
        "Later ecology can emphasize sparse pioneer cover and exposed mineral surfaces.",
        "This candidate is in the near-term planning set, so archetype allowances and deformation operators should be locked first.",
    ],
};
