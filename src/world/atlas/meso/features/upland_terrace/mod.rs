use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "upland_terrace",
    summary: "Step-like slope deformation for plateaus, basin shoulders, and selected coasts.",
    placement_family: MesoPlacementFamily::RidgeShoulder,
    hydrology_coupling: MesoHydrologyCoupling::AvoidPrimaryCorridor,
    terrain_effects: &[
        "Introduces broad stepped elevation rather than smooth only or jagged random relief.",
        "Helps upland transitions feel authored without using hard vertical cliffs everywhere.",
    ],
    ecology_notes: &[
        "Creates repeated shelf-like habitat bands for later ecology systems.",
        "Encourages alternating soil depth and rock exposure zones.",
    ],
};
