use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

#[path = "shared/world_dump_common.rs"]
mod world_dump_common;

use world_dump_common::read_manifest;

const DEFAULT_TOP_RESULTS: usize = 16;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let world_dir = PathBuf::from(parse_required::<String>(&mut args, "world-dir")?);
    let mut top = DEFAULT_TOP_RESULTS;

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--top" => top = parse_required::<usize>(&mut args, "top")?,
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    let manifest = read_manifest(&world_dir)?;
    println!("world dir: {}", world_dir.display());
    println!("seed: {}", manifest.seed);
    println!(
        "chunk bounds: x={}..{}, y={}..{}, z={}..{}",
        manifest.min_chunk[0],
        manifest.max_chunk[0],
        manifest.min_chunk[1],
        manifest.max_chunk[1],
        manifest.min_chunk[2],
        manifest.max_chunk[2]
    );
    println!(
        "default preview center: ({}, {})",
        manifest.default_preview_center[0],
        manifest.default_preview_center[1]
    );
    println!("top stack candidates:");

    for summary in manifest.stacks.iter().take(top) {
        println!(
            "  ({:>4}, {:>4}) score={} relief={} min={:?} max={:?} solid_columns={}",
            summary.center_x,
            summary.center_z,
            summary.score,
            summary.relief_range,
            summary.relief_min_y,
            summary.relief_max_y,
            summary.solid_columns
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
    "usage: cargo run --bin world_coords -- <world-dir> [--top <usize>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}
