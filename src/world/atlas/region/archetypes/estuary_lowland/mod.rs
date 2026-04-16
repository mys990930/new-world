use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::EstuaryLowland,
    biome_family: BiomeFamily::EstuarineCoast,
    terrain_form_family: TerrainFormFamily::EstuaryLowland,
    summary: "Brackish river mouth lowland and tidal reach.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["shallow_basin", "ravine",],
    seasonal_profile: SeasonalSurfaceProfile::CoastalTemperate,
    water_response: WaterResponseHint::EmbracesFloodplain,
    ecology_density: EcologyDensityHint::Sparse,
};
