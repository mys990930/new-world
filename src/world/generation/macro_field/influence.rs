use rayon::prelude::*;

use super::context::MacroFieldRasterContext;
use super::geometry::polyline_distance;
use super::height::{envelope, ridge_envelope, roughened_distance};
use super::types::{MacroFieldSample, MacroFieldTileConfig, MacroFieldTileStats};
use crate::world::generation::boundary::NoisyBoundaryCurve;
use crate::world::generation::graph::WorldPlanePoint;

pub(super) const RIDGE_FIELD_SOURCE_MIN_RIDGENESS: f32 = 0.44;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct MacroFieldInfluenceStats {
    pub(super) ridge_source_curve_count: usize,
    pub(super) river_source_curve_count: usize,
    pub(super) coast_source_curve_count: usize,
    pub(super) ridge_source_pixel_count: usize,
    pub(super) river_source_pixel_count: usize,
    pub(super) coast_source_pixel_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MacroFieldInfluenceSample {
    pub(super) ridge_influence: f32,
    pub(super) coast_influence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct MacroFieldInfluenceFields {
    pub(super) ridge_distance_blocks: Vec<f32>,
    pub(super) coast_distance_blocks: Vec<f32>,
    pub(super) stats: MacroFieldInfluenceStats,
}

impl MacroFieldInfluenceFields {
    pub(super) fn sample(
        &self,
        index: usize,
        config: MacroFieldTileConfig,
    ) -> MacroFieldInfluenceSample {
        let ridge_distance = self.ridge_distance_blocks[index];
        let coast_distance = self.coast_distance_blocks[index];

        MacroFieldInfluenceSample {
            ridge_influence: ridge_envelope(ridge_distance, config.ridge_radius_blocks),
            coast_influence: envelope(
                roughened_distance(
                    coast_distance,
                    config.sample_position(index),
                    config.boundary_roughness_blocks,
                    0xC0A5_7001,
                ),
                config.coast_radius_blocks,
            ),
        }
    }
}

pub(super) fn rasterize_influence_fields(
    context: &MacroFieldRasterContext<'_>,
    config: MacroFieldTileConfig,
) -> MacroFieldInfluenceFields {
    let ridge =
        rasterize_curve_distance_field(&context.ridge_curves, config, config.ridge_radius_blocks);
    let coast =
        rasterize_curve_distance_field(&context.coast_curves, config, config.coast_radius_blocks);
    let stats = MacroFieldInfluenceStats {
        ridge_source_curve_count: context.ridge_curves.len(),
        river_source_curve_count: 0,
        coast_source_curve_count: context.coast_curves.len(),
        ridge_source_pixel_count: ridge.source_pixel_count,
        river_source_pixel_count: 0,
        coast_source_pixel_count: coast.source_pixel_count,
    };

    MacroFieldInfluenceFields {
        ridge_distance_blocks: ridge.distance_blocks,
        coast_distance_blocks: coast.distance_blocks,
        stats,
    }
}

#[derive(Debug, Clone, PartialEq)]
struct DistanceField {
    distance_blocks: Vec<f32>,
    source_pixel_count: usize,
}

fn rasterize_curve_distance_field(
    curves: &[&NoisyBoundaryCurve],
    config: MacroFieldTileConfig,
    radius_blocks: f32,
) -> DistanceField {
    let distance_blocks = (0..config.sample_count())
        .into_par_iter()
        .map(|index| min_curve_distance(config.sample_position(index), curves))
        .collect::<Vec<_>>();
    let source_pixel_count = distance_blocks
        .iter()
        .filter(|distance| distance.is_finite() && **distance <= radius_blocks)
        .count();

    DistanceField {
        distance_blocks,
        source_pixel_count,
    }
}

fn min_curve_distance(position: WorldPlanePoint, curves: &[&NoisyBoundaryCurve]) -> f32 {
    curves
        .iter()
        .map(|curve| polyline_distance(position, &curve.points))
        .min_by(f32::total_cmp)
        .unwrap_or(f32::INFINITY)
}

pub(super) fn macro_field_stats(
    samples: &[MacroFieldSample],
    influence_stats: MacroFieldInfluenceStats,
) -> MacroFieldTileStats {
    if samples.is_empty() {
        return MacroFieldTileStats::default();
    }

    let mut stats = MacroFieldTileStats {
        sample_count: samples.len(),
        min_combined_macro_height: f32::INFINITY,
        max_combined_macro_height: f32::NEG_INFINITY,
        min_dry_basin_height: f32::INFINITY,
        max_dry_basin_height: f32::NEG_INFINITY,
        ridge_source_curve_count: influence_stats.ridge_source_curve_count,
        river_source_curve_count: influence_stats.river_source_curve_count,
        coast_source_curve_count: influence_stats.coast_source_curve_count,
        ridge_source_pixel_count: influence_stats.ridge_source_pixel_count,
        river_source_pixel_count: influence_stats.river_source_pixel_count,
        coast_source_pixel_count: influence_stats.coast_source_pixel_count,
        ..MacroFieldTileStats::default()
    };

    let mut ridge_sum = 0.0;
    let mut dry_height_sum = 0.0;
    for sample in samples {
        stats.min_combined_macro_height = stats
            .min_combined_macro_height
            .min(sample.combined_macro_height);
        stats.max_combined_macro_height = stats
            .max_combined_macro_height
            .max(sample.combined_macro_height);
        stats.max_ridge_influence = stats.max_ridge_influence.max(sample.ridge_influence);
        ridge_sum += sample.ridge_influence;
        if sample.ridge_influence > 0.0 {
            stats.ridge_active_sample_count += 1;
        }
        if sample.ocean_mask > 0.5 {
            stats.ocean_sample_count += 1;
        }
        if sample.lake_mask > 0.5 {
            stats.lake_sample_count += 1;
        }
        if sample.dry_basin_mask > 0.5 {
            stats.dry_basin_sample_count += 1;
            stats.min_dry_basin_height =
                stats.min_dry_basin_height.min(sample.combined_macro_height);
            stats.max_dry_basin_height =
                stats.max_dry_basin_height.max(sample.combined_macro_height);
            dry_height_sum += sample.combined_macro_height;
        }
    }
    stats.average_ridge_influence = ridge_sum / samples.len() as f32;
    if stats.dry_basin_sample_count > 0 {
        stats.average_dry_basin_height = dry_height_sum / stats.dry_basin_sample_count as f32;
    } else {
        stats.min_dry_basin_height = 0.0;
        stats.max_dry_basin_height = 0.0;
    }

    stats
}
