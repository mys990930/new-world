use super::context::ColumnAtlasSample;
use super::super::atlas::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasArea, AtlasCell, AtlasCoord, AtlasFieldMap, generate_atlas_fields,
};
use super::super::coord::{CHUNK_EDGE_I32, ChunkCoord};
use super::super::meta::WorldMeta;

const ATLAS_CELL_SPAN_BLOCKS_I32: i32 = ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32;

pub(super) fn generate_chunk_atlas_fields(coord: ChunkCoord, meta: &WorldMeta) -> AtlasFieldMap {
    let origin = AtlasCoord::new(
        coord.0.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
        coord.2.div_euclid(ATLAS_CELL_SIZE_IN_CHUNKS as i32),
    );
    let area = AtlasArea::new(origin, 2, 2).expect("generation atlas area is valid");
    generate_atlas_fields(meta, area)
}

pub(super) fn sample_column_atlas(
    fields: &AtlasFieldMap,
    world_x: i32,
    world_z: i32,
) -> ColumnAtlasSample {
    let atlas_x = world_x.div_euclid(ATLAS_CELL_SPAN_BLOCKS_I32);
    let atlas_z = world_z.div_euclid(ATLAS_CELL_SPAN_BLOCKS_I32);
    let frac_x = (world_x.rem_euclid(ATLAS_CELL_SPAN_BLOCKS_I32) as f32 + 0.5)
        / ATLAS_CELL_SPAN_BLOCKS_I32 as f32;
    let frac_z = (world_z.rem_euclid(ATLAS_CELL_SPAN_BLOCKS_I32) as f32 + 0.5)
        / ATLAS_CELL_SPAN_BLOCKS_I32 as f32;

    let c00 = fields
        .get(AtlasCoord::new(atlas_x, atlas_z))
        .expect("generation atlas sample must exist");
    let c10 = fields
        .get(AtlasCoord::new(atlas_x + 1, atlas_z))
        .expect("generation atlas east sample must exist");
    let c01 = fields
        .get(AtlasCoord::new(atlas_x, atlas_z + 1))
        .expect("generation atlas south sample must exist");
    let c11 = fields
        .get(AtlasCoord::new(atlas_x + 1, atlas_z + 1))
        .expect("generation atlas southeast sample must exist");

    ColumnAtlasSample {
        landness: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.landness),
        ocean_distance: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| {
            cell.ocean_distance
        }),
        coast_factor: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.coast_factor),
        continent_core_factor: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| {
            cell.continent_core_factor
        }),
        macro_elevation: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| {
            cell.macro_elevation
        }),
        ridge_factor: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.ridge_factor),
        mountain_mass: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| {
            cell.mountain_mass
        }),
        ruggedness: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| cell.ruggedness),
        riverine_factor: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| {
            cell.riverine_factor
        }),
        alpine_factor: bilerp_cell(c00, c10, c01, c11, frac_x, frac_z, |cell| {
            cell.alpine_factor
        }),
    }
}

fn bilerp_cell(
    c00: &AtlasCell,
    c10: &AtlasCell,
    c01: &AtlasCell,
    c11: &AtlasCell,
    tx: f32,
    tz: f32,
    sample: impl Fn(&AtlasCell) -> f32,
) -> f32 {
    bilerp(sample(c00), sample(c10), sample(c01), sample(c11), tx, tz)
}

fn bilerp(a00: f32, a10: f32, a01: f32, a11: f32, tx: f32, tz: f32) -> f32 {
    let north = lerp_f32(a00, a10, tx);
    let south = lerp_f32(a01, a11, tx);
    lerp_f32(north, south, tz)
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}
