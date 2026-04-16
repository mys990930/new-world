#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeasonalPhase {
    Spring,
    Summer,
    Autumn,
    Winter,
    WetSeason,
    DrySeason,
    Thaw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeasonalBiomeStateId {
    TemperateGrowing,
    TemperateSnowy,
    TropicalWetSeason,
    TropicalDrySeason,
    ColdFrozenWetland,
    AlpineSnowpack,
    CoastalStormSeason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeasonalBiomeStateDef {
    pub id: SeasonalBiomeStateId,
    pub phase: SeasonalPhase,
    pub summary: &'static str,
    pub cover_override_hint: &'static str,
}

pub const SEASONAL_BIOME_STATES: &[SeasonalBiomeStateDef] = &[
    SeasonalBiomeStateDef {
        id: SeasonalBiomeStateId::TemperateGrowing,
        phase: SeasonalPhase::Summer,
        summary: "Temperate growing season with active green cover.",
        cover_override_hint: "favor greener grass and active wetland cover.",
    },
    SeasonalBiomeStateDef {
        id: SeasonalBiomeStateId::TemperateSnowy,
        phase: SeasonalPhase::Winter,
        summary: "Temperate winter state with snow-capable grass cover.",
        cover_override_hint: "swap exposed grass to snowy grass where snow persists.",
    },
    SeasonalBiomeStateDef {
        id: SeasonalBiomeStateId::TropicalWetSeason,
        phase: SeasonalPhase::WetSeason,
        summary: "Tropical wet-season expression with fuller channels and greener cover.",
        cover_override_hint: "increase wet cover and reduce exposed dry sediment.",
    },
    SeasonalBiomeStateDef {
        id: SeasonalBiomeStateId::TropicalDrySeason,
        phase: SeasonalPhase::DrySeason,
        summary: "Tropical dry-season expression with duller cover and more exposed margins.",
        cover_override_hint: "permit drier cover and more visible channel sediment.",
    },
    SeasonalBiomeStateDef {
        id: SeasonalBiomeStateId::ColdFrozenWetland,
        phase: SeasonalPhase::Winter,
        summary: "Cold wetland state with frozen mud and partial ice expression.",
        cover_override_hint: "freeze exposed wet cover before changing the archetype itself.",
    },
    SeasonalBiomeStateDef {
        id: SeasonalBiomeStateId::AlpineSnowpack,
        phase: SeasonalPhase::Winter,
        summary: "Persistent alpine snowpack state with sparse exposed rock.",
        cover_override_hint: "maintain strong snow cover with limited thaw exposure.",
    },
    SeasonalBiomeStateDef {
        id: SeasonalBiomeStateId::CoastalStormSeason,
        phase: SeasonalPhase::Autumn,
        summary: "Storm-facing coastal season with sparse cover and exposed shore materials.",
        cover_override_hint: "permit stronger rock and shoreline exposure near surf zones.",
    },
];

pub fn default_seasonal_biome_states() -> &'static [SeasonalBiomeStateDef] {
    SEASONAL_BIOME_STATES
}
