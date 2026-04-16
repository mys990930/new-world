use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::TemperatePlateau,
    biome_family: BiomeFamily::TemperateGrassland,
    terrain_form_family: TerrainFormFamily::Plateau,
    summary: "High but readable tableland for escarpment-led transitions.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["escarpment_band", "upland_terrace", "shallow_basin",],
    seasonal_profile: SeasonalSurfaceProfile::TemperateSnowCapable,
    water_response: WaterResponseHint::AllowsWetMargins,
    ecology_density: EcologyDensityHint::Open,
};
