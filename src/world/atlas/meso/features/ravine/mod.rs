use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "ravine",
    summary: "Ravine planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::ValleyFloor,
    hydrology_coupling: MesoHydrologyCoupling::AvoidPrimaryCorridor,
    terrain_effects: &[
        "Sharpens valley-like incision or localized linear lowland structure.",
        "Should avoid displacing major river corridors and instead sit beside or above them.",
    ],
    ecology_notes: &[
        "Later ecology can use these cuts to channel denser vegetation, shade, or runoff-biased cover.",
        "This candidate is in the near-term planning set, so archetype allowances and deformation operators should be locked first.",
    ],
};
