use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "permafrost_pingo_field",
    summary: "Permafrost Pingo Field planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::CryoSurface,
    hydrology_coupling: MesoHydrologyCoupling::RequiresSeasonalWetness,
    terrain_effects: &[
        "Adds cold-climate landform accents tied to freeze, snow, or ice processes.",
        "Needs recurrent wetness, saturation, or thaw-season moisture to read correctly.",
    ],
    ecology_notes: &[
        "Later ecology can bias snow persistence, frost-tolerant cover, or exposed ice margins.",
        "This candidate should stay scaffolded until launch archetype coverage and core meso behavior are stable.",
    ],
};
