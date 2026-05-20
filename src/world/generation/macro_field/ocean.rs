use std::collections::VecDeque;

use super::height::combine_macro_height_with_river_longitudinal;
use super::types::{MacroFieldSample, MacroFieldTileConfig};
use crate::world::generation::biome::{GraphBiomeWaterRole, classify_graph_biome};
use crate::world::generation::macro_map::MacroSurfaceKind;

pub(super) const ISOLATED_OCEAN_FRAGMENT_MAX_BLOCK_AREA: f32 = 512.0;
pub(super) fn prune_isolated_ocean_fragments(
    samples: &mut [MacroFieldSample],
    config: MacroFieldTileConfig,
) {
    let width = config.width as usize;
    let height = config.height as usize;
    if width == 0 || height == 0 || samples.len() != width * height {
        return;
    }
    let mut visited = vec![false; samples.len()];
    let mut components = Vec::new();
    for start in 0..samples.len() {
        if visited[start] || samples[start].ocean_mask <= 0.5 {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        let mut indices = Vec::new();
        let mut touches_edge = false;
        let mut has_ocean_basin_source = false;
        visited[start] = true;
        while let Some(index) = queue.pop_front() {
            indices.push(index);
            let x = index % width;
            let z = index / width;
            touches_edge |= x == 0 || z == 0 || x + 1 == width || z + 1 == height;
            has_ocean_basin_source |= matches!(
                samples[index].surface_kind,
                Some(MacroSurfaceKind::OceanBasin)
            );
            for neighbor in ocean_component_neighbors(index, x, z, width, height) {
                if !visited[neighbor] && samples[neighbor].ocean_mask > 0.5 {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        components.push(OceanComponent {
            indices,
            touches_edge,
            has_ocean_basin_source,
        });
    }
    let max_fragment_samples =
        isolated_ocean_fragment_max_samples(config.sample_spacing_blocks).max(1);
    for component in &components {
        if component.touches_edge
            || (component.has_ocean_basin_source && component.indices.len() > max_fragment_samples)
        {
            continue;
        }
        for &sample_index in &component.indices {
            clear_isolated_ocean_sample(&mut samples[sample_index], config);
        }
    }
}
fn isolated_ocean_fragment_max_samples(sample_spacing_blocks: f32) -> usize {
    let sample_area = sample_spacing_blocks.max(1.0).powi(2);
    (ISOLATED_OCEAN_FRAGMENT_MAX_BLOCK_AREA / sample_area).ceil() as usize
}
#[derive(Debug)]
struct OceanComponent {
    indices: Vec<usize>,
    touches_edge: bool,
    has_ocean_basin_source: bool,
}
fn ocean_component_neighbors(
    index: usize,
    x: usize,
    z: usize,
    width: usize,
    height: usize,
) -> impl Iterator<Item = usize> {
    let mut neighbors = [None; 4];
    if x > 0 {
        neighbors[0] = Some(index - 1);
    }
    if x + 1 < width {
        neighbors[1] = Some(index + 1);
    }
    if z > 0 {
        neighbors[2] = Some(index - width);
    }
    if z + 1 < height {
        neighbors[3] = Some(index + width);
    }
    neighbors.into_iter().flatten()
}
fn clear_isolated_ocean_sample(sample: &mut MacroFieldSample, config: MacroFieldTileConfig) {
    if sample.lake_mask > 0.5 {
        return;
    }
    sample.ocean_mask = 0.0;
    sample.surface_kind = match sample.surface_kind {
        Some(MacroSurfaceKind::CoastOcean) => Some(MacroSurfaceKind::CoastLand),
        Some(MacroSurfaceKind::OceanBasin) => Some(MacroSurfaceKind::Continent),
        other => other,
    };
    if let Some(mut context) = sample.biome_context {
        let reclassify_context = matches!(
            context.water_role,
            GraphBiomeWaterRole::ShallowOcean | GraphBiomeWaterRole::DeepOcean
        );
        if reclassify_context {
            context.water_role = match sample.surface_kind {
                Some(MacroSurfaceKind::CoastLand | MacroSurfaceKind::CoastIsland) => {
                    GraphBiomeWaterRole::Coast
                }
                Some(
                    MacroSurfaceKind::Continent
                    | MacroSurfaceKind::Island
                    | MacroSurfaceKind::DryBasin,
                ) => GraphBiomeWaterRole::Land,
                Some(MacroSurfaceKind::WetlandCandidate) => GraphBiomeWaterRole::Wetland,
                Some(MacroSurfaceKind::LakeCandidate) => GraphBiomeWaterRole::Lake,
                _ => context.water_role,
            };
        }
        sample.biome_context = Some(context);
        if reclassify_context {
            sample.biome = Some(classify_graph_biome(context));
        }
    }
    sample.combined_macro_height = combine_macro_height_with_river_longitudinal(
        sample.macro_elevation,
        sample.ocean_mask,
        sample.coast_mask,
        sample.lake_mask,
        sample.dry_basin_mask,
        0.0,
        sample.ridge_influence,
        sample.river_shoulder_strength,
        sample.river_flow_hint,
        None,
        sample.river_longitudinal_blocks,
        config,
    );
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::world::generation::biome::{GraphBiomeContext, GraphBiomeKind, GraphBiomeWaterRole};
    use crate::world::generation::boundary::{
        BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
    };
    use crate::world::generation::graph::{
        VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId, WorldPlanePoint,
    };
    use crate::world::generation::macro_field::height::combine_macro_height;
    use crate::world::generation::macro_field::test_support::*;
    use crate::world::generation::macro_field::{
        MacroFieldRasterContext, MacroFieldTileConfig, generate_macro_field_tile,
        sample_macro_field_point,
    };
    use crate::world::generation::macro_map::{
        GraphMacroMap, MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
    };
    use crate::world::generation::river_plan::{
        DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS, RiverPlan,
    };

    #[test]
    fn isolated_ocean_fragment_is_pruned_without_touching_connected_ocean() {
        let mut tile = test_contour_tile(&[32.0; 25], 5, 5);
        let config = tile.config;
        for z in 0..5 {
            let index = z * 5;
            tile.samples[index].surface_kind = Some(MacroSurfaceKind::OceanBasin);
            tile.samples[index].ocean_mask = 1.0;
        }

        let fragment_index = 12;
        tile.samples[fragment_index].surface_kind = Some(MacroSurfaceKind::CoastOcean);
        tile.samples[fragment_index].macro_elevation = 0.04;
        tile.samples[fragment_index].ocean_mask = 1.0;
        tile.samples[fragment_index].combined_macro_height = combine_macro_height(
            tile.samples[fragment_index].macro_elevation,
            1.0,
            tile.samples[fragment_index].coast_mask,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            config,
        );

        prune_isolated_ocean_fragments(&mut tile.samples, config);

        assert_eq!(tile.samples[0].ocean_mask, 1.0);
        assert_eq!(tile.samples[20].ocean_mask, 1.0);
        assert_eq!(tile.samples[fragment_index].ocean_mask, 0.0);
        assert_eq!(
            tile.samples[fragment_index].surface_kind,
            Some(MacroSurfaceKind::CoastLand)
        );
        assert!(tile.samples[fragment_index].combined_macro_height > 0.0);
    }

    #[test]
    fn lone_ocean_fragment_is_not_kept_as_largest_component() {
        let mut tile = test_contour_tile(&[32.0; 25], 5, 5);
        let config = tile.config;
        let fragment_index = 12;
        tile.samples[fragment_index].surface_kind = Some(MacroSurfaceKind::CoastOcean);
        tile.samples[fragment_index].macro_elevation = 0.03;
        tile.samples[fragment_index].ocean_mask = 1.0;
        tile.samples[fragment_index].combined_macro_height = combine_macro_height(
            tile.samples[fragment_index].macro_elevation,
            1.0,
            tile.samples[fragment_index].coast_mask,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            config,
        );

        prune_isolated_ocean_fragments(&mut tile.samples, config);

        assert_eq!(tile.samples[fragment_index].ocean_mask, 0.0);
        assert_eq!(
            tile.samples[fragment_index].surface_kind,
            Some(MacroSurfaceKind::CoastLand)
        );
        assert!(
            tile.samples[fragment_index].combined_macro_height > 0.0,
            "isolated ocean owner sample should return to interpolated land/coast height"
        );
    }

    #[test]
    fn pruned_ocean_fragment_reclassifies_biome_context_instead_of_dropping_it() {
        let mut tile = test_contour_tile(&[32.0; 25], 5, 5);
        let config = tile.config;
        let fragment_index = 12;
        tile.samples[fragment_index].surface_kind = Some(MacroSurfaceKind::CoastOcean);
        tile.samples[fragment_index].macro_elevation = 0.03;
        tile.samples[fragment_index].ocean_mask = 1.0;
        tile.samples[fragment_index].biome_context = Some(GraphBiomeContext {
            temperature: 0.55,
            hydration: 0.50,
            elevation: 0.03,
            continentality: 0.02,
            coastness: 1.0,
            mountainness: 0.0,
            ruggedness: 0.0,
            water_role: GraphBiomeWaterRole::ShallowOcean,
        });
        tile.samples[fragment_index].biome = Some(GraphBiomeKind::ShallowOcean);

        prune_isolated_ocean_fragments(&mut tile.samples, config);

        let sample = tile.samples[fragment_index];
        assert_eq!(sample.ocean_mask, 0.0);
        assert_eq!(sample.surface_kind, Some(MacroSurfaceKind::CoastLand));
        assert_eq!(
            sample.biome_context.map(|context| context.water_role),
            Some(GraphBiomeWaterRole::Coast)
        );
        assert_eq!(sample.biome, Some(GraphBiomeKind::SandyCoast));
    }

    #[test]
    fn pruned_ocean_fragment_preserves_non_ocean_biome_contract() {
        let mut tile = test_contour_tile(&[32.0; 25], 5, 5);
        let config = tile.config;
        let fragment_index = 12;
        let land_context = GraphBiomeContext {
            temperature: 0.55,
            hydration: 0.50,
            elevation: 0.03,
            continentality: 0.08,
            coastness: 0.0,
            mountainness: 0.0,
            ruggedness: 0.0,
            water_role: GraphBiomeWaterRole::Land,
        };
        tile.samples[fragment_index].surface_kind = Some(MacroSurfaceKind::CoastOcean);
        tile.samples[fragment_index].macro_elevation = 0.03;
        tile.samples[fragment_index].ocean_mask = 1.0;
        tile.samples[fragment_index].biome_context = Some(land_context);
        tile.samples[fragment_index].biome = Some(GraphBiomeKind::TemperateMixedForest);

        prune_isolated_ocean_fragments(&mut tile.samples, config);

        let sample = tile.samples[fragment_index];
        assert_eq!(sample.ocean_mask, 0.0);
        assert_eq!(sample.surface_kind, Some(MacroSurfaceKind::CoastLand));
        assert_eq!(sample.biome_context, Some(land_context));
        assert_eq!(sample.biome, Some(GraphBiomeKind::TemperateMixedForest));
    }

    #[test]
    fn large_detached_coast_ocean_fragment_is_pruned() {
        let mut tile = test_contour_tile(&[32.0; 900], 30, 30);
        let config = tile.config;
        for z in 5..25 {
            for x in 5..25 {
                let index = z * 30 + x;
                tile.samples[index].surface_kind = Some(MacroSurfaceKind::CoastOcean);
                tile.samples[index].macro_elevation = 0.03;
                tile.samples[index].ocean_mask = 1.0;
                tile.samples[index].combined_macro_height = combine_macro_height(
                    tile.samples[index].macro_elevation,
                    1.0,
                    tile.samples[index].coast_mask,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    config,
                );
            }
        }

        prune_isolated_ocean_fragments(&mut tile.samples, config);

        assert!(
            tile.samples.iter().all(|sample| sample.ocean_mask <= 0.5),
            "detached CoastOcean-only blobs should not survive as standalone ocean"
        );
        assert!(
            tile.samples
                .iter()
                .all(|sample| sample.surface_kind != Some(MacroSurfaceKind::CoastOcean)),
            "cleared coast-ocean fragments should be reclassified to coast-land"
        );
    }
}
