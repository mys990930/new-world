use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::DryShrublandBadlands,
    biome_family: BiomeFamily::DryShrubland,
    terrain_form_family: TerrainFormFamily::Badlands,
    summary: "Dry eroded badlands with sparse scrub pockets and exposed sediment ribs.",
    regional_traits: &[
        "Planning stub: erosion-driven shape rules still need a dedicated V2 terrain pass.",
        "This archetype should later lock sharper sediment ownership and runoff responses.",
    ],
    ecology_notes: &[
        "Dry shrubland cover should stay sparse and broken by exposed ground.",
        "Detailed seasonal color shift and habitat density remain to be specified.",
    ],
    allowed_meso_keys: &["badlands_patch", "ravine"],
    seasonal_profile: SeasonalSurfaceProfile::AridSparse,
    water_response: WaterResponseHint::AvoidsStandingWater,
    ecology_density: EcologyDensityHint::Sparse,
};
