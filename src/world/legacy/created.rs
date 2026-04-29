use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::chunk::ChunkData;
use super::coord::{CHUNK_EDGE_I32, ChunkCoord};
use super::generation::{
    ChunkGenerationInputs, VoxelizationColumnPlan, VoxelizationPlan, WORLD_FLOOR_Y,
    build_chunk_generation_voxelization_plan, chunk_generation_input_area,
    generate_chunk_from_voxelization_plan, prepare_chunk_generation_inputs,
};
use super::meta::WorldMeta;
use super::registry::BlockRegistry;
use super::storage::{load_chunk, save_chunk};
use super::{WorldBlockCoord, WorldCore};

pub const CREATED_WORLD_MANIFEST_FILE: &str = "manifest.toml";
const CREATED_WORLD_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreatedWorldManifest {
    pub format_version: u32,
    pub seed: u64,
    pub generator_version: u32,
    pub save_format_version: u32,
    pub min_chunk: [i32; 3],
    pub max_chunk: [i32; 3],
    pub default_preview_center: [i32; 2],
    pub stacks: Vec<CreatedWorldStackSummary>,
}

impl CreatedWorldManifest {
    pub fn new(
        seed: u64,
        generator_version: u32,
        save_format_version: u32,
        min_chunk: ChunkCoord,
        max_chunk: ChunkCoord,
        default_preview_center: [i32; 2],
        stacks: Vec<CreatedWorldStackSummary>,
    ) -> Self {
        Self {
            format_version: CREATED_WORLD_FORMAT_VERSION,
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
        ChunkCoord(
            self.default_preview_center[0],
            0,
            self.default_preview_center[1],
        )
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
pub struct CreatedWorldStackSummary {
    pub center_x: i32,
    pub center_z: i32,
    pub relief_min_y: Option<i32>,
    pub relief_max_y: Option<i32>,
    pub relief_range: i32,
    pub solid_columns: u32,
    pub score: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedWorldSource {
    root: PathBuf,
    manifest: CreatedWorldManifest,
}

impl CreatedWorldSource {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CreatedWorldError> {
        let root = root.as_ref().to_path_buf();
        let manifest = read_created_world_manifest(&root)?;
        Ok(Self { root, manifest })
    }

    pub fn root(&self) -> &Path {
        self.root.as_path()
    }

    pub fn manifest(&self) -> &CreatedWorldManifest {
        &self.manifest
    }

    pub fn contains_chunk(&self, coord: ChunkCoord) -> bool {
        self.manifest.contains_chunk(coord)
    }

    pub fn default_preview_chunk(&self) -> ChunkCoord {
        self.manifest.default_preview_chunk()
    }

    pub fn load_chunk(&self, coord: ChunkCoord) -> Result<ChunkData, CreatedWorldError> {
        load_created_world_chunk(self.root(), coord)
    }
}

#[derive(Debug)]
pub enum CreatedWorldError {
    InvalidCreateWorldConfig {
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

pub fn read_created_world_manifest(root: &Path) -> Result<CreatedWorldManifest, CreatedWorldError> {
    let path = created_world_manifest_path(root);
    let text = fs::read_to_string(&path).map_err(|source| CreatedWorldError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    let manifest = toml::from_str::<CreatedWorldManifest>(&text).map_err(|source| {
        CreatedWorldError::ParseManifest {
            path: path.clone(),
            source,
        }
    })?;
    if manifest.format_version != CREATED_WORLD_FORMAT_VERSION {
        return Err(CreatedWorldError::UnsupportedFormatVersion {
            path,
            found: manifest.format_version,
        });
    }
    Ok(manifest)
}

pub fn load_created_world_chunk(
    root: &Path,
    coord: ChunkCoord,
) -> Result<ChunkData, CreatedWorldError> {
    let path = created_world_chunk_path(root, coord);
    let bytes = fs::read(&path).map_err(|source| CreatedWorldError::ReadChunk {
        path: path.clone(),
        source,
    })?;
    load_chunk(&bytes).map_err(|source| CreatedWorldError::DecodeChunk { path, source })
}

pub fn write_created_world_manifest(
    root: &Path,
    manifest: &CreatedWorldManifest,
) -> Result<(), CreatedWorldError> {
    fs::create_dir_all(root).map_err(|source| CreatedWorldError::CreateDirectory {
        path: root.to_path_buf(),
        source,
    })?;
    let path = created_world_manifest_path(root);
    let text = toml::to_string_pretty(manifest).map_err(|source| {
        CreatedWorldError::SerializeManifest {
            path: path.clone(),
            source,
        }
    })?;
    fs::write(&path, text).map_err(|source| CreatedWorldError::WriteManifest {
        path: created_world_manifest_path(root),
        source,
    })
}

pub fn save_created_world_chunk(
    root: &Path,
    chunk: &ChunkData,
) -> Result<PathBuf, CreatedWorldError> {
    let path = created_world_chunk_path(root, chunk.coord());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CreatedWorldError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let bytes = save_chunk(&chunk.snapshot()).map_err(|source| CreatedWorldError::EncodeChunk {
        path: path.clone(),
        source,
    })?;
    fs::write(&path, bytes).map_err(|source| CreatedWorldError::WriteChunk {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

pub fn summarize_created_world_stack(
    world: &WorldCore,
    center_x: i32,
    center_z: i32,
    min_chunk_y: i32,
    max_chunk_y: i32,
) -> CreatedWorldStackSummary {
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
                let Some(block) = world.get_block(WorldBlockCoord(world_x, world_y, world_z))
                else {
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

    CreatedWorldStackSummary {
        center_x,
        center_z,
        relief_min_y,
        relief_max_y,
        relief_range,
        solid_columns,
        score,
    }
}

pub fn summarize_created_world_stack_from_voxelization_plan(
    plan: &VoxelizationPlan,
    center_x: i32,
    center_z: i32,
    min_chunk_y: i32,
    max_chunk_y: i32,
) -> CreatedWorldStackSummary {
    let min_world_y = min_chunk_y * CHUNK_EDGE_I32;
    let max_world_y = (max_chunk_y + 1) * CHUNK_EDGE_I32 - 1;
    let scan_min_y = min_world_y.max(WORLD_FLOOR_Y);

    let mut relief_min_y = i32::MAX;
    let mut relief_max_y = i32::MIN;
    let mut solid_columns = 0_u32;

    if scan_min_y <= max_world_y {
        for column in &plan.columns {
            let top_y = column_top_non_air_y(*column);
            if top_y < scan_min_y {
                continue;
            }

            let relief_y = top_y.min(max_world_y);
            solid_columns = solid_columns.saturating_add(1);
            relief_min_y = relief_min_y.min(relief_y);
            relief_max_y = relief_max_y.max(relief_y);
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

    CreatedWorldStackSummary {
        center_x,
        center_z,
        relief_min_y,
        relief_max_y,
        relief_range,
        solid_columns,
        score,
    }
}

fn column_top_non_air_y(column: VoxelizationColumnPlan) -> i32 {
    column
        .water_top_y
        .unwrap_or(i32::MIN)
        .max(column.terrain_top_y)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateWorldConfig {
    pub seed: u64,
    pub center_x: i32,
    pub center_z: i32,
    pub radius: i32,
    pub min_y_chunk: i32,
    pub max_y_chunk: i32,
}

impl CreateWorldConfig {
    pub fn total_chunk_count(self) -> Option<u32> {
        if self.radius < 0 || self.min_y_chunk > self.max_y_chunk {
            return None;
        }

        let horizontal_edge = i64::from(self.radius).checked_mul(2)?.checked_add(1)?;
        let vertical_edge = i64::from(self.max_y_chunk)
            .checked_sub(i64::from(self.min_y_chunk))?
            .checked_add(1)?;
        let total = horizontal_edge
            .checked_mul(horizontal_edge)?
            .checked_mul(vertical_edge)?;
        u32::try_from(total).ok()
    }
}

impl Default for CreateWorldConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            center_x: 0,
            center_z: 0,
            radius: 0,
            min_y_chunk: -2,
            max_y_chunk: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateWorldProgress {
    pub completed_chunks: u32,
    pub total_chunks: u32,
}

pub fn create_world_to_directory(
    root: &Path,
    config: CreateWorldConfig,
    block_registry: &BlockRegistry,
) -> Result<CreatedWorldManifest, CreatedWorldError> {
    create_world_to_directory_with_progress(root, config, block_registry, |_| {})
}

pub fn create_world_to_directory_with_progress(
    root: &Path,
    config: CreateWorldConfig,
    block_registry: &BlockRegistry,
    mut report_progress: impl FnMut(CreateWorldProgress),
) -> Result<CreatedWorldManifest, CreatedWorldError> {
    if config.radius < 0 || config.min_y_chunk > config.max_y_chunk {
        return Err(CreatedWorldError::InvalidCreateWorldConfig {
            path: root.to_path_buf(),
            message: format!(
                "invalid create-world config: radius={} min_y_chunk={} max_y_chunk={}",
                config.radius, config.min_y_chunk, config.max_y_chunk
            ),
        });
    }

    let meta = WorldMeta::new(config.seed);
    let min_x = checked_chunk_bound(root, config, config.center_x.checked_sub(config.radius))?;
    let max_x = checked_chunk_bound(root, config, config.center_x.checked_add(config.radius))?;
    let min_z = checked_chunk_bound(root, config, config.center_z.checked_sub(config.radius))?;
    let max_z = checked_chunk_bound(root, config, config.center_z.checked_add(config.radius))?;
    let total_chunks =
        config
            .total_chunk_count()
            .ok_or_else(|| CreatedWorldError::InvalidCreateWorldConfig {
                path: root.to_path_buf(),
                message: format!(
                    "invalid create-world chunk count: radius={} min_y_chunk={} max_y_chunk={}",
                    config.radius, config.min_y_chunk, config.max_y_chunk
                ),
            })?;
    let min_chunk = ChunkCoord(min_x, config.min_y_chunk, min_z);
    let max_chunk = ChunkCoord(max_x, config.max_y_chunk, max_z);

    if root.exists() {
        fs::remove_dir_all(root).map_err(|source| CreatedWorldError::RemoveRoot {
            path: root.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir_all(root).map_err(|source| CreatedWorldError::CreateDirectory {
        path: root.to_path_buf(),
        source,
    })?;

    let mut stack_summaries = Vec::new();
    let mut best_stack: Option<CreatedWorldStackSummary> = None;
    let mut completed_chunks = 0_u32;
    let stack_coords = created_world_stack_xz_coords(min_chunk, max_chunk);
    report_progress(CreateWorldProgress {
        completed_chunks,
        total_chunks,
    });
    let input_cache = CreatedWorldInputCache::for_stacks(&meta, &stack_coords);

    let generated_stacks = stack_coords
        .par_iter()
        .copied()
        .map(|(chunk_x, chunk_z)| {
            generate_created_world_stack(
                root,
                chunk_x,
                chunk_z,
                min_chunk.1,
                max_chunk.1,
                &input_cache,
                block_registry,
            )
        })
        .collect::<Vec<_>>();

    for generated in generated_stacks {
        let generated = generated?;
        completed_chunks = completed_chunks.saturating_add(generated.written_chunks);
        report_progress(CreateWorldProgress {
            completed_chunks,
            total_chunks,
        });
        let summary = generated.summary;
        if best_stack
            .map(|current| summary.score > current.score)
            .unwrap_or(true)
        {
            best_stack = Some(summary);
        }
        stack_summaries.push(summary);
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
    let manifest = CreatedWorldManifest::new(
        config.seed,
        meta.generator_version,
        meta.save_format_version,
        min_chunk,
        max_chunk,
        default_preview_center,
        stack_summaries,
    );
    write_created_world_manifest(root, &manifest)?;
    Ok(manifest)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CreatedWorldInputCacheKey {
    origin_x: i32,
    origin_z: i32,
    width: u32,
    height: u32,
}

impl CreatedWorldInputCacheKey {
    fn for_chunk(coord: ChunkCoord) -> Self {
        let area = chunk_generation_input_area(coord);

        Self {
            origin_x: area.origin().x,
            origin_z: area.origin().z,
            width: area.width(),
            height: area.height(),
        }
    }
}

#[derive(Debug, Clone)]
struct CreatedWorldInputCache {
    entries: HashMap<CreatedWorldInputCacheKey, ChunkGenerationInputs>,
}

impl CreatedWorldInputCache {
    fn for_stacks(meta: &WorldMeta, stack_coords: &[(i32, i32)]) -> Self {
        let mut representatives = HashMap::<CreatedWorldInputCacheKey, ChunkCoord>::new();
        for &(chunk_x, chunk_z) in stack_coords {
            let coord = ChunkCoord(chunk_x, 0, chunk_z);
            representatives
                .entry(CreatedWorldInputCacheKey::for_chunk(coord))
                .or_insert(coord);
        }

        let entries = representatives
            .into_par_iter()
            .map(|(key, coord)| (key, prepare_chunk_generation_inputs(coord, meta)))
            .collect::<HashMap<_, _>>();

        Self { entries }
    }

    fn inputs_for_chunk(&self, coord: ChunkCoord) -> ChunkGenerationInputs {
        let key = CreatedWorldInputCacheKey::for_chunk(coord);
        let mut inputs = self
            .entries
            .get(&key)
            .expect("created-world input cache should cover every requested stack")
            .clone();
        inputs.chunk = coord;
        inputs
    }
}

#[derive(Debug, Clone, Copy)]
struct GeneratedCreatedWorldStack {
    summary: CreatedWorldStackSummary,
    written_chunks: u32,
}

fn created_world_stack_xz_coords(min_chunk: ChunkCoord, max_chunk: ChunkCoord) -> Vec<(i32, i32)> {
    let mut coords = Vec::new();
    for chunk_z in min_chunk.2..=max_chunk.2 {
        for chunk_x in min_chunk.0..=max_chunk.0 {
            coords.push((chunk_x, chunk_z));
        }
    }
    coords
}

fn generate_created_world_stack(
    root: &Path,
    chunk_x: i32,
    chunk_z: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
    input_cache: &CreatedWorldInputCache,
    block_registry: &BlockRegistry,
) -> Result<GeneratedCreatedWorldStack, CreatedWorldError> {
    let plan_coord = ChunkCoord(chunk_x, 0, chunk_z);
    let inputs = input_cache.inputs_for_chunk(plan_coord);
    let voxelization = build_chunk_generation_voxelization_plan(&inputs);
    let mut written_chunks = 0_u32;

    for chunk_y in min_y_chunk..=max_y_chunk {
        let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
        let chunk = generate_chunk_from_voxelization_plan(coord, &voxelization, block_registry);
        save_created_world_chunk(root, &chunk)?;
        written_chunks = written_chunks.saturating_add(1);
    }

    let summary = summarize_created_world_stack_from_voxelization_plan(
        &voxelization,
        chunk_x,
        chunk_z,
        min_y_chunk,
        max_y_chunk,
    );

    Ok(GeneratedCreatedWorldStack {
        summary,
        written_chunks,
    })
}

fn checked_chunk_bound(
    root: &Path,
    config: CreateWorldConfig,
    bound: Option<i32>,
) -> Result<i32, CreatedWorldError> {
    bound.ok_or_else(|| CreatedWorldError::InvalidCreateWorldConfig {
        path: root.to_path_buf(),
        message: format!(
            "create-world bounds overflow: center_x={} center_z={} radius={}",
            config.center_x, config.center_z, config.radius
        ),
    })
}

pub fn created_world_manifest_path(root: &Path) -> PathBuf {
    root.join(CREATED_WORLD_MANIFEST_FILE)
}

pub fn created_world_chunk_path(root: &Path, coord: ChunkCoord) -> PathBuf {
    root.join("chunks")
        .join(format!("cx{}_cy{}_cz{}.bin", coord.0, coord.1, coord.2))
}

pub fn detect_latest_created_world_root(base_dir: &Path) -> io::Result<Option<PathBuf>> {
    if !base_dir.exists() {
        return Ok(None);
    }

    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(base_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() || !created_world_manifest_path(&path).exists() {
            continue;
        }

        let metadata = fs::metadata(&path)?;
        let modified = metadata
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
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

impl std::fmt::Display for CreatedWorldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCreateWorldConfig { path, message } => {
                write!(
                    f,
                    "invalid created world config for {}: {message}",
                    path.display()
                )
            }
            Self::ReadManifest { path, source } => {
                write!(
                    f,
                    "failed to read created world manifest {}: {source}",
                    path.display()
                )
            }
            Self::ParseManifest { path, source } => {
                write!(
                    f,
                    "failed to parse created world manifest {}: {source}",
                    path.display()
                )
            }
            Self::SerializeManifest { path, source } => {
                write!(
                    f,
                    "failed to serialize created world manifest {}: {source}",
                    path.display()
                )
            }
            Self::UnsupportedFormatVersion { path, found } => {
                write!(
                    f,
                    "unsupported created world manifest version {found} in {}",
                    path.display()
                )
            }
            Self::RemoveRoot { path, source } => {
                write!(
                    f,
                    "failed to clear created world output root {}: {source}",
                    path.display()
                )
            }
            Self::CreateDirectory { path, source } => {
                write!(f, "failed to create directory {}: {source}", path.display())
            }
            Self::WriteManifest { path, source } => {
                write!(
                    f,
                    "failed to write created world manifest {}: {source}",
                    path.display()
                )
            }
            Self::ReadChunk { path, source } => {
                write!(
                    f,
                    "failed to read created-world chunk {}: {source}",
                    path.display()
                )
            }
            Self::WriteChunk { path, source } => {
                write!(
                    f,
                    "failed to write created-world chunk {}: {source}",
                    path.display()
                )
            }
            Self::EncodeChunk { path, source } => {
                write!(
                    f,
                    "failed to encode created-world chunk {}: {source:?}",
                    path.display()
                )
            }
            Self::DecodeChunk { path, source } => {
                write!(
                    f,
                    "failed to decode created-world chunk {}: {source:?}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for CreatedWorldError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_bounds_include_default_preview_chunk() {
        let manifest = CreatedWorldManifest {
            format_version: CREATED_WORLD_FORMAT_VERSION,
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

    #[test]
    fn create_world_config_counts_requested_chunks() {
        let config = CreateWorldConfig {
            seed: 42,
            center_x: 0,
            center_z: 0,
            radius: 2,
            min_y_chunk: -1,
            max_y_chunk: 1,
        };

        assert_eq!(config.total_chunk_count(), Some(75));
    }

    #[test]
    fn default_create_world_config_is_single_stack() {
        let config = CreateWorldConfig::default();

        assert_eq!(config.radius, 0);
        assert_eq!(config.total_chunk_count(), Some(6));
    }

    fn synthetic_column(terrain_top_y: i32, water_top_y: Option<i32>) -> VoxelizationColumnPlan {
        VoxelizationColumnPlan {
            owner_archetype: crate::world::RegionArchetype::TemperatePlain,
            material_policy: crate::world::MaterialPolicyId::TemperateGrassland,
            seasonal_state: None,
            cover_phase: crate::world::CoverPhase::Growing,
            cover_override_key: None,
            terrain_top_y,
            water_top_y,
            top_block_key: "grass",
            filler_block_key: "dirt",
            core_block_key: "stone",
            water_block_key: water_top_y.map(|_| "water"),
            filler_depth: 4,
        }
    }

    fn synthetic_plan(chunk: ChunkCoord, column: VoxelizationColumnPlan) -> VoxelizationPlan {
        VoxelizationPlan {
            chunk,
            columns: vec![column; (CHUNK_EDGE_I32 * CHUNK_EDGE_I32) as usize],
        }
    }

    #[test]
    fn stack_summary_can_be_derived_from_voxelization_plan() {
        let plan = synthetic_plan(ChunkCoord(0, 0, 0), synthetic_column(50, Some(52)));

        let summary = summarize_created_world_stack_from_voxelization_plan(&plan, 0, 0, -2, 3);

        assert_eq!(summary.solid_columns, 1024);
        assert_eq!(summary.relief_min_y, Some(52));
        assert_eq!(summary.relief_max_y, Some(52));
        assert_eq!(summary.relief_range, 0);
    }

    #[test]
    fn stack_summary_clips_to_requested_vertical_window() {
        let plan = synthetic_plan(ChunkCoord(0, 0, 0), synthetic_column(50, None));

        let underground = summarize_created_world_stack_from_voxelization_plan(&plan, 0, 0, -2, -1);
        let sky = summarize_created_world_stack_from_voxelization_plan(&plan, 0, 0, 3, 3);

        assert_eq!(underground.solid_columns, 1024);
        assert_eq!(underground.relief_min_y, Some(-1));
        assert_eq!(underground.relief_max_y, Some(-1));
        assert_eq!(sky.solid_columns, 0);
        assert_eq!(sky.relief_min_y, None);
        assert_eq!(sky.relief_max_y, None);
    }
}
