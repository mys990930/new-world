use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "summit_group",
    summary: "Summit Group planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::RidgeShoulder,
    hydrology_coupling: MesoHydrologyCoupling::None,
    terrain_effects: &[
        "Modulates ridge shoulders, upland steps, or slope breaks under an already-classified archetype.",
        "Should be resolved mainly through archetype context and local relief budget.",
    ],
    ecology_notes: &[
        "Later ecology can bias sparse cover, wind exposure, or stepped vegetation belts here.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
