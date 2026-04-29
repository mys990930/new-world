use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "alluvial_fan",
    summary: "Alluvial Fan planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::FloodplainMargin,
    hydrology_coupling: MesoHydrologyCoupling::SupportsFloodplain,
    terrain_effects: &[
        "Shapes secondary lowland edges around channels or alluvial surfaces.",
        "May widen into soft lowlands that later cooperate with floodplain material policy.",
    ],
    ecology_notes: &[
        "Later ecology can emphasize riparian strips, sediment bars, or seasonal wet margins.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
