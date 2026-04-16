use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "dune_field",
    summary: "Wind-shaped arid relief patch that gives deserts readable local identity.",
    placement_family: MesoPlacementFamily::AridExposure,
    hydrology_coupling: MesoHydrologyCoupling::PrefersAridRunoff,
    terrain_effects: &[
        "Adds directional sand relief without turning the whole desert into uniform waves.",
        "Should remain bounded and archetype-driven rather than everywhere in arid land.",
    ],
    ecology_notes: &[
        "Keeps vegetation sparse and tied to exceptional moisture pockets.",
        "Can create lee-side accumulation and exposed crests for later ecology bias.",
    ],
};
