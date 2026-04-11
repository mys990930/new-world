use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::chunk::ChunkData;
use super::coord::ChunkCoord;
use super::meta::WorldMeta;
use super::storage::load_chunk;

pub const BAKED_WORLD_MANIFEST_FILE: &str = "manifest.toml";
const BAKED_WORLD_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BakedWorldManifest {
    pub format_version: u32,
    pub seed: u64,
    pub generator_version: u32,
    pub save_format_version: u32,
    pub min_chunk: [i32; 3],
    pub max_chunk: [i32; 3],
    pub default_preview_center: [i32; 2],
    pub stacks: Vec<BakedStackSummary>,
}

impl BakedWorldManifest {
    pub fn min_chunk_coord(&self) -> ChunkCoord {
        ChunkCoord(self.min_chunk[0], self.min_chunk[1], self.min_chunk[2])
    }

    pub fn max_chunk_coord(&self) -> ChunkCoord {
        ChunkCoord(self.max_chunk[0], self.max_chunk[1], self.max_chunk[2])
    }

    pub fn default_preview_chunk(&self) -> ChunkCoord {
        ChunkCoord(self.default_preview_center[0], 0, self.default_preview_center[1])
    }

    pub fn contains_chunk(&self, coord: ChunkCoord) -> bool {
        let min = self.min_chunk_coord();
        let max = self.max_chunk_coord();
        coord.0 >= min.0
            && coord.0 <= max.0
            && coord.1 >= min.1
            && coord.1 <= max.1
            && coord.2 >= min.2
            && coord.2 <= max.2
    }

    pub fn world_meta(&self) -> WorldMeta {
        WorldMeta {
            seed: self.seed,
            world_version: WorldMeta::CURRENT_WORLD_VERSION,
            generator_version: self.generator_version,
            save_format_version: self.save_format_version,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BakedStackSummary {
    pub center_x: i32,
    pub center_z: i32,
    pub relief_min_y: Option<i32>,
    pub relief_max_y: Option<i32>,
    pub relief_range: i32,
    pub solid_columns: u32,
    pub score: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakedWorldSource {
    root: PathBuf,
    manifest: BakedWorldManifest,
}

impl BakedWorldSource {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, BakedWorldError> {
        let root = root.as_ref().to_path_buf();
        let manifest = read_baked_world_manifest(&root)?;
        Ok(Self { root, manifest })
    }

    pub fn root(&self) -> &Path {
        self.root.as_path()
    }

    pub fn manifest(&self) -> &BakedWorldManifest {
        &self.manifest
    }

    pub fn contains_chunk(&self, coord: ChunkCoord) -> bool {
        self.manifest.contains_chunk(coord)
    }

    pub fn default_preview_chunk(&self) -> ChunkCoord {
        self.manifest.default_preview_chunk()
    }

    pub fn load_chunk(&self, coord: ChunkCoord) -> Result<ChunkData, BakedWorldError> {
        load_baked_chunk(self.root(), coord)
    }
}

#[derive(Debug)]
pub enum BakedWorldError {
    ReadManifest {
        path: PathBuf,
        source: io::Error,
    },
    ParseManifest {
        path: PathBuf,
        source: toml::de::Error,
    },
    UnsupportedFormatVersion {
        path: PathBuf,
        found: u32,
    },
    ReadChunk {
        path: PathBuf,
        source: io::Error,
    },
    DecodeChunk {
        path: PathBuf,
        source: super::storage::StorageError,
    },
}

pub fn read_baked_world_manifest(root: &Path) -> Result<BakedWorldManifest, BakedWorldError> {
    let path = baked_world_manifest_path(root);
    let text = fs::read_to_string(&path).map_err(|source| BakedWorldError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    let manifest = toml::from_str::<BakedWorldManifest>(&text).map_err(|source| {
        BakedWorldError::ParseManifest {
            path: path.clone(),
            source,
        }
    })?;
    if manifest.format_version != BAKED_WORLD_FORMAT_VERSION {
        return Err(BakedWorldError::UnsupportedFormatVersion {
            path,
            found: manifest.format_version,
        });
    }
    Ok(manifest)
}

pub fn load_baked_chunk(root: &Path, coord: ChunkCoord) -> Result<ChunkData, BakedWorldError> {
    let path = baked_world_chunk_path(root, coord);
    let bytes = fs::read(&path).map_err(|source| BakedWorldError::ReadChunk {
        path: path.clone(),
        source,
    })?;
    load_chunk(&bytes).map_err(|source| BakedWorldError::DecodeChunk { path, source })
}

pub fn baked_world_manifest_path(root: &Path) -> PathBuf {
    root.join(BAKED_WORLD_MANIFEST_FILE)
}

pub fn baked_world_chunk_path(root: &Path, coord: ChunkCoord) -> PathBuf {
    root.join("chunks")
        .join(format!("cx{}_cy{}_cz{}.bin", coord.0, coord.1, coord.2))
}

pub fn detect_latest_baked_world_root(base_dir: &Path) -> io::Result<Option<PathBuf>> {
    if !base_dir.exists() {
        return Ok(None);
    }

    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(base_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() || !baked_world_manifest_path(&path).exists() {
            continue;
        }

        let metadata = fs::metadata(&path)?;
        let modified = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let replace = best
            .as_ref()
            .map(|(current, _)| modified > *current)
            .unwrap_or(true);
        if replace {
            best = Some((modified, path));
        }
    }

    Ok(best.map(|(_, path)| path))
}

impl std::fmt::Display for BakedWorldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadManifest { path, source } => {
                write!(f, "failed to read baked world manifest {}: {source}", path.display())
            }
            Self::ParseManifest { path, source } => {
                write!(f, "failed to parse baked world manifest {}: {source}", path.display())
            }
            Self::UnsupportedFormatVersion { path, found } => {
                write!(
                    f,
                    "unsupported baked world manifest version {found} in {}",
                    path.display()
                )
            }
            Self::ReadChunk { path, source } => {
                write!(f, "failed to read baked chunk {}: {source}", path.display())
            }
            Self::DecodeChunk { path, source } => {
                write!(
                    f,
                    "failed to decode baked chunk {}: {source:?}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for BakedWorldError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_bounds_include_default_preview_chunk() {
        let manifest = BakedWorldManifest {
            format_version: BAKED_WORLD_FORMAT_VERSION,
            seed: 42,
            generator_version: 10,
            save_format_version: 1,
            min_chunk: [-2, -8, -2],
            max_chunk: [2, 3, 2],
            default_preview_center: [0, 1],
            stacks: Vec::new(),
        };

        assert!(manifest.contains_chunk(manifest.default_preview_chunk()));
        assert!(!manifest.contains_chunk(ChunkCoord(3, 0, 1)));
    }
}
