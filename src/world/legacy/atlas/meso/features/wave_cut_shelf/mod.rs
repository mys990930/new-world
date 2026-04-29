use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "wave_cut_shelf",
    summary: "Wave Cut Shelf planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::CoastalEdge,
    hydrology_coupling: MesoHydrologyCoupling::RequiresCoast,
    terrain_effects: &[
        "Breaks up shoreline or coast-parallel terrain without replacing the owning coastal archetype.",
        "Only valid when coastal context has already been resolved.",
    ],
    ecology_notes: &[
        "Later ecology can separate exposed cliff or spray-tolerant cover from inland cover.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
