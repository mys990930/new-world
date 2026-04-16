use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "coastal_cliff_band",
    summary: "Marine-facing cliff accent for coast archetypes that should not read like inland escarpments.",
    placement_family: MesoPlacementFamily::CoastalEdge,
    hydrology_coupling: MesoHydrologyCoupling::RequiresCoast,
    terrain_effects: &[
        "Creates shore-facing vertical drama without breaking macro shoreline direction.",
        "Should cooperate with coast sediment policy rather than replace it.",
    ],
    ecology_notes: &[
        "Supports sparse salt-tolerant cover and exposed rock ecology later.",
        "Can pair with coves or rocky shelves in later coast systems.",
    ],
};
