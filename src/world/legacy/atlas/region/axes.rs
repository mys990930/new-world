#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawClassificationDimension {
    pub key: &'static str,
    pub summary: &'static str,
}

pub const RAW_CLASSIFICATION_DIMENSIONS: &[RawClassificationDimension] = &[
    RawClassificationDimension {
        key: "temperature_mean",
        summary: "Annual mean thermal level before seasonal state is applied.",
    },
    RawClassificationDimension {
        key: "moisture_balance",
        summary: "Long-term wet versus dry tendency after aridity pressure.",
    },
    RawClassificationDimension {
        key: "macro_elevation",
        summary: "Atlas-scale elevation used before local heightfield solving.",
    },
    RawClassificationDimension {
        key: "relief_energy",
        summary: "Macro ruggedness or terrain energy before local slope emerges.",
    },
    RawClassificationDimension {
        key: "drainage_potential",
        summary: "Whether water tends to evacuate, collect, or form corridors here.",
    },
    RawClassificationDimension {
        key: "coast_exposure",
        summary: "How marine-facing versus inland the annual setting is.",
    },
    RawClassificationDimension {
        key: "thermal_seasonality",
        summary: "How strong the warm-cold annual swing is.",
    },
    RawClassificationDimension {
        key: "precipitation_seasonality",
        summary: "How evenly rainfall spreads across the year.",
    },
    RawClassificationDimension {
        key: "snow_persistence",
        summary: "How likely snow cover is to remain across seasons.",
    },
    RawClassificationDimension {
        key: "freeze_thaw_tendency",
        summary: "How often the surface oscillates around freezing conditions.",
    },
];

pub const RESOLVED_CLASSIFICATION_DIMENSIONS: &[RawClassificationDimension] = &[
    RawClassificationDimension {
        key: "temperature_band",
        summary: "Resolved thermal band for regional classification.",
    },
    RawClassificationDimension {
        key: "moisture_band",
        summary: "Resolved wetness band for regional classification.",
    },
    RawClassificationDimension {
        key: "elevation_band",
        summary: "Resolved macro altitude band for terrain identity.",
    },
    RawClassificationDimension {
        key: "relief_class",
        summary: "Resolved macro roughness class used for terrain form choice.",
    },
    RawClassificationDimension {
        key: "hydrology_context",
        summary: "Resolved water-context class such as dryland, basin, or corridor.",
    },
    RawClassificationDimension {
        key: "coastal_context",
        summary: "Resolved marine exposure class such as inland, beach, or rocky coast.",
    },
    RawClassificationDimension {
        key: "climate_regime",
        summary: "Derived long-pattern climate class such as oceanic, continental, or monsoonal.",
    },
];
