use super::chunk::{BlockId, ChunkData, ChunkSnapshot};
use super::coord::{CHUNK_VOLUME, ChunkCoord};

const STORAGE_MAGIC: [u8; 4] = *b"NWCH";
const STORAGE_VERSION: u32 = 1;
const HEADER_LEN: usize = 4 + 4 + 4 + 4 + 4 + 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    TooShort,
    InvalidMagic,
    UnsupportedVersion { found: u32 },
    InvalidBlockCount { found: usize },
}

pub fn save_chunk(snapshot: &ChunkSnapshot) -> Result<Vec<u8>, StorageError> {
    let coord = snapshot.coord();
    let mut bytes = Vec::with_capacity(HEADER_LEN + snapshot.blocks().len() * 2);
    bytes.extend_from_slice(&STORAGE_MAGIC);
    bytes.extend_from_slice(&STORAGE_VERSION.to_le_bytes());
    bytes.extend_from_slice(&coord.0.to_le_bytes());
    bytes.extend_from_slice(&coord.1.to_le_bytes());
    bytes.extend_from_slice(&coord.2.to_le_bytes());
    bytes.extend_from_slice(&(snapshot.blocks().len() as u32).to_le_bytes());
    for block in snapshot.blocks() {
        bytes.extend_from_slice(&block.to_raw().to_le_bytes());
    }

    Ok(bytes)
}

pub fn load_chunk(bytes: &[u8]) -> Result<ChunkData, StorageError> {
    if bytes.len() < HEADER_LEN {
        return Err(StorageError::TooShort);
    }

    if bytes[0..4] != STORAGE_MAGIC {
        return Err(StorageError::InvalidMagic);
    }

    let version = u32::from_le_bytes(bytes[4..8].try_into().expect("header slice must fit"));
    if version != STORAGE_VERSION {
        return Err(StorageError::UnsupportedVersion { found: version });
    }

    let coord = ChunkCoord(
        i32::from_le_bytes(bytes[8..12].try_into().expect("x slice must fit")),
        i32::from_le_bytes(bytes[12..16].try_into().expect("y slice must fit")),
        i32::from_le_bytes(bytes[16..20].try_into().expect("z slice must fit")),
    );
    let block_count =
        u32::from_le_bytes(bytes[20..24].try_into().expect("count slice must fit")) as usize;
    if block_count != CHUNK_VOLUME {
        return Err(StorageError::InvalidBlockCount { found: block_count });
    }

    let expected_len = HEADER_LEN + block_count * 2;
    if bytes.len() != expected_len {
        return Err(StorageError::TooShort);
    }

    let mut blocks = Vec::with_capacity(block_count);
    for chunk in bytes[HEADER_LEN..].chunks_exact(2) {
        let raw = u16::from_le_bytes(chunk.try_into().expect("block slice must fit"));
        blocks.push(BlockId::from_raw(raw));
    }

    Ok(ChunkData::from_blocks(coord, blocks))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::LocalBlockCoord;

    #[test]
    fn storage_round_trip_preserves_chunk_contents() {
        let mut chunk = ChunkData::new_empty(ChunkCoord(-2, 0, 5));
        chunk
            .set_block(LocalBlockCoord::new(0, 0, 0).unwrap(), BlockId::GRASS)
            .unwrap();
        chunk
            .set_block(LocalBlockCoord::new(3, 1, 4).unwrap(), BlockId::STONE)
            .unwrap();

        let bytes = save_chunk(&chunk.snapshot()).expect("save should succeed");
        let restored = load_chunk(&bytes).expect("load should succeed");

        assert_eq!(restored.coord(), ChunkCoord(-2, 0, 5));
        assert_eq!(
            restored.get_block(LocalBlockCoord::new(0, 0, 0).unwrap()),
            Some(BlockId::GRASS)
        );
        assert_eq!(
            restored.get_block(LocalBlockCoord::new(3, 1, 4).unwrap()),
            Some(BlockId::STONE)
        );
    }
}
