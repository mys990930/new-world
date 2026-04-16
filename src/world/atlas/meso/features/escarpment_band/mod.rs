use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "escarpment_band",
    summary: "Strong several-chunk terrain edge that makes plateau and upland transitions legible.",
    placement_family: MesoPlacementFamily::RidgeShoulder,
    hydrology_coupling: MesoHydrologyCoupling::AvoidPrimaryCorridor,
    terrain_effects: &[
        "Creates a clear break in elevation without random stair noise.",
        "Adds silhouette-defining edges for plateau or ridge transitions.",
    ],
    ecology_notes: &[
        "Promotes exposed rock and slope-specialized ecology later.",
        "Can cast rain-shadow or wind-exposed patterns in later systems.",
    ],
};
