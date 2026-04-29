use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};
use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::DryShrublandKarst,
    biome_family: BiomeFamily::DryShrubland,
    terrain_form_family: TerrainFormFamily::Karst,
    summary: "Dry karst country with broken limestone relief and sparse shrub cover.",
    regional_traits: &[
        "Planning stub: sink formation, exposed stone bands, and drainage loss still need a dedicated pass.",
        "This archetype should later define how closed pockets and fissures affect hydrology.",
    ],
    ecology_notes: &[
        "Surface cover should stay thin and discontinuous across exposed stone.",
        "Detailed cave, sinkhole, and seasonal moisture notes remain to be specified.",
    ],
    allowed_meso_keys: &["sinkhole_field", "ravine"],
    seasonal_profile: SeasonalSurfaceProfile::AridSparse,
    water_response: WaterResponseHint::AvoidsStandingWater,
    ecology_density: EcologyDensityHint::Sparse,
};
