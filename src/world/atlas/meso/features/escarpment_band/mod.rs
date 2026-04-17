use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "escarpment_band",
    summary: "Escarpment Band planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::RidgeShoulder,
    hydrology_coupling: MesoHydrologyCoupling::AvoidPrimaryCorridor,
    terrain_effects: &[
        "Modulates ridge shoulders, upland steps, or slope breaks under an already-classified archetype.",
        "Should avoid displacing major river corridors and instead sit beside or above them.",
    ],
    ecology_notes: &[
        "Later ecology can bias sparse cover, wind exposure, or stepped vegetation belts here.",
        "This candidate is in the near-term planning set, so archetype allowances and deformation operators should be locked first.",
    ],
};
