#![allow(dead_code)]

use new_world::world::generation::{
    apply_headwater_source_hydration_to_biomes, build_river_plan, generate_macro_map,
    generate_noisy_boundaries, generate_voronoi_graph_patch, graph_region_for_world_block,
    solve_hydrology, BoundaryCache, BoundaryConfig, GraphHydrologyGraph, GraphMacroMap,
    GraphRegionArea, GraphRegionCoord, HydrologyConfig, MacroMapConfig, RiverPlan,
    VoronoiGraphConfig, VoronoiGraphPatch, VoronoiGraphPatchRequest,
};
use new_world::world::WorldMeta;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewStageInput {
    pub center_world_x: i32,
    pub center_world_z: i32,
    pub region_size_blocks: i32,
    pub site_spacing_blocks: i32,
    pub land_bias: f32,
    pub graph_area: GraphRegionArea,
}

#[derive(Debug, Clone)]
pub struct CommonPreviewWorld {
    pub patch: VoronoiGraphPatch,
    pub macro_map: GraphMacroMap,
    pub hydrology: GraphHydrologyGraph,
    pub boundary: BoundaryCache,
    pub river_plan: RiverPlan,
}

pub fn build_common_preview_world(
    meta: &WorldMeta,
    input: PreviewStageInput,
) -> Result<CommonPreviewWorld, String> {
    let center_region = graph_region_for_world_block(
        input.center_world_x,
        input.center_world_z,
        input.region_size_blocks,
    );
    let padding_regions = required_padding_regions(center_region, input.graph_area)?;
    let graph_config = VoronoiGraphConfig {
        seed: meta.seed,
        generator_version: meta.generator_version,
        region_size_blocks: input.region_size_blocks,
        site_spacing_blocks: input.site_spacing_blocks,
        padding_regions,
    };
    let patch = generate_voronoi_graph_patch(VoronoiGraphPatchRequest::new(
        graph_config,
        input.center_world_x,
        input.center_world_z,
    ));
    if patch.sites.is_empty() {
        return Err("generated graph patch did not contain sites".to_string());
    }

    let mut macro_map = generate_macro_map(
        &patch,
        MacroMapConfig {
            land_bias: input.land_bias,
            ..MacroMapConfig::new(meta.seed, meta.generator_version)
        },
    );
    let hydrology = solve_hydrology(&patch, &macro_map, HydrologyConfig::default());
    apply_headwater_source_hydration_to_biomes(&patch, &mut macro_map, &hydrology);
    let boundary = generate_noisy_boundaries(
        &patch,
        &macro_map,
        BoundaryConfig::new(meta.seed, meta.generator_version),
    );
    let river_plan = build_river_plan(&patch, &macro_map, &hydrology, Default::default());

    Ok(CommonPreviewWorld {
        patch,
        macro_map,
        hydrology,
        boundary,
        river_plan,
    })
}

pub fn required_padding_regions(
    center: GraphRegionCoord,
    area: GraphRegionArea,
) -> Result<u32, String> {
    let dx = (center.x - area.min.x)
        .abs()
        .max((area.max.x - center.x).abs());
    let dz = (center.z - area.min.z)
        .abs()
        .max((area.max.z - center.z).abs());
    u32::try_from(dx.max(dz).saturating_add(1))
        .map_err(|_| "preview graph padding overflowed".to_string())
}
