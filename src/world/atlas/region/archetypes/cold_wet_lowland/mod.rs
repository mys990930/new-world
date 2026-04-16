use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::WetLowland,
    biome_family: BiomeFamily::Marsh,
    terrain_form_family: TerrainFormFamily::WetLowland,
    summary: "Cold, moisture-retaining lowland where freeze-thaw and wet margins define the surface.",
    regional_traits: &[
        "Standing water, marshy margins, and soft basins should be regionally coherent.",
        "Seasonal freezing should change the cover state without replacing the archetype.",
    ],
    ecology_notes: &[
        "Supports marsh vegetation, sedges, and sparse cold-tolerant cover later.",
        "Winter can freeze edges and flatten some shallow water expression.",
    ],
    allowed_meso_keys: &["shallow_basin", "ravine", "hill_cluster"],
    seasonal_profile: SeasonalSurfaceProfile::ColdFreezeThaw,
    water_response: WaterResponseHint::EmbracesFloodplain,
    ecology_density: EcologyDensityHint::Saturated,
};
