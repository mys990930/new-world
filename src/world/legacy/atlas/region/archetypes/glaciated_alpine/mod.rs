use super::super::{BiomeFamily, RegionArchetype, TerrainFormFamily};
use super::{
    EcologyDensityHint, PrototypeArchetypeHint, RegionArchetypeDef, SeasonalSurfaceProfile,
    WaterResponseHint,
};

pub const PROTOTYPE_HINT: PrototypeArchetypeHint = PrototypeArchetypeHint {
    macro_height_bonus_delta: 6.0,
    wet_flatten_delta: -0.4,
    low_freq_amp_scale: 1.08,
    mid_freq_amp_scale: 1.18,
    terrace_amp_scale: 1.10,
    relief_base_scale: 1.06,
    relief_gain_scale: 1.18,
    corridor_depth_scale: 1.06,
    floodplain_width_scale: 0.94,
    ridge_lift_scale: 1.18,
    ridge_shoulder_lift_scale: 1.12,
    ridge_preservation_scale: 1.10,
};

pub const DEF: RegionArchetypeDef = RegionArchetypeDef {
    id: RegionArchetype::GlaciatedAlpine,
    biome_family: BiomeFamily::PolarIce,
    terrain_form_family: TerrainFormFamily::Icefield,
    summary: "Persistent ice and snow upland with glacial control.",
    regional_traits: &[
        "Planning stub: detailed prototype solving and hydrology coupling still need a dedicated pass.",
        "This archetype should later receive explicit seasonal, material, and ecology policy locks.",
    ],
    ecology_notes: &[
        "Biome family and terrain-form family are locked for this candidate.",
        "Detailed vegetation density and gameplay-facing ecology rules remain to be specified.",
    ],
    allowed_meso_keys: &[],
    seasonal_profile: SeasonalSurfaceProfile::AlpineSnowPersistent,
    water_response: WaterResponseHint::GlacialMeltDriven,
    ecology_density: EcologyDensityHint::Barren,
};
