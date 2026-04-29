use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "upland_knob_field",
    summary: "Upland Knob Field planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::InteriorLandform,
    hydrology_coupling: MesoHydrologyCoupling::None,
    terrain_effects: &[
        "Biases inland terrain prototypes without taking over the owning region identity.",
        "Should be resolved mainly through archetype context and local relief budget.",
    ],
    ecology_notes: &[
        "Later ecology can use this feature to break uniform cover into readable local habitat patches.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
