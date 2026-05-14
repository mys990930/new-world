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

pub(super) fn chunk_generation_atlas_area(coord: ChunkCoord) -> AtlasArea {
    let base = AtlasCoord::new(
        coord.0.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
        coord.2.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
    );
    let context_origin = AtlasCoord::new(
        base.x.div_euclid(GENERATION_ATLAS_CONTEXT_EDGE_CELLS)
            * GENERATION_ATLAS_CONTEXT_EDGE_CELLS,
        base.z.div_euclid(GENERATION_ATLAS_CONTEXT_EDGE_CELLS)
            * GENERATION_ATLAS_CONTEXT_EDGE_CELLS,
    );
    let origin = AtlasCoord::new(
        context_origin.x - GENERATION_ATLAS_PADDING_CELLS,
        context_origin.z - GENERATION_ATLAS_PADDING_CELLS,
    );
    let span = (GENERATION_ATLAS_PADDING_CELLS * 2 + GENERATION_ATLAS_CONTEXT_EDGE_CELLS) as u32;
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
