use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::sync::Arc;

use rayon::prelude::*;

use new_world::world::{
    BlockRegistry, ChunkCoord, GraphFirstVoxelBuildConfig, GraphFirstVoxelPlan, WorldMeta,
    build_graph_first_voxel_plan, voxelize_graph_first_chunk,
};

#[path = "shared/world_dump_common.rs"]
mod world_dump_common;

use world_dump_common::{
    CreatedWorldManifest, CreatedWorldStackSummary, save_chunk_to_dump,
    summarize_stack_from_graph_first_voxel_plan, write_manifest,
};

const DEFAULT_CENTER_X: i32 = 0;
const DEFAULT_CENTER_Z: i32 = 0;
const DEFAULT_RADIUS: i32 = 16;
const DEFAULT_MIN_Y_CHUNK: i32 = -2;
const DEFAULT_MAX_Y_CHUNK: i32 = 3;
const DEFAULT_TOP_RESULTS: usize = 8;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let seed = parse_required::<u64>(&mut args, "seed")?;
    let mut center_x = DEFAULT_CENTER_X;
    let mut center_z = DEFAULT_CENTER_Z;
    let mut radius = DEFAULT_RADIUS;
    let mut min_y_chunk = DEFAULT_MIN_Y_CHUNK;
    let mut max_y_chunk = DEFAULT_MAX_Y_CHUNK;
    let mut output: Option<PathBuf> = None;

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--center-x" => center_x = parse_required::<i32>(&mut args, "center-x")?,
            "--center-z" => center_z = parse_required::<i32>(&mut args, "center-z")?,
            "--radius" => radius = parse_required::<i32>(&mut args, "radius")?,
            "--min-y-chunk" => min_y_chunk = parse_required::<i32>(&mut args, "min-y-chunk")?,
            "--max-y-chunk" => max_y_chunk = parse_required::<i32>(&mut args, "max-y-chunk")?,
            "--output" => {
                output = Some(PathBuf::from(parse_required::<String>(
                    &mut args, "output",
                )?))
            }
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    if radius < 0 {
        return Err(cli_error("radius must be non-negative"));
    }
    if min_y_chunk > max_y_chunk {
        return Err(cli_error("min-y-chunk must be <= max-y-chunk"));
    }

    let output = output.unwrap_or_else(|| {
        PathBuf::from(format!(
            "target/world-create/seed_{seed}_cx{center_x}_cz{center_z}_r{radius}"
        ))
    });
    let block_registry = Arc::new(
        BlockRegistry::load_default()
            .map_err(|error| cli_error(format!("failed to load block registry: {error:?}")))?,
    );
    let meta = WorldMeta::new(seed);

    let min_chunk = ChunkCoord(center_x - radius, min_y_chunk, center_z - radius);
    let max_chunk = ChunkCoord(center_x + radius, max_y_chunk, center_z + radius);

    let stack_coords = stack_xz_coords(min_chunk, max_chunk);
    let voxel_plan = Arc::new(build_graph_first_voxel_plan(
        &meta,
        min_chunk.0,
        max_chunk.0,
        min_chunk.2,
        max_chunk.2,
        GraphFirstVoxelBuildConfig::new(meta.seed, meta.generator_version),
    )?);
    let generated_stacks = stack_coords
        .into_par_iter()
        .map(|(chunk_x, chunk_z)| {
            generate_stack(
                chunk_x,
                chunk_z,
                min_chunk.1,
                max_chunk.1,
                voxel_plan.as_ref(),
                block_registry.as_ref(),
                output.as_path(),
            )
        })
        .collect::<Vec<_>>();

    let mut stack_summaries = Vec::with_capacity(generated_stacks.len());
    let mut best_stack: Option<CreatedWorldStackSummary> = None;

    for generated in generated_stacks {
        let summary = generated.map_err(cli_error)?;
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
        .unwrap_or([center_x, center_z]);
    let manifest = CreatedWorldManifest::new(
        seed,
        meta.generator_version,
        meta.save_format_version,
        min_chunk,
        max_chunk,
        default_preview_center,
        stack_summaries.clone(),
    );
    write_manifest(&output, &manifest)?;

    println!("world create complete");
    println!("generator: graph-first voxel fill");
    println!("seed: {seed}");
    println!(
        "chunk bounds: x={}..{}, y={}..{}, z={}..{}",
        min_chunk.0, max_chunk.0, min_chunk.1, max_chunk.1, min_chunk.2, max_chunk.2
    );
    println!(
        "default preview center: ({}, {})",
        default_preview_center[0], default_preview_center[1]
    );
    println!("output: {}", output.display());
    println!("top stack candidates:");
    for summary in stack_summaries.iter().take(DEFAULT_TOP_RESULTS) {
        println!(
            "  ({:>4}, {:>4}) score={} relief={} solid_columns={} top={:?}",
            summary.center_x,
            summary.center_z,
            summary.score,
            summary.relief_range,
            summary.solid_columns,
            summary.relief_max_y
        );
    }

    Ok(())
}

fn stack_xz_coords(min_chunk: ChunkCoord, max_chunk: ChunkCoord) -> Vec<(i32, i32)> {
    let mut coords = Vec::new();
    for chunk_z in min_chunk.2..=max_chunk.2 {
        for chunk_x in min_chunk.0..=max_chunk.0 {
            coords.push((chunk_x, chunk_z));
        }
    }
    coords
}

fn generate_stack(
    chunk_x: i32,
    chunk_z: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
    voxel_plan: &GraphFirstVoxelPlan,
    block_registry: &BlockRegistry,
    output: &std::path::Path,
) -> Result<CreatedWorldStackSummary, String> {
    for chunk_y in min_y_chunk..=max_y_chunk {
        let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
        let chunk = voxelize_graph_first_chunk(coord, voxel_plan, block_registry)
            .map_err(|error| error.to_string())?;
        save_chunk_to_dump(output, &chunk).map_err(|error| error.to_string())?;
    }

    Ok(summarize_stack_from_graph_first_voxel_plan(
        voxel_plan,
        chunk_x,
        chunk_z,
        min_y_chunk,
        max_y_chunk,
    ))
}

fn parse_required<T>(args: &mut Vec<String>, label: &str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
{
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| cli_error(format!("missing {label}\n\n{}", usage())))?;
    args.remove(0);
    value
        .parse::<T>()
        .map_err(|error| cli_error(format!("invalid {label}: {error}")))
}

fn usage() -> &'static str {
    "usage: cargo run --bin world_create -- <seed> [--center-x <i32>] [--center-z <i32>] [--radius <i32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}
