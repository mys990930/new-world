use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "dune_field",
    summary: "Dune Field planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::AridExposure,
    hydrology_coupling: MesoHydrologyCoupling::PrefersAridRunoff,
    terrain_effects: &[
        "Introduces erosion- or aridity-driven forms under dry regional conditions.",
        "Favors dry runoff, sediment, or wind-shaped settings over wet lowlands.",
    ],
    ecology_notes: &[
        "Later ecology can bias sparse scrub, exposed sediment, or dune-tolerant cover here.",
        "This candidate is in the near-term planning set, so archetype allowances and deformation operators should be locked first.",
    ],
};
