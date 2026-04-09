use super::chunk::ChunkData;
use super::coord::{CHUNK_EDGE_I32, ChunkCoord, LocalBlockCoord, chunk_local_to_world};
use super::meta::WorldMeta;
use super::registry::BlockRegistry;

pub const FLAT_WORLD_SURFACE_Y: i32 = 0;
pub const FLAT_PATCH_MIN_XZ: u8 = 1;
pub const FLAT_PATCH_MAX_XZ: u8 = 5;

pub fn generate_chunk(coord: ChunkCoord, _meta: &WorldMeta, registry: &BlockRegistry) -> ChunkData {
    let mut chunk = ChunkData::new_empty(coord);
    let Some(grass) = registry.block_id("grass") else {
        return chunk;
    };

    for z in 0..CHUNK_EDGE_I32 as u8 {
        for x in 0..CHUNK_EDGE_I32 as u8 {
            let local = LocalBlockCoord::new(x, 0, z).expect("flat-plane local coord is valid");
            let world = chunk_local_to_world(coord, local);
            if world.1 == FLAT_WORLD_SURFACE_Y && contains_flat_patch(local.x, local.z) {
                let _ = chunk.set_block(local, grass);
            }
        }
    }

    chunk
}

fn contains_flat_patch(x: u8, z: u8) -> bool {
    (FLAT_PATCH_MIN_XZ..=FLAT_PATCH_MAX_XZ).contains(&x)
        && (FLAT_PATCH_MIN_XZ..=FLAT_PATCH_MAX_XZ).contains(&z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_patch_generation_populates_only_requested_local_region() {
        let meta = WorldMeta::new(7);
        let registry = BlockRegistry::load_default().expect("default registry should load");
        let surface_chunk = generate_chunk(ChunkCoord(0, 0, 0), &meta, &registry);
        let upper_chunk = generate_chunk(ChunkCoord(0, 1, 0), &meta, &registry);

        assert_eq!(
            surface_chunk
                .get_block(LocalBlockCoord::new(3, 0, 3).unwrap())
                .unwrap(),
            super::super::BlockId::GRASS
        );
        assert_eq!(
            surface_chunk
                .get_block(LocalBlockCoord::new(3, 1, 3).unwrap())
                .unwrap(),
            super::super::BlockId::AIR
        );
        assert_eq!(
            upper_chunk
                .get_block(LocalBlockCoord::new(3, 0, 3).unwrap())
                .unwrap(),
            super::super::BlockId::AIR
        );
        assert_eq!(
            surface_chunk
                .get_block(LocalBlockCoord::new(0, 0, 0).unwrap())
                .unwrap(),
            super::super::BlockId::AIR
        );
        assert_eq!(
            surface_chunk
                .get_block(LocalBlockCoord::new(6, 0, 6).unwrap())
                .unwrap(),
            super::super::BlockId::AIR
        );
    }
}
