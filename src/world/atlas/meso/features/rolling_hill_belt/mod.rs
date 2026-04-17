use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "rolling_hill_belt",
    summary: "Rolling Hill Belt planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::InteriorLandform,
    hydrology_coupling: MesoHydrologyCoupling::AvoidPrimaryCorridor,
    terrain_effects: &[
        "Biases inland terrain prototypes without taking over the owning region identity.",
        "Should avoid displacing major river corridors and instead sit beside or above them.",
    ],
    ecology_notes: &[
        "Later ecology can use this feature to break uniform cover into readable local habitat patches.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
