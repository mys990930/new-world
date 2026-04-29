#![allow(dead_code)]

use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use new_world::world::{
    CHUNK_EDGE_I32, ChunkCoord, ChunkData, VoxelizationColumnPlan, VoxelizationPlan, WORLD_FLOOR_Y,
    WorldBlockCoord, WorldCore, load_chunk, save_chunk,
};

pub const WORLD_CREATE_MANIFEST_FILE: &str = "manifest.toml";
const WORLD_CREATE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            format_version: WORLD_CREATE_FORMAT_VERSION,
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CreatedWorldStackSummary {
    pub center_x: i32,
    pub center_z: i32,
    pub relief_min_y: Option<i32>,
    pub relief_max_y: Option<i32>,
    pub relief_range: i32,
    pub solid_columns: u32,
    pub score: i64,
}

pub fn write_manifest(
    root: &Path,
    manifest: &CreatedWorldManifest,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(root)?;
    let text = toml::to_string_pretty(manifest)?;
    fs::write(manifest_path(root), text)?;
    Ok(())
}

pub fn read_manifest(root: &Path) -> Result<CreatedWorldManifest, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(manifest_path(root))?;
    let manifest = toml::from_str::<CreatedWorldManifest>(&text)?;
    if manifest.format_version != WORLD_CREATE_FORMAT_VERSION {
        return Err(format!(
            "unsupported created world manifest version: {}",
            manifest.format_version
        )
        .into());
    }
    Ok(manifest)
}

pub fn save_chunk_to_dump(
    root: &Path,
    chunk: &ChunkData,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = chunk_path(root, chunk.coord());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = save_chunk(&chunk.snapshot()).map_err(|error| {
        io::Error::new(
            ErrorKind::InvalidData,
            format!("failed to serialize chunk {:?}: {error:?}", chunk.coord()),
        )
    })?;
    fs::write(&path, bytes)?;
    Ok(path)
}

pub fn load_chunk_from_dump(
    root: &Path,
    coord: ChunkCoord,
) -> Result<ChunkData, Box<dyn std::error::Error>> {
    let bytes = fs::read(chunk_path(root, coord))?;
    load_chunk(&bytes).map_err(|error| {
        Box::new(io::Error::new(
            ErrorKind::InvalidData,
            format!("failed to deserialize chunk {coord:?}: {error:?}"),
        )) as Box<dyn std::error::Error>
    })
}

pub fn chunk_path(root: &Path, coord: ChunkCoord) -> PathBuf {
    root.join("chunks")
        .join(format!("cx{}_cy{}_cz{}.bin", coord.0, coord.1, coord.2))
}

pub fn manifest_path(root: &Path) -> PathBuf {
    root.join(WORLD_CREATE_MANIFEST_FILE)
}

pub fn summarize_stack(
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

pub fn summarize_stack_from_voxelization_plan(
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
