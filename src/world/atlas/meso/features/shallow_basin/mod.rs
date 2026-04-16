use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "shallow_basin",
    summary: "Broad depressions that create lowland identity before full lake or marsh systems exist.",
    placement_family: MesoPlacementFamily::BasinFloor,
    hydrology_coupling: MesoHydrologyCoupling::SupportsFloodplain,
    terrain_effects: &[
        "Lowers a several-chunk area to invite wetlands, ponds, or soft lowlands later.",
        "Provides readable terrain ownership without needing hard cliff walls.",
    ],
    ecology_notes: &[
        "Supports wetland, meadow, or seasonally waterlogged ecology later.",
        "Can transition to exposed mud or frozen edges depending on season.",
    ],
};
