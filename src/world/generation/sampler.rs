use super::super::atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCoord, AtlasFieldMap, AtlasStructureMap,
    MesoGuideMap, RegionClassMap, generate_atlas_fields, generate_atlas_structure,
    generate_meso_guides, resolve_region_classes,
};
use super::super::coord::ChunkCoord;
use super::super::meta::WorldMeta;

const GENERATION_ATLAS_PADDING_CELLS: i32 = 4;

fn chunk_generation_atlas_area(coord: ChunkCoord) -> AtlasArea {
    let base = AtlasCoord::new(
        coord.0.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
        coord.2.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
    );
    let origin = AtlasCoord::new(
        base.x - GENERATION_ATLAS_PADDING_CELLS,
        base.z - GENERATION_ATLAS_PADDING_CELLS,
    );
    let span = (GENERATION_ATLAS_PADDING_CELLS * 2 + 2) as u32;
    AtlasArea::new(origin, span, span).expect("generation atlas area is valid")
}

pub(super) fn generate_chunk_atlas_fields(coord: ChunkCoord, meta: &WorldMeta) -> AtlasFieldMap {
    generate_atlas_fields(meta, chunk_generation_atlas_area(coord))
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
