use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "gorge_cut",
    summary: "Gorge Cut planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::ValleyFloor,
    hydrology_coupling: MesoHydrologyCoupling::RequiresRiverCorridor,
    terrain_effects: &[
        "Sharpens valley-like incision or localized linear lowland structure.",
        "Only makes sense when a river or distributary corridor is already present.",
    ],
    ecology_notes: &[
        "Later ecology can use these cuts to channel denser vegetation, shade, or runoff-biased cover.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
