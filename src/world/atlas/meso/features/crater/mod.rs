use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "crater",
    summary: "Round impact or volcanic depression accent used sparingly for strong local identity.",
    placement_family: MesoPlacementFamily::InteriorLandform,
    hydrology_coupling: MesoHydrologyCoupling::SupportsFloodplain,
    terrain_effects: &[
        "Creates a clear circular basin with rim emphasis.",
        "Can host later water, lava, or bare-rock fill policies depending on archetype.",
    ],
    ecology_notes: &[
        "Supports unusual localized ecology compared with the surrounding region.",
        "Can remain barren or hold wet pockets depending on material policy later.",
    ],
};
