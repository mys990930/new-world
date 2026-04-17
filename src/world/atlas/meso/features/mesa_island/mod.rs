use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "mesa_island",
    summary: "Mesa Island planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::AridExposure,
    hydrology_coupling: MesoHydrologyCoupling::None,
    terrain_effects: &[
        "Introduces erosion- or aridity-driven forms under dry regional conditions.",
        "Should be resolved mainly through archetype context and local relief budget.",
    ],
    ecology_notes: &[
        "Later ecology can bias sparse scrub, exposed sediment, or dune-tolerant cover here.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
