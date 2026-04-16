use super::probe::{
    ChunkGenerationProbe, ChunkSurfaceLodGrid, ColumnGenerationProbe,
    probe_chunk as probe_chunk_v1, probe_column as probe_column_v1,
    sample_chunk_surface_lod as sample_chunk_surface_lod_v1,
};
use super::realize::generate_chunk as generate_chunk_v1;
use super::super::chunk::ChunkData;
use super::super::coord::ChunkCoord;
use super::super::meta::WorldMeta;
use super::super::registry::BlockRegistry;

pub const GENERATOR_LABEL: &str = "legacy_v1";

pub fn generate_chunk(coord: ChunkCoord, meta: &WorldMeta, registry: &BlockRegistry) -> ChunkData {
    generate_chunk_v1(coord, meta, registry)
}

pub fn probe_chunk(coord: ChunkCoord, meta: &WorldMeta) -> ChunkGenerationProbe {
    probe_chunk_v1(coord, meta)
}

pub fn probe_column(
    coord: ChunkCoord,
    local_x: u8,
    local_z: u8,
    meta: &WorldMeta,
) -> ColumnGenerationProbe {
    probe_column_v1(coord, local_x, local_z, meta)
}

pub fn sample_chunk_surface_lod(
    coord: ChunkCoord,
    step_blocks: u8,
    meta: &WorldMeta,
) -> ChunkSurfaceLodGrid {
    sample_chunk_surface_lod_v1(coord, step_blocks, meta)
}
