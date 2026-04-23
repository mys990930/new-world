#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverPhase {
    Dormant,
    Growing,
    Dry,
    Saturated,
    Frozen,
    SnowCovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverOverrideRule {
    pub key: &'static str,
    pub summary: &'static str,
}

pub const COVER_OVERRIDE_RULES: &[CoverOverrideRule] = &[
    CoverOverrideRule {
        key: "snowy_grass",
        summary: "Swaps exposed temperate grass cover to snowy grass under winter snow state.",
    },
    CoverOverrideRule {
        key: "frozen_mud",
        summary: "Freezes wet exposed mud margins under sustained cold seasonal states.",
    },
    CoverOverrideRule {
        key: "wet_season_greening",
        summary: "Strengthens green cover during wet-season tropical states.",
    },
];

pub fn default_cover_override_rules() -> &'static [CoverOverrideRule] {
    COVER_OVERRIDE_RULES
}

pub fn cover_override_rule(key: &str) -> Option<&'static CoverOverrideRule> {
    COVER_OVERRIDE_RULES.iter().find(|rule| rule.key == key)
}
