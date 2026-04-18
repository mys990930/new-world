use std::collections::HashMap;

use super::super::atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, ATLAS_STRUCTURE_REGION_EDGE_CELLS, AtlasArea, AtlasCoord,
    AtlasFieldMap, AtlasStructureMap, AtlasStructureRegion, AtlasStructureRegionCoord,
    MesoGuideMap, RegionClassMap, atlas_structure_region_coord_for_atlas, generate_atlas_fields,
    generate_atlas_structure, generate_meso_guides, resolve_region_classes,
};
use super::super::coord::ChunkCoord;
use super::super::meta::WorldMeta;

const GENERATION_ATLAS_PADDING_CELLS: i32 = 4;
const GENERATION_ATLAS_CONTEXT_EDGE_CELLS: i32 = ATLAS_STRUCTURE_REGION_EDGE_CELLS as i32;
const GENERATION_CANONICAL_FIELD_PADDING_CELLS: i32 = 16;

fn chunk_generation_atlas_area(coord: ChunkCoord) -> AtlasArea {
    let base = AtlasCoord::new(
        coord.0.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
        coord.2.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
    );
    let context_origin = AtlasCoord::new(
        base.x.div_euclid(GENERATION_ATLAS_CONTEXT_EDGE_CELLS) * GENERATION_ATLAS_CONTEXT_EDGE_CELLS,
        base.z.div_euclid(GENERATION_ATLAS_CONTEXT_EDGE_CELLS) * GENERATION_ATLAS_CONTEXT_EDGE_CELLS,
    );
    let origin = AtlasCoord::new(
        context_origin.x - GENERATION_ATLAS_PADDING_CELLS,
        context_origin.z - GENERATION_ATLAS_PADDING_CELLS,
    );
    let span =
        (GENERATION_ATLAS_PADDING_CELLS * 2 + GENERATION_ATLAS_CONTEXT_EDGE_CELLS) as u32;
    AtlasArea::new(origin, span, span).expect("generation atlas area is valid")
}

pub(super) fn generate_chunk_atlas_fields(coord: ChunkCoord, meta: &WorldMeta) -> AtlasFieldMap {
    assemble_chunk_atlas_fields(meta, chunk_generation_atlas_area(coord))
}

pub(super) fn generate_chunk_atlas_structure(
    coord: ChunkCoord,
    meta: &WorldMeta,
) -> AtlasStructureMap {
    generate_atlas_structure(meta, chunk_generation_atlas_area(coord))
}

pub(super) fn generate_chunk_meso_guides(
    coord: ChunkCoord,
    meta: &WorldMeta,
    atlas_fields: &AtlasFieldMap,
    atlas_structure: &AtlasStructureMap,
) -> MesoGuideMap {
    let _ = coord;
    generate_meso_guides(meta, atlas_fields.area(), atlas_fields, atlas_structure)
}

pub(super) fn generate_chunk_region_classes(
    coord: ChunkCoord,
    meta: &WorldMeta,
    atlas_fields: &AtlasFieldMap,
    atlas_structure: &AtlasStructureMap,
) -> RegionClassMap {
    let _ = coord;
    resolve_region_classes(meta, atlas_fields.area(), atlas_fields, atlas_structure)
}

fn assemble_chunk_atlas_fields(meta: &WorldMeta, area: AtlasArea) -> AtlasFieldMap {
    let mut region_cache = HashMap::<AtlasStructureRegionCoord, AtlasFieldMap>::new();
    let mut values = Vec::with_capacity(area.len());

    for coord in area.coords() {
        let region_coord = atlas_structure_region_coord_for_atlas(coord);
        let region_fields = region_cache.entry(region_coord).or_insert_with(|| {
            generate_atlas_fields(meta, canonical_field_sample_area(region_coord))
        });
        let cell = *region_fields
            .get(coord)
            .expect("canonical atlas field region must cover its core cell");
        values.push(cell);
    }

    AtlasFieldMap::from_cells(area, values)
}

fn canonical_field_sample_area(region_coord: AtlasStructureRegionCoord) -> AtlasArea {
    let region = AtlasStructureRegion::new(region_coord);
    expand_area(region.core_area(), GENERATION_CANONICAL_FIELD_PADDING_CELLS)
}

