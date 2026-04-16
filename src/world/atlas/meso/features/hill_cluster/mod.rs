use super::{MesoFeatureDef, MesoHydrologyCoupling, MesoPlacementFamily};

pub const DEF: MesoFeatureDef = MesoFeatureDef {
    key: "hill_cluster",
    summary: "Broad low upland bumps that make plains and gentle uplands readable in a small play view.",
    placement_family: MesoPlacementFamily::InteriorLandform,
    hydrology_coupling: MesoHydrologyCoupling::AvoidPrimaryCorridor,
    terrain_effects: &[
        "Raises clustered local relief without changing the owning archetype.",
        "Breaks broad flats into traversable mounds instead of per-block noise.",
    ],
    ecology_notes: &[
        "Can support denser tree belts or open woodland later.",
        "Creates localized dry shoulders around otherwise even plains.",
    ],
};
