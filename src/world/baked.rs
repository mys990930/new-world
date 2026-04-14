use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::chunk::ChunkData;
use super::coord::{CHUNK_EDGE_I32, ChunkCoord};
use super::generation::{WORLD_FLOOR_Y, generate_chunk};
use super::meta::WorldMeta;
use super::registry::BlockRegistry;
use super::storage::{load_chunk, save_chunk};
use super::{WorldBlockCoord, WorldCore};

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
    pub fn new(
        seed: u64,
        generator_version: u32,
        save_format_version: u32,
        min_chunk: ChunkCoord,
        max_chunk: ChunkCoord,
        default_preview_center: [i32; 2],
        stacks: Vec<BakedStackSummary>,
    ) -> Self {
        Self {
            format_version: BAKED_WORLD_FORMAT_VERSION,
            seed,
            generator_version,
            save_format_version,
            min_chunk: [min_chunk.0, min_chunk.1, min_chunk.2],
            max_chunk: [max_chunk.0, max_chunk.1, max_chunk.2],
            default_preview_center,
            stacks,
        }
    }

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
    InvalidBakeConfig {
        path: PathBuf,
        message: String,
    },
    ReadManifest {
        path: PathBuf,
        source: io::Error,
    },
    ParseManifest {
        path: PathBuf,
        source: toml::de::Error,
    },
    SerializeManifest {
        path: PathBuf,
        source: toml::ser::Error,
    },
    UnsupportedFormatVersion {
        path: PathBuf,
        found: u32,
    },
    RemoveRoot {
        path: PathBuf,
        source: io::Error,
    },
    CreateDirectory {
        path: PathBuf,
        source: io::Error,
    },
    WriteManifest {
        path: PathBuf,
        source: io::Error,
    },
    ReadChunk {
        path: PathBuf,
        source: io::Error,
    },
    WriteChunk {
        path: PathBuf,
        source: io::Error,
    },
    EncodeChunk {
        path: PathBuf,
        source: super::storage::StorageError,
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

pub fn write_baked_world_manifest(
    root: &Path,
    manifest: &BakedWorldManifest,
) -> Result<(), BakedWorldError> {
    fs::create_dir_all(root).map_err(|source| BakedWorldError::CreateDirectory {
        path: root.to_path_buf(),
        source,
    })?;
    let path = baked_world_manifest_path(root);
    let text = toml::to_string_pretty(manifest).map_err(|source| BakedWorldError::SerializeManifest {
        path: path.clone(),
        source,
    })?;
    fs::write(&path, text).map_err(|source| BakedWorldError::WriteManifest {
        path: baked_world_manifest_path(root),
        source,
    })
}

pub fn save_baked_chunk(root: &Path, chunk: &ChunkData) -> Result<PathBuf, BakedWorldError> {
    let path = baked_world_chunk_path(root, chunk.coord());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| BakedWorldError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let bytes = save_chunk(&chunk.snapshot()).map_err(|source| BakedWorldError::EncodeChunk {
        path: path.clone(),
        source,
    })?;
    fs::write(&path, bytes).map_err(|source| BakedWorldError::WriteChunk {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

pub fn summarize_baked_stack(
    world: &WorldCore,
    center_x: i32,
    center_z: i32,
    min_chunk_y: i32,
    max_chunk_y: i32,
) -> BakedStackSummary {
    let min_world_y = min_chunk_y * CHUNK_EDGE_I32;
    let max_world_y = (max_chunk_y + 1) * CHUNK_EDGE_I32 - 1;

    let mut relief_min_y = i32::MAX;
    let mut relief_max_y = i32::MIN;
    let mut solid_columns = 0_u32;

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let world_x = center_x * CHUNK_EDGE_I32 + local_x;
            let world_z = center_z * CHUNK_EDGE_I32 + local_z;

            let mut top_solid_y = None;
            for world_y in (min_world_y.max(WORLD_FLOOR_Y)..=max_world_y).rev() {
                let Some(block) = world.get_block(WorldBlockCoord(world_x, world_y, world_z)) else {
                    continue;
                };
                if block.is_air() {
                    continue;
                }
                top_solid_y = Some(world_y);
                break;
            }

            if let Some(relief_y) = top_solid_y {
                solid_columns = solid_columns.saturating_add(1);
                relief_min_y = relief_min_y.min(relief_y);
                relief_max_y = relief_max_y.max(relief_y);
            }
        }
    }

    let relief_min_y = (solid_columns > 0).then_some(relief_min_y);
    let relief_max_y = (solid_columns > 0).then_some(relief_max_y);
    let relief_range = match (relief_min_y, relief_max_y) {
        (Some(min_y), Some(max_y)) => max_y - min_y,
        _ => 0,
    };
    let score = i64::from(solid_columns) * 100_000
        + i64::from(relief_range) * 1_000
        + i64::from(relief_max_y.unwrap_or(min_world_y));

    BakedStackSummary {
        center_x,
        center_z,
        relief_min_y,
        relief_max_y,
        relief_range,
        solid_columns,
        score,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BakeWorldConfig {
    pub seed: u64,
    pub center_x: i32,
    pub center_z: i32,
    pub radius: i32,
    pub min_y_chunk: i32,
    pub max_y_chunk: i32,
}

impl Default for BakeWorldConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            center_x: 0,
            center_z: 0,
            radius: 6,
            min_y_chunk: -2,
            max_y_chunk: 3,
        }
    }
}

pub fn bake_world_to_directory(
    root: &Path,
    config: BakeWorldConfig,
    block_registry: &BlockRegistry,
) -> Result<BakedWorldManifest, BakedWorldError> {
    if config.radius < 0 || config.min_y_chunk > config.max_y_chunk {
        return Err(BakedWorldError::InvalidBakeConfig {
            path: root.to_path_buf(),
            message: format!(
                "invalid bake config: radius={} min_y_chunk={} max_y_chunk={}",
                config.radius, config.min_y_chunk, config.max_y_chunk
            ),
        });
    }

    let meta = WorldMeta::new(config.seed);
    let min_chunk = ChunkCoord(
        config.center_x - config.radius,
        config.min_y_chunk,
        config.center_z - config.radius,
    );
    let max_chunk = ChunkCoord(
        config.center_x + config.radius,
        config.max_y_chunk,
        config.center_z + config.radius,
    );

    if root.exists() {
        fs::remove_dir_all(root).map_err(|source| BakedWorldError::RemoveRoot {
            path: root.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir_all(root).map_err(|source| BakedWorldError::CreateDirectory {
        path: root.to_path_buf(),
        source,
    })?;

    let mut stack_summaries = Vec::new();
    let mut best_stack: Option<BakedStackSummary> = None;

    for chunk_z in min_chunk.2..=max_chunk.2 {
        for chunk_x in min_chunk.0..=max_chunk.0 {
            let mut stack_world = WorldCore::new(meta, std::sync::Arc::new(block_registry.clone()));

            for chunk_y in min_chunk.1..=max_chunk.1 {
                let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
                let chunk = generate_chunk(coord, stack_world.meta(), block_registry);
                save_baked_chunk(root, &chunk)?;
                stack_world.insert_chunk(coord, chunk);
            }

            let summary = summarize_baked_stack(
                &stack_world,
                chunk_x,
                chunk_z,
                min_chunk.1,
                max_chunk.1,
            );
            if best_stack
                .map(|current| summary.score > current.score)
                .unwrap_or(true)
            {
                best_stack = Some(summary);
            }
            stack_summaries.push(summary);
        }
    }

    stack_summaries.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.relief_range.cmp(&left.relief_range))
            .then_with(|| left.center_z.cmp(&right.center_z))
            .then_with(|| left.center_x.cmp(&right.center_x))
    });

    let default_preview_center = best_stack
        .map(|summary| [summary.center_x, summary.center_z])
        .unwrap_or([config.center_x, config.center_z]);
    let manifest = BakedWorldManifest::new(
        config.seed,
        meta.generator_version,
        meta.save_format_version,
        min_chunk,
        max_chunk,
        default_preview_center,
        stack_summaries,
    );
    write_baked_world_manifest(root, &manifest)?;
    Ok(manifest)
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
            Self::InvalidBakeConfig { path, message } => {
                write!(f, "invalid baked world config for {}: {message}", path.display())
            }
            Self::ReadManifest { path, source } => {
                write!(f, "failed to read baked world manifest {}: {source}", path.display())
            }
            Self::ParseManifest { path, source } => {
                write!(f, "failed to parse baked world manifest {}: {source}", path.display())
            }
            Self::SerializeManifest { path, source } => {
                write!(
                    f,
                    "failed to serialize baked world manifest {}: {source}",
                    path.display()
                )
            }
            Self::UnsupportedFormatVersion { path, found } => {
                write!(
                    f,
                    "unsupported baked world manifest version {found} in {}",
                    path.display()
                )
            }
            Self::RemoveRoot { path, source } => {
                write!(
                    f,
                    "failed to clear baked world output root {}: {source}",
                    path.display()
                )
            }
            Self::CreateDirectory { path, source } => {
                write!(f, "failed to create directory {}: {source}", path.display())
            }
            Self::WriteManifest { path, source } => {
                write!(f, "failed to write baked world manifest {}: {source}", path.display())
            }
            Self::ReadChunk { path, source } => {
                write!(f, "failed to read baked chunk {}: {source}", path.display())
            }
            Self::WriteChunk { path, source } => {
                write!(f, "failed to write baked chunk {}: {source}", path.display())
            }
            Self::EncodeChunk { path, source } => {
                write!(
                    f,
                    "failed to encode baked chunk {}: {source:?}",
                    path.display()
                )
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