fn expand_area(area: AtlasArea, padding_cells: i32) -> AtlasArea {
    AtlasArea::new(
        AtlasCoord::new(
            area.origin().x - padding_cells,
            area.origin().z - padding_cells,
        ),
        (area.width() as i32 + padding_cells * 2) as u32,
        (area.height() as i32 + padding_cells * 2) as u32,
    )
    .expect("expanded atlas area must stay valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::atlas::{MesoGuideSample, sample_meso_guides};
    use crate::world::coord::CHUNK_EDGE_I32;

    fn overlap_area(a: AtlasArea, b: AtlasArea) -> AtlasArea {
        let min_x = a.origin().x.max(b.origin().x);
        let min_z = a.origin().z.max(b.origin().z);
        let max_x = (a.origin().x + a.width() as i32 - 1).min(b.origin().x + b.width() as i32 - 1);
        let max_z = (a.origin().z + a.height() as i32 - 1).min(b.origin().z + b.height() as i32 - 1);
        AtlasArea::new(
            AtlasCoord::new(min_x, min_z),
            (max_x - min_x + 1) as u32,
            (max_z - min_z + 1) as u32,
        )
        .expect("neighboring chunk atlas areas should overlap")
    }

    #[test]
    fn canonical_field_assembly_keeps_overlap_identical_across_context_boundaries() {
        let meta = WorldMeta::new(42);
        let left = generate_chunk_atlas_fields(ChunkCoord(127, 0, 0), &meta);
        let right = generate_chunk_atlas_fields(ChunkCoord(128, 0, 0), &meta);

        assert_ne!(left.area(), right.area());

        for coord in overlap_area(left.area(), right.area()).coords() {
            assert_eq!(left.get(coord), right.get(coord), "field overlap drifted at {coord:?}");
        }
    }

    #[test]
    fn region_classification_overlap_stays_identical_across_context_boundaries() {
        let meta = WorldMeta::new(42);
        let left_chunk = ChunkCoord(127, 0, 0);
        let right_chunk = ChunkCoord(128, 0, 0);
        let left_fields = generate_chunk_atlas_fields(left_chunk, &meta);
        let right_fields = generate_chunk_atlas_fields(right_chunk, &meta);
        let left_structure = generate_chunk_atlas_structure(left_chunk, &meta);
        let right_structure = generate_chunk_atlas_structure(right_chunk, &meta);
        let left_regions =
            generate_chunk_region_classes(left_chunk, &meta, &left_fields, &left_structure);
        let right_regions =
            generate_chunk_region_classes(right_chunk, &meta, &right_fields, &right_structure);

        for coord in overlap_area(left_regions.area(), right_regions.area()).coords() {
            assert_eq!(
                left_regions.get(coord),
                right_regions.get(coord),
                "region overlap drifted at {coord:?}"
            );
        }
    }

    #[test]
    fn meso_sampling_stays_stable_across_context_boundaries() {
        let meta = WorldMeta::new(42);
        let left_chunk = ChunkCoord(127, 0, 0);
        let right_chunk = ChunkCoord(128, 0, 0);
        let left_fields = generate_chunk_atlas_fields(left_chunk, &meta);
        let right_fields = generate_chunk_atlas_fields(right_chunk, &meta);
        let left_structure = generate_chunk_atlas_structure(left_chunk, &meta);
        let right_structure = generate_chunk_atlas_structure(right_chunk, &meta);
        let left_guides =
            generate_chunk_meso_guides(left_chunk, &meta, &left_fields, &left_structure);
        let right_guides =
            generate_chunk_meso_guides(right_chunk, &meta, &right_fields, &right_structure);
        let min_world_x = left_chunk.0 * CHUNK_EDGE_I32 - 16;
        let max_world_x = right_chunk.0 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32 + 15;
        let min_world_z = -16;
        let max_world_z = 31;

        for world_z in (min_world_z..=max_world_z).step_by(4) {
            for world_x in (min_world_x..=max_world_x).step_by(4) {
                let left = sample_meso_guides(&left_guides, world_x, world_z);
                let right = sample_meso_guides(&right_guides, world_x, world_z);
                assert_meso_samples_match(left, right, world_x, world_z);
            }
        }
    }

    fn assert_meso_samples_match(
        left: MesoGuideSample,
        right: MesoGuideSample,
        world_x: i32,
        world_z: i32,
    ) {
        let epsilon = 0.0001_f32;
        assert!(
            (left.hilliness - right.hilliness).abs() <= epsilon
                && (left.hill_height - right.hill_height).abs() <= epsilon
                && (left.basin_weight - right.basin_weight).abs() <= epsilon
                && (left.basin_depth - right.basin_depth).abs() <= epsilon
                && (left.escarpment_weight - right.escarpment_weight).abs() <= epsilon
                && (left.escarpment_height - right.escarpment_height).abs() <= epsilon
                && (left.escarpment_signed_distance_cells - right.escarpment_signed_distance_cells).abs()
                    <= epsilon
                && (left.terrace_weight - right.terrace_weight).abs() <= epsilon
                && (left.terrace_step_height - right.terrace_step_height).abs() <= epsilon
                && (left.terrace_spacing_cells - right.terrace_spacing_cells).abs() <= epsilon
                && (left.terrace_signed_distance_cells - right.terrace_signed_distance_cells).abs()
                    <= epsilon,
            "meso guide sample drifted at world ({world_x}, {world_z})\nleft={left:?}\nright={right:?}"
        );
    }
}
