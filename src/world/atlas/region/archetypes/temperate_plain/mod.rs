use super::{
    EcologyDensityHint, PrototypeArchetypeHint, RegionArchetypeDef, SeasonalSurfaceProfile,
    WaterResponseHint,
};
use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};

pub const PROTOTYPE_HINT: PrototypeArchetypeHint = PrototypeArchetypeHint {
    macro_height_bonus_delta: -2.0,
    wet_flatten_delta: 2.4,
    low_freq_amp_scale: 0.72,
    mid_freq_amp_scale: 0.58,
    terrace_amp_scale: 0.70,
    relief_base_scale: 0.82,
    relief_gain_scale: 0.80,
    corridor_depth_scale: 0.90,
    floodplain_width_scale: 1.12,
    ridge_lift_scale: 0.94,
    ridge_shoulder_lift_scale: 0.92,
    ridge_preservation_scale: 0.92,
};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::TemperatePlain,
    biome_family: BiomeFamily::TemperateGrassland,
    terrain_form_family: TerrainFormFamily::Plain,
    summary: "Baseline temperate lowland plain.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &["hill_cluster", "shallow_basin", "ravine"],
    seasonal_profile: SeasonalSurfaceProfile::TemperateSnowCapable,
    water_response: WaterResponseHint::AllowsWetMargins,
    ecology_density: EcologyDensityHint::Open,
};
