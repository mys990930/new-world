use super::{EcologyDensityHint, RegionArchetypeDef, SeasonalSurfaceProfile, WaterResponseHint};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::CoastalBeach,
    biome_family: BiomeFamily::Coast,
    terrain_form_family: TerrainFormFamily::Coast,
    summary: "Marine-facing terrain where shore expression and cliff transitions override inland defaults.",
    regional_traits: &[
        "Surface should read as coast-first even when upland transitions exist nearby.",
        "Rock exposure, spray, and sediment pockets should be resolved through coast policy.",
    ],
    ecology_notes: &[
        "Supports sparse salt-tolerant cover and exposed rock ecology later.",
        "Storm or cold season states may alter shoreline cover more than inland seasons do.",
    ],
    allowed_meso_keys: &["coastal_cliff_band", "upland_terrace", "shallow_basin"],
    seasonal_profile: SeasonalSurfaceProfile::CoastalTemperate,
    water_response: WaterResponseHint::CoastalSprayExposed,
    ecology_density: EcologyDensityHint::Sparse,
};
