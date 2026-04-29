use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "creek_corridor",
    summary: "Creek Corridor planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::RiverCorridor,
    hydrology_coupling: MesoHydrologyCoupling::RequiresRiverCorridor,
    terrain_effects: &[
        "Tracks water-adjacent terrain accents that should follow secondary drainage logic.",
        "Only makes sense when a river or distributary corridor is already present.",
    ],
    ecology_notes: &[
        "Later ecology can align vegetation and sediment variation with secondary drainage lines.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
