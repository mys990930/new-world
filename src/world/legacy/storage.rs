use super::chunk::{BlockId, ChunkData, ChunkSnapshot, ChunkStorageEncoding};
use super::coord::{CHUNK_VOLUME, ChunkCoord};

const STORAGE_MAGIC: [u8; 4] = *b"NWCH";
const STORAGE_VERSION: u32 = 1;
const HEADER_LEN: usize = 4 + 4 + 4 + 4 + 4 + 1 + 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    TooShort,
    InvalidMagic,
    UnsupportedVersion { found: u32 },
    UnsupportedEncoding { found: u8 },
    InvalidBlockCount { found: usize },
}

pub fn save_chunk(snapshot: &ChunkSnapshot) -> Result<Vec<u8>, StorageError> {
    let coord = snapshot.coord();
    let mut bytes = match snapshot.storage_encoding() {
        ChunkStorageEncoding::Uniform => Vec::with_capacity(HEADER_LEN + 2),
        ChunkStorageEncoding::Dense => Vec::with_capacity(HEADER_LEN + snapshot.block_count() * 2),
    };
    bytes.extend_from_slice(&STORAGE_MAGIC);
    bytes.extend_from_slice(&STORAGE_VERSION.to_le_bytes());
    bytes.extend_from_slice(&coord.0.to_le_bytes());
    bytes.extend_from_slice(&coord.1.to_le_bytes());
    bytes.extend_from_slice(&coord.2.to_le_bytes());
    bytes.push(snapshot.storage_encoding() as u8);
    bytes.extend_from_slice(&(snapshot.block_count() as u32).to_le_bytes());
    match snapshot.storage_encoding() {
        ChunkStorageEncoding::Uniform => {
            let block = snapshot
                .uniform_block()
                .expect("uniform encoding must expose a uniform block");
            bytes.extend_from_slice(&block.to_raw().to_le_bytes());
        }
        ChunkStorageEncoding::Dense => {
            for block in snapshot.iter_blocks() {
                bytes.extend_from_slice(&block.to_raw().to_le_bytes());
            }
        }
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

    let coord = decode_coord(bytes);
    let encoding = bytes[20];
    let block_count =
        u32::from_le_bytes(bytes[21..25].try_into().expect("count slice must fit")) as usize;
    if block_count != CHUNK_VOLUME {
        return Err(StorageError::InvalidBlockCount { found: block_count });
    }

    match encoding {
        value if value == ChunkStorageEncoding::Uniform as u8 => {
            let expected_len = HEADER_LEN + 2;
            if bytes.len() != expected_len {
                return Err(StorageError::TooShort);
            }

            let raw = u16::from_le_bytes(
                bytes[HEADER_LEN..HEADER_LEN + 2]
                    .try_into()
                    .expect("uniform block slice must fit"),
            );
            Ok(ChunkData::new_filled(coord, BlockId::from_raw(raw)))
        }
        value if value == ChunkStorageEncoding::Dense as u8 => {
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
        found => Err(StorageError::UnsupportedEncoding { found }),
    }
}

fn decode_coord(bytes: &[u8]) -> ChunkCoord {
    ChunkCoord(
        i32::from_le_bytes(bytes[8..12].try_into().expect("x slice must fit")),
        i32::from_le_bytes(bytes[12..16].try_into().expect("y slice must fit")),
        i32::from_le_bytes(bytes[16..20].try_into().expect("z slice must fit")),
    )
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

    #[test]
    fn uniform_air_chunk_uses_compact_v1_encoding() {
        let chunk = ChunkData::new_empty(ChunkCoord(1, 2, 3));

        let bytes = save_chunk(&chunk.snapshot()).expect("save should succeed");
        let restored = load_chunk(&bytes).expect("load should succeed");

        assert_eq!(bytes.len(), HEADER_LEN + 2);
        assert_eq!(restored.coord(), ChunkCoord(1, 2, 3));
        assert_eq!(
            restored.get_block(LocalBlockCoord::new(31, 31, 31).unwrap()),
            Some(BlockId::AIR)
        );
    }
}
