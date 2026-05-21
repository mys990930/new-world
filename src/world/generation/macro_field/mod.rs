use rayon::prelude::*;

mod context;
mod contour;
mod height;
mod influence;
mod ocean;
mod river;
#[cfg(test)]
pub(in crate::world::generation::macro_field) mod test_support;
mod types;

pub use context::MacroFieldRasterContext;
pub use contour::*;
pub use types::*;

use crate::world::generation::boundary::BoundaryCache;
use crate::world::generation::graph::{VoronoiGraphPatch, WorldPlanePoint};
use crate::world::generation::macro_map::{GraphMacroMap, MacroSurfaceKind};
use crate::world::generation::river_plan::RiverPlan;
use height::{
    combine_macro_height_with_estuary_profile, combine_macro_height_with_river_profile, envelope,
    is_lake_surface, ridge_envelope, roughened_distance,
};
use influence::{MacroFieldInfluenceSample, macro_field_stats, rasterize_influence_fields};
use ocean::prune_isolated_ocean_fragments;
use river::polyline_distance;

pub fn generate_macro_field_tile(
    patch: &VoronoiGraphPatch,
    macro_map: &GraphMacroMap,
    river_plan: &RiverPlan,
    boundary: &BoundaryCache,
    config: MacroFieldTileConfig,
) -> MacroFieldTile {
    validate_macro_field_config(config);
    assert_eq!(
        boundary.stats.missing_macro_edge_count, 0,
        "macro field requires complete canonical boundary coverage"
    );

    let context = MacroFieldRasterContext::new(patch, macro_map, river_plan, boundary);
    let influence_fields = rasterize_influence_fields(&context, config);
    let mut samples = (0..config.sample_count())
        .into_par_iter()
        .map(|index| {
            sample_macro_field_point_with_influence(
                &context,
                config,
                config.sample_position(index),
                influence_fields.sample(index, config),
            )
        })
        .collect::<Vec<_>>();
    prune_isolated_ocean_fragments(&mut samples, config);
    let stats = macro_field_stats(&samples, influence_fields.stats);

    MacroFieldTile {
        config,
        samples,
        stats,
    }
}
pub fn sample_macro_field_point(
    context: &MacroFieldRasterContext<'_>,
    config: MacroFieldTileConfig,
    position: WorldPlanePoint,
) -> MacroFieldSample {
    let owner_sample = context.owner_sample(position, config);
    let nearest_site = owner_sample.primary;
    let surface_kind = nearest_site.map(|site| site.surface_kind);
    let biome_cell = nearest_site.and_then(|site| context.biome_for_site(site.id));
    let macro_elevation = owner_sample.macro_elevation;
    let site_coastness = nearest_site.map(|site| site.coastness).unwrap_or_default();
    let ocean_mask = surface_kind
        .is_some_and(MacroSurfaceKind::is_ocean_owned)
        .then_some(1.0)
        .unwrap_or(0.0);
    let lake_mask = surface_kind
        .is_some_and(is_lake_surface)
        .then_some(1.0)
        .unwrap_or(0.0);
    let dry_basin_mask = surface_kind
        .is_some_and(|kind| kind == MacroSurfaceKind::DryBasin)
        .then_some(1.0)
        .unwrap_or(0.0);
    let coast_mask = context
        .nearest_coast_distance(position, config.coast_radius_blocks)
        .map(|distance| {
            envelope(
                roughened_distance(
                    distance,
                    position,
                    config.boundary_roughness_blocks,
                    0xC0A5_7001,
                ),
                config.coast_radius_blocks,
            )
        })
        .unwrap_or(site_coastness)
        .max(site_coastness)
        .clamp(0.0, 1.0);
    let ridge_influence = context
        .ridge_grid
        .candidate_indices(position, config.ridge_radius_blocks)
        .into_iter()
        .filter_map(|index| context.ridge_curves.get(index))
        .map(|curve| {
            ridge_envelope(
                polyline_distance(position, &curve.points),
                config.ridge_radius_blocks,
            )
        })
        .fold(0.0, f32::max);
    let river_influence = context.river_valley(position, config);
    let river_distance_blocks = river_influence.distance_blocks;
    let river_flow_hint = river_influence.flow_hint;
    let river_longitudinal_blocks = 0.0;
    let river_core_strength = river_influence.core_strength;
    let river_shoulder_strength = river_influence.shoulder_strength;
    let river_valley_strength = river_influence.valley_strength;
    let combined_macro_height = combine_macro_height_with_river_profile(
        macro_elevation,
        ocean_mask,
        coast_mask,
        lake_mask,
        dry_basin_mask,
        owner_sample.lake_lowering_factor,
        ridge_influence,
        river_shoulder_strength,
        river_core_strength,
        river_flow_hint,
        river_influence.bed_depth_hint,
        river_influence.bank_roughness_hint,
        river_influence.gravel_hint,
        river_influence.cutbank_hint,
        None,
        river_longitudinal_blocks,
        Some(position),
        config,
    );

    MacroFieldSample {
        position,
        nearest_site: nearest_site.map(|site| site.id),
        surface_kind,
        biome_context: biome_cell.map(|biome| biome.context),
        biome: biome_cell.map(|biome| biome.biome),
        macro_elevation,
        ocean_mask,
        coast_mask,
        lake_mask,
        dry_basin_mask,
        ridge_influence,
        river_core_strength,
        river_shoulder_strength,
        river_valley_strength,
        river_distance_blocks,
        river_flow_hint,
        river_longitudinal_blocks,
        river_bed_depth_hint: river_influence.bed_depth_hint,
        river_bank_roughness_hint: river_influence.bank_roughness_hint,
        river_gravel_hint: river_influence.gravel_hint,
        river_cutbank_hint: river_influence.cutbank_hint,
        combined_macro_height,
    }
}

