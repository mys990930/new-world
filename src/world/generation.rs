use super::chunk::{BlockId, ChunkData};
use super::coord::{CHUNK_EDGE_I32, ChunkCoord, LocalBlockCoord, chunk_local_to_world};
use super::meta::WorldMeta;

pub const FLAT_WORLD_SURFACE_Y: i32 = 0;

pub fn generate_chunk(coord: ChunkCoord, _meta: &WorldMeta) -> ChunkData {
    let mut chunk = ChunkData::new_empty(coord);

    for z in 0..CHUNK_EDGE_I32 as u8 {
        for x in 0..CHUNK_EDGE_I32 as u8 {
            let local = LocalBlockCoord::new(x, 0, z).expect("flat-plane local coord is valid");
            let world = chunk_local_to_world(coord, local);
            if world.1 == FLAT_WORLD_SURFACE_Y {
                let _ = chunk.set_block(local, BlockId::Grass);
            }
        }
    }

    chunk
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_plane_generation_populates_only_world_y_zero_layer() {
        let meta = WorldMeta::new(7);
        let surface_chunk = generate_chunk(ChunkCoord(0, 0, 0), &meta);
        let upper_chunk = generate_chunk(ChunkCoord(0, 1, 0), &meta);

        assert_eq!(
            surface_chunk
                .get_block(LocalBlockCoord::new(5, 0, 8).unwrap())
                .unwrap(),
            BlockId::Grass
        );
        assert_eq!(
            surface_chunk
                .get_block(LocalBlockCoord::new(5, 1, 8).unwrap())
                .unwrap(),
            BlockId::Air
        );
        assert_eq!(
            upper_chunk
                .get_block(LocalBlockCoord::new(5, 0, 8).unwrap())
                .unwrap(),
            BlockId::Air
        );
    }
}
