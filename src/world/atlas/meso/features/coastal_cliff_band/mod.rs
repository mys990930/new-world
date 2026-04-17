use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "coastal_cliff_band",
    summary: "Coastal Cliff Band planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::CoastalEdge,
    hydrology_coupling: MesoHydrologyCoupling::RequiresCoast,
    terrain_effects: &[
        "Breaks up shoreline or coast-parallel terrain without replacing the owning coastal archetype.",
        "Only valid when coastal context has already been resolved.",
    ],
    ecology_notes: &[
        "Later ecology can separate exposed cliff or spray-tolerant cover from inland cover.",
        "This candidate is in the near-term planning set, so archetype allowances and deformation operators should be locked first.",
    ],
};