fn sample_macro_field_point_with_influence(
    context: &MacroFieldRasterContext<'_>,
    config: MacroFieldTileConfig,
    position: WorldPlanePoint,
    influence: MacroFieldInfluenceSample,
) -> MacroFieldSample {
    let owner_sample = context.owner_sample(position, config);
    let nearest_site = owner_sample.primary;
    let surface_kind = nearest_site.map(|site| site.surface_kind);
    let biome_cell = nearest_site.and_then(|site| context.biome_for_site(site.id));
    let macro_elevation = owner_sample.macro_elevation;
    let site_coastness = nearest_site.map(|site| site.coastness).unwrap_or_default();
    let ocean_mask = surface_kind
        .is_some_and(MacroSurfaceKind::is_ocean_owned)
        .then_some(1.0)
        .unwrap_or(0.0);
    let lake_mask = surface_kind
        .is_some_and(is_lake_surface)
        .then_some(1.0)
        .unwrap_or(0.0);
    let dry_basin_mask = surface_kind
        .is_some_and(|kind| kind == MacroSurfaceKind::DryBasin)
        .then_some(1.0)
        .unwrap_or(0.0);
    let coast_mask = influence
        .coast_influence
        .max(site_coastness)
        .clamp(0.0, 1.0);
    let ridge_influence = influence.ridge_influence;
    let river_distance_blocks = influence.river_distance_blocks;
    let river_flow_hint = influence.river_flow_hint;
    let river_longitudinal_blocks = influence.river_longitudinal_blocks;
    let river_centerline_macro_elevation = influence
        .river_centerline_position
        .map(|centerline| context.owner_sample(centerline, config).macro_elevation);
    let river_core_strength = influence.river_core_strength;
    let river_shoulder_strength = influence.river_shoulder_strength;
    let river_valley_strength = influence.river_valley_strength;
    let combined_macro_height = combine_macro_height_with_estuary_profile(
        macro_elevation,
        ocean_mask,
        coast_mask,
        lake_mask,
        dry_basin_mask,
        owner_sample.lake_lowering_factor,
        ridge_influence,
        river_shoulder_strength,
        river_core_strength,
        river_flow_hint,
        influence.river_bed_depth_hint,
        influence.river_bank_roughness_hint,
        influence.river_gravel_hint,
        influence.river_cutbank_hint,
        river_centerline_macro_elevation,
        river_longitudinal_blocks,
        Some(position),
        influence.estuary_strength,
        influence.estuary_progress,
        influence.estuary_flow_hint,
        influence.estuary_bed_depth_hint,
        config,
    );

    MacroFieldSample {
        position,
        nearest_site: nearest_site.map(|site| site.id),
        surface_kind,
        biome_context: biome_cell.map(|biome| biome.context),
        biome: biome_cell.map(|biome| biome.biome),
        macro_elevation,
        ocean_mask,
        coast_mask,
        lake_mask,
        dry_basin_mask,
        ridge_influence,
        river_core_strength,
        river_shoulder_strength,
        river_valley_strength,
        river_distance_blocks,
        river_flow_hint,
        river_longitudinal_blocks,
        river_bed_depth_hint: influence.river_bed_depth_hint,
        river_bank_roughness_hint: influence.river_bank_roughness_hint,
        river_gravel_hint: influence.river_gravel_hint,
        river_cutbank_hint: influence.river_cutbank_hint,
        combined_macro_height,
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn macro_field_tile_generation_is_deterministic() {
        let inputs = test_inputs(42);
        let config = test_tile_config();

        let first = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
            config,
        );
        let second = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
            config,
        );

        assert_eq!(first, second);
    }

    #[test]
    fn macro_field_tile_dimensions_match_channel_lengths() {
        let inputs = test_inputs(42);
        let config = test_tile_config();

        let tile = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
            config,
        );

        assert_eq!(tile.samples.len(), config.sample_count());
        assert_eq!(tile.stats.sample_count, config.sample_count());
        assert!(tile.sample(0, 0).is_some());
        assert!(tile.sample(config.width, 0).is_none());
    }

    #[test]
    fn combined_macro_height_is_finite_and_sane() {
        let inputs = test_inputs(42);
        let tile = generate_macro_field_tile(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
            test_tile_config(),
        );

        assert!(tile.samples.iter().all(|sample| {
            sample.macro_elevation.is_finite()
                && sample.combined_macro_height.is_finite()
                && (-2.0..=2.0).contains(&sample.combined_macro_height)
        }));
        assert!(tile.stats.min_combined_macro_height <= tile.stats.max_combined_macro_height);
    }

    #[test]
    #[ignore = "diagnostic helper for combined_macro_height continuity tuning"]
    fn diagnose_combined_macro_height_neighbor_deltas() {
        for seed in [7, 42, 91] {
            let inputs = test_inputs(seed);
            let tile = generate_macro_field_tile(
                &inputs.patch,
                &inputs.macro_map,
                &inputs.river_plan,
                &inputs.boundary,
                MacroFieldTileConfig::new(-768.0, -768.0, 32, 32, 48.0),
            );
            let summary = combined_neighbor_delta_summary(&tile);
            eprintln!(
                "seed={seed} max={:.6} p95={:.6} pairs={} owner={} kind={} mask={} coast={} river={} dry={}",
                summary.max_delta,
                summary.p95_delta,
                summary.pair_count,
                summary.owner_switch_count,
                summary.surface_kind_switch_count,
                summary.hard_mask_switch_count,
                summary.coast_influence_count,
                summary.river_influence_count,
                summary.dry_basin_profile_count
            );
            for pair in top_combined_neighbor_delta_pairs(&tile).into_iter().take(3) {
                let left = tile.samples[pair.left_index];
                let right = tile.samples[pair.right_index];
                eprintln!(
                    "  top delta={:.6} left=({:.0},{:.0}) site={:?} kind={:?} macro={:.6} coast={:.3} river={:.3}/{:.3} combined={:.6} right=({:.0},{:.0}) site={:?} kind={:?} macro={:.6} coast={:.3} river={:.3}/{:.3} combined={:.6}",
                    pair.delta,
                    left.position.x,
                    left.position.z,
                    left.nearest_site,
                    left.surface_kind,
                    left.macro_elevation,
                    left.coast_mask,
                    left.river_valley_strength,
                    left.river_flow_hint,
                    left.combined_macro_height,
                    right.position.x,
                    right.position.z,
                    right.nearest_site,
                    right.surface_kind,
                    right.macro_elevation,
                    right.coast_mask,
                    right.river_valley_strength,
                    right.river_flow_hint,
                    right.combined_macro_height
                );
            }
            if let Some(pair) = top_combined_neighbor_delta_pairs(&tile).into_iter().next() {
                let dense = dense_delta_summary_around_pair(&inputs, &tile, pair, 1.0);
                eprintln!(
                    "  dense spacing={:.1} center=({:.0},{:.0}) max={:.6} p95={:.6} over_1block={}/{}",
                    dense.spacing,
                    dense.center.x,
                    dense.center.z,
                    dense.max_delta,
                    dense.p95_delta,
                    dense.over_one_block_count,
                    dense.pair_count
                );
            }
        }
    }
}
