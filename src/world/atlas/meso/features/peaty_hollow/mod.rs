use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "peaty_hollow",
    summary: "Peaty Hollow planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::BasinFloor,
    hydrology_coupling: MesoHydrologyCoupling::RequiresSeasonalWetness,
    terrain_effects: &[
        "Adds localized lowland or depression structure inside broader regional terrain.",
        "Needs recurrent wetness, saturation, or thaw-season moisture to read correctly.",
    ],
    ecology_notes: &[
        "Later ecology can thicken wetland, pond-edge, or dense grass cover in these pockets.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
