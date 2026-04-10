use super::context::{ColumnRealization, GenerationPalette};
use super::profile::resolve_profile;
use super::profiles::surface_y_for_profile;
use super::sampler::{generate_chunk_atlas_fields, sample_column_atlas};
use super::super::atlas::AtlasTuning;
use super::super::chunk::{BlockId, ChunkData};
use super::super::coord::{CHUNK_EDGE_I32, ChunkCoord, LocalBlockCoord, chunk_local_to_world};
use super::super::meta::WorldMeta;
use super::super::registry::BlockRegistry;
use super::WORLD_FLOOR_Y;

pub fn generate_chunk(coord: ChunkCoord, meta: &WorldMeta, registry: &BlockRegistry) -> ChunkData {
    let palette = GenerationPalette::from_registry(registry);
    let atlas_fields = generate_chunk_atlas_fields(coord, meta);
    let land_threshold = AtlasTuning::default().normalization.land_threshold;
    let mut chunk = ChunkData::new_empty(coord);

    for local_z in 0..CHUNK_EDGE_I32 as u8 {
        for local_x in 0..CHUNK_EDGE_I32 as u8 {
            let column_origin =
                chunk_local_to_world(coord, LocalBlockCoord::new(local_x, 0, local_z).unwrap());
            let world_x = column_origin.0;
            let world_z = column_origin.2;
            let atlas_sample = sample_column_atlas(&atlas_fields, world_x, world_z);
            let profile = resolve_profile(atlas_sample, land_threshold);
            let surface_y = surface_y_for_profile(meta.seed, world_x, world_z, atlas_sample, profile);
            let realization = ColumnRealization { surface_y, profile };

            for local_y in 0..CHUNK_EDGE_I32 as u8 {
                let local = LocalBlockCoord::new(local_x, local_y, local_z).unwrap();
                let world_y = chunk_local_to_world(coord, local).1;
                let block = block_for_world_y(world_y, realization, palette);
                if block != BlockId::AIR {
                    let _ = chunk.set_block(local, block);
                }
            }
        }
    }

    chunk
}

pub(super) fn block_for_world_y(
    world_y: i32,
    column: ColumnRealization,
    palette: GenerationPalette,
) -> BlockId {
    if world_y < WORLD_FLOOR_Y {
        return BlockId::AIR;
    }

    if world_y <= column.surface_y {
        return palette.terrain;
    }

    BlockId::AIR
}
