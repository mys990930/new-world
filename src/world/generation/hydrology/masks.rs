use super::context::RegionHydrologySignals;
use super::{HydrologyMode, sanitize_unit_interval};

pub(super) fn basin_signal(
    field: crate::world::atlas::AtlasCell,
    region: RegionHydrologySignals,
    positive_concavity: f32,
    slope_signal: f32,
) -> f32 {
    sanitize_unit_interval(
        field.lake_potential * 0.42
            + field.basinness * 0.26
            + region.lake_bias * 0.22
            + positive_concavity * 0.24
            - field.aridity * 0.14
            - slope_signal * 0.10,
        0.0,
    )
}

pub(super) fn saturation(
    field: crate::world::atlas::AtlasCell,
    region: RegionHydrologySignals,
    wet_margin_influence: f32,
    outer_influence: f32,
    floodplain_influence: f32,
    bank_influence: f32,
    positive_concavity: f32,
    basin_signal: f32,
) -> f32 {
    sanitize_unit_interval(
        wet_margin_influence * 0.22
            + outer_influence * 0.18
            + floodplain_influence * 0.22
            + bank_influence * 0.06
            + field.wetness * 0.24
            + field.wetland_factor * 0.12
            + region.wetland_bias * 0.24
            + positive_concavity * 0.15
            + basin_signal * 0.08
            - field.aridity * 0.20,
        0.0,
    )
}

pub(super) fn water_presence(
    region: RegionHydrologySignals,
    water_sheet_influence: f32,
    core_influence: f32,
    bank_influence: f32,
    lake_bias: f32,
    positive_concavity: f32,
) -> f32 {
    sanitize_unit_interval(
        water_sheet_influence * 1.20
            + core_influence * 0.14
            + bank_influence * 0.08
            + lake_bias * 0.36
            + region.wetland_bias * 0.05
            + positive_concavity * 0.04,
        0.0,
    )
}

pub(super) fn channel_has_visible_water(
    water_sheet_influence: f32,
    distance_blocks: f32,
    water_radius: f32,
    bank_influence: f32,
) -> bool {
    water_sheet_influence >= 0.14
        || (distance_blocks <= water_radius * 1.05 && bank_influence >= 0.25)
}

pub(super) fn classify_hydrology_mode(
    standing_water: Option<f32>,
    lake_bias: f32,
    core_influence: f32,
    basin_signal: f32,
    channel_has_visible_water: bool,
    saturation: f32,
    floodplain_influence: f32,
    outer_influence: f32,
) -> HydrologyMode {
    if standing_water.is_some() {
        if lake_bias >= core_influence.max(0.24) && basin_signal >= 0.50 {
            HydrologyMode::Lake
        } else if channel_has_visible_water {
            HydrologyMode::Channel
        } else {
            HydrologyMode::Floodplain
        }
    } else if saturation >= 0.56 && floodplain_influence >= 0.34 {
        HydrologyMode::Wetland
    } else if floodplain_influence >= 0.40 || outer_influence >= 0.52 {
        HydrologyMode::Floodplain
    } else {
        HydrologyMode::Dry
    }
}
