use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "icefall_breakup",
    summary: "Icefall Breakup planning stub for a multi-chunk terrain accent.",
    placement_family: MesoPlacementFamily::SpecialThreeDimensional,
    hydrology_coupling: MesoHydrologyCoupling::SpecialCase,
    terrain_effects: &[
        "Likely needs dedicated geometry or later special-case realization beyond the first heightfield pass.",
        "Should stay stubbed until a dedicated realization pass exists.",
    ],
    ecology_notes: &[
        "Ecology hooks should wait until the final geometry and traversal implications are clearer.",
        "This candidate is intentionally deferred until later terrain or special-geometry passes are ready.",
    ],
};
