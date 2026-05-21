use std::fs;
use std::path::Path;
use std::sync::{Arc, mpsc};

use rayon::prelude::*;

use crate::world::legacy::{
    BlockRegistry, CHUNK_EDGE_I32, ChunkCoord, CreateWorldConfig, CreateWorldProgress,
    CreatedWorldError, CreatedWorldManifest, CreatedWorldStackSummary, WORLD_FLOOR_Y, WorldMeta,
    save_created_world_chunk, write_created_world_manifest,
};

use super::{
    GraphFirstVoxelBuildConfig, GraphFirstVoxelError, GraphFirstVoxelPlan,
    build_graph_first_voxel_plan, voxelize_graph_first_chunk,
};

#[derive(Debug)]
pub enum GraphFirstCreatedWorldError {
    InvalidConfig(String),
    Io(String),
    Generation(GraphFirstVoxelError),
    Storage(CreatedWorldError),
    WorkerPanic,
}

impl std::fmt::Display for GraphFirstCreatedWorldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(message) => f.write_str(message),
            Self::Io(message) => f.write_str(message),
            Self::Generation(error) => error.fmt(f),
            Self::Storage(error) => error.fmt(f),
            Self::WorkerPanic => f.write_str("graph-first create-world worker panicked"),
        }
    }
}

impl std::error::Error for GraphFirstCreatedWorldError {}

impl From<GraphFirstVoxelError> for GraphFirstCreatedWorldError {
    fn from(value: GraphFirstVoxelError) -> Self {
        Self::Generation(value)
    }
}

impl From<CreatedWorldError> for GraphFirstCreatedWorldError {
    fn from(value: CreatedWorldError) -> Self {
        Self::Storage(value)
    }
}

#[derive(Debug, Clone, Copy)]
struct GeneratedGraphFirstCreatedWorldStack {
    summary: CreatedWorldStackSummary,
    written_chunks: u32,
}

pub fn create_graph_first_world_to_directory_with_progress(
    root: &Path,
    config: CreateWorldConfig,
    block_registry: Arc<BlockRegistry>,
    mut report_progress: impl FnMut(CreateWorldProgress),
) -> Result<CreatedWorldManifest, GraphFirstCreatedWorldError> {
    if config.radius < 0 || config.min_y_chunk > config.max_y_chunk {
        return Err(GraphFirstCreatedWorldError::InvalidConfig(format!(
            "invalid create-world config: radius={} min_y_chunk={} max_y_chunk={}",
            config.radius, config.min_y_chunk, config.max_y_chunk
        )));
    }

    let meta = WorldMeta::new(config.seed);
    let min_x = checked_chunk_bound(config.center_x.checked_sub(config.radius), config)?;
    let max_x = checked_chunk_bound(config.center_x.checked_add(config.radius), config)?;
    let min_z = checked_chunk_bound(config.center_z.checked_sub(config.radius), config)?;
    let max_z = checked_chunk_bound(config.center_z.checked_add(config.radius), config)?;
    let min_chunk = ChunkCoord(min_x, config.min_y_chunk, min_z);
    let max_chunk = ChunkCoord(max_x, config.max_y_chunk, max_z);
    let total_chunks = config.total_chunk_count().ok_or_else(|| {
        GraphFirstCreatedWorldError::InvalidConfig(format!(
            "invalid create-world chunk count: radius={}",
            config.radius
        ))
    })?;

    if root.exists() {
        fs::remove_dir_all(root).map_err(|error| {
            GraphFirstCreatedWorldError::Io(format!(
                "failed to remove existing world root: {error}"
            ))
        })?;
    }
    fs::create_dir_all(root).map_err(|error| {
        GraphFirstCreatedWorldError::Io(format!(
            "failed to create world root {}: {error}",
            root.display()
        ))
    })?;

    report_progress(CreateWorldProgress {
        completed_chunks: 0,
        total_chunks,
    });

    let voxel_plan = Arc::new(build_graph_first_voxel_plan(
        &meta,
        min_chunk.0,
        max_chunk.0,
        min_chunk.2,
        max_chunk.2,
        GraphFirstVoxelBuildConfig::new(meta.seed, meta.generator_version),
    )?);

    let stack_coords = created_world_stack_xz_coords(min_chunk, max_chunk);
    let (tx, rx) = mpsc::channel();
    let mut stack_summaries = Vec::new();
    let mut best_stack: Option<CreatedWorldStackSummary> = None;
    let mut completed_chunks = 0_u32;
    let mut first_error: Option<GraphFirstCreatedWorldError> = None;

    std::thread::scope(|scope| {
        let root = root.to_path_buf();
        let voxel_plan = voxel_plan.clone();
        let block_registry = block_registry.clone();
        let worker = scope.spawn(move || {
            stack_coords.into_par_iter().for_each(|(chunk_x, chunk_z)| {
                let result = generate_graph_first_created_world_stack(
                    root.as_path(),
                    chunk_x,
                    chunk_z,
                    min_chunk.1,
                    max_chunk.1,
                    voxel_plan.as_ref(),
                    block_registry.as_ref(),
                );
                let _ = tx.send(result);
            });
        });

        for generated in rx {
            let generated = match generated {
                Ok(generated) => generated,
                Err(error) => {
                    first_error.get_or_insert(error);
                    continue;
                }
            };
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

        if worker.join().is_err() {
            first_error.get_or_insert(GraphFirstCreatedWorldError::WorkerPanic);
        }
    });

    if let Some(error) = first_error {
        return Err(error);
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

fn checked_chunk_bound(
    bound: Option<i32>,
    config: CreateWorldConfig,
) -> Result<i32, GraphFirstCreatedWorldError> {
    bound.ok_or_else(|| {
        GraphFirstCreatedWorldError::InvalidConfig(format!(
            "create-world bounds overflow: center_x={} center_z={} radius={}",
            config.center_x, config.center_z, config.radius
        ))
    })
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

fn generate_graph_first_created_world_stack(
    root: &Path,
    chunk_x: i32,
    chunk_z: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
    voxel_plan: &GraphFirstVoxelPlan,
    block_registry: &BlockRegistry,
) -> Result<GeneratedGraphFirstCreatedWorldStack, GraphFirstCreatedWorldError> {
    let mut written_chunks = 0_u32;

    for chunk_y in min_y_chunk..=max_y_chunk {
        let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
        let chunk = voxelize_graph_first_chunk(coord, voxel_plan, block_registry)?;
        save_created_world_chunk(root, &chunk)?;
        written_chunks = written_chunks.saturating_add(1);
    }

    Ok(GeneratedGraphFirstCreatedWorldStack {
        summary: summarize_graph_first_created_world_stack(
            voxel_plan,
            chunk_x,
            chunk_z,
            min_y_chunk,
            max_y_chunk,
        ),
        written_chunks,
    })
}

fn summarize_graph_first_created_world_stack(
    plan: &GraphFirstVoxelPlan,
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
        for column in plan.columns_for_chunk_xz(center_x, center_z) {
            let top_y = column.top_non_air_y();
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
