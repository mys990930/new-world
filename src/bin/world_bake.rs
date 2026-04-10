use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::sync::Arc;

use new_world::world::{
    CHUNK_EDGE_I32, ChunkCoord, WORLD_FLOOR_Y, WorldCore, WorldMeta, BlockRegistry, generate_chunk,
};

#[path = "shared/world_dump_common.rs"]
mod world_dump_common;

use world_dump_common::{BakedStackSummary, BakedWorldManifest, save_chunk_to_dump, summarize_stack, write_manifest};

const DEFAULT_CENTER_X: i32 = 0;
const DEFAULT_CENTER_Z: i32 = 0;
const DEFAULT_RADIUS: i32 = 16;
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
    let mut min_y_chunk = WORLD_FLOOR_Y.div_euclid(CHUNK_EDGE_I32);
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
            "--output" => output = Some(PathBuf::from(parse_required::<String>(&mut args, "output")?)),
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
            "target/world-bake/seed_{seed}_cx{center_x}_cz{center_z}_r{radius}"
        ))
    });
    let block_registry = Arc::new(
        BlockRegistry::load_default()
            .map_err(|error| cli_error(format!("failed to load block registry: {error:?}")))?,
    );
    let meta = WorldMeta::new(seed);

    let min_chunk = ChunkCoord(center_x - radius, min_y_chunk, center_z - radius);
    let max_chunk = ChunkCoord(center_x + radius, max_y_chunk, center_z + radius);

    let mut stack_summaries = Vec::new();
    let mut best_stack: Option<BakedStackSummary> = None;

    for chunk_z in min_chunk.2..=max_chunk.2 {
        for chunk_x in min_chunk.0..=max_chunk.0 {
            let mut stack_world = WorldCore::new(meta, Arc::clone(&block_registry));

            for chunk_y in min_chunk.1..=max_chunk.1 {
                let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
                let chunk = generate_chunk(coord, stack_world.meta(), block_registry.as_ref());
                save_chunk_to_dump(&output, &chunk)?;
                stack_world.insert_chunk(coord, chunk);
            }

            let summary = summarize_stack(&stack_world, chunk_x, chunk_z, min_chunk.1, max_chunk.1);
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
        .unwrap_or([center_x, center_z]);
    let manifest = BakedWorldManifest::new(
        seed,
        meta.generator_version,
        meta.save_format_version,
        min_chunk,
        max_chunk,
        default_preview_center,
        stack_summaries.clone(),
    );
    write_manifest(&output, &manifest)?;

    println!("world bake complete");
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
    "usage: cargo run --bin world_bake -- <seed> [--center-x <i32>] [--center-z <i32>] [--radius <i32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}
