use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "ravine",
    summary: "Narrow incision feature for hill and upland terrain that reinforces runoff structure.",
    placement_family: MesoPlacementFamily::RidgeShoulder,
    hydrology_coupling: MesoHydrologyCoupling::SupportsFloodplain,
    terrain_effects: &[
        "Cuts a readable narrow valley into already-nonflat terrain.",
        "Works best when aligned with real drainage instead of random carving.",
    ],
    ecology_notes: &[
        "Can support wetter, shadier ecology than adjacent slopes.",
        "May host exposed rock walls and denser riparian strips later.",
    ],
};
