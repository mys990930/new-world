use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::BarrierCoast,
    biome_family: BiomeFamily::SandyCoast,
    terrain_form_family: TerrainFormFamily::BarrierCoast,
    summary: "Long barrier-backed coast with sediment strips.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["dune_field", "shallow_basin",],
    seasonal_profile: SeasonalSurfaceProfile::CoastalTemperate,
    water_response: WaterResponseHint::CoastalSprayExposed,
    ecology_density: EcologyDensityHint::Sparse,
};
