use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "shoulder_shelf",
    summary: "Shoulder Shelf planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::RidgeShoulder,
    hydrology_coupling: MesoHydrologyCoupling::AvoidPrimaryCorridor,
    terrain_effects: &[
        "Modulates ridge shoulders, upland steps, or slope breaks under an already-classified archetype.",
        "Should avoid displacing major river corridors and instead sit beside or above them.",
    ],
    ecology_notes: &[
        "Later ecology can bias sparse cover, wind exposure, or stepped vegetation belts here.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
