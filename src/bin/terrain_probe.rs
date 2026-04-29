use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};

use new_world::world::{
    ATLAS_CELL_SIZE_IN_CHUNKS, ATLAS_CELL_SIZE_M, AtlasArea, AtlasCoord, BLOCK_SIZE_M, CHUNK_EDGE,
    CHUNK_EDGE_M, ChunkCoord, TerrainProfileCounts, WorldMeta, generate_atlas_fields, probe_chunk,
    probe_column,
};

const DEFAULT_LOCAL_X: u8 = (CHUNK_EDGE / 2) as u8;
const DEFAULT_LOCAL_Z: u8 = (CHUNK_EDGE / 2) as u8;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let seed = parse_required::<u64>(&mut args, "seed")?;
    let mut chunk_x = 0_i32;
    let mut chunk_z = 0_i32;
    let mut local_x = DEFAULT_LOCAL_X;
    let mut local_z = DEFAULT_LOCAL_Z;

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--chunk-x" => chunk_x = parse_required::<i32>(&mut args, "chunk-x")?,
            "--chunk-z" => chunk_z = parse_required::<i32>(&mut args, "chunk-z")?,
            "--local-x" => local_x = parse_required::<u8>(&mut args, "local-x")?,
            "--local-z" => local_z = parse_required::<u8>(&mut args, "local-z")?,
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    if usize::from(local_x) >= CHUNK_EDGE || usize::from(local_z) >= CHUNK_EDGE {
        return Err(cli_error(format!(
            "local coordinates must be within 0..{}",
            CHUNK_EDGE - 1
        )));
    }

    let meta = WorldMeta::new(seed);
    let coord = ChunkCoord(chunk_x, 0, chunk_z);
    let chunk_probe = probe_chunk(coord, &meta);
    let column_probe = probe_column(coord, local_x, local_z, &meta);
    let atlas_area =
        AtlasArea::new(column_probe.atlas_coord, 2, 2).expect("probe atlas area should be valid");
    let atlas = generate_atlas_fields(&meta, atlas_area);

    println!("seed: {seed}");
    println!("chunk: ({chunk_x}, {chunk_z})");
    println!(
        "chunk footprint: {} blocks = {:.1}m per side",
        CHUNK_EDGE, CHUNK_EDGE_M
    );
    println!(
        "atlas cell footprint: {} chunks = {} blocks = {}m per side",
        ATLAS_CELL_SIZE_IN_CHUNKS,
        ATLAS_CELL_SIZE_IN_CHUNKS as usize * CHUNK_EDGE,
        ATLAS_CELL_SIZE_M
    );
    println!(
        "selected column: local=({}, {}), world=({}, {}) blocks, world=({:.1}m, {:.1}m)",
        column_probe.local_x,
        column_probe.local_z,
        column_probe.world_x,
        column_probe.world_z,
        column_probe.world_x as f32 * BLOCK_SIZE_M,
        column_probe.world_z as f32 * BLOCK_SIZE_M
    );
    println!(
        "atlas sample origin: ({}, {}), frac=({:.3}, {:.3})",
        column_probe.atlas_coord.x,
        column_probe.atlas_coord.z,
        column_probe.atlas_frac_x,
        column_probe.atlas_frac_z
    );
    println!("atlas corners:");
    for dz in 0..2 {
        for dx in 0..2 {
            let corner = AtlasCoord::new(
                column_probe.atlas_coord.x + dx,
                column_probe.atlas_coord.z + dz,
            );
            let cell = atlas.get(corner).expect("probe atlas corner should exist");
            println!(
                "  ({:>3}, {:>3}) landness={:.3} ocean={:.3} coast={:.3} macro={:.3} ridge={:.3} mountain={:.3} rugged={:.3} river={:.3} alpine={:.3}",
                corner.x,
                corner.z,
                cell.landness,
                cell.ocean_distance,
                cell.coast_factor,
                cell.macro_elevation,
                cell.ridge_factor,
                cell.mountain_mass,
                cell.ruggedness,
                cell.riverine_factor,
                cell.alpine_factor
            );
        }
    }
    println!("bilerp sample:");
    println!(
        "  landness={:.3} ocean={:.3} coast={:.3} core={:.3} macro={:.3}",
        column_probe.atlas_sample.landness,
        column_probe.atlas_sample.ocean_distance,
        column_probe.atlas_sample.coast_factor,
        column_probe.atlas_sample.continent_core_factor,
        column_probe.atlas_sample.macro_elevation
    );
    println!(
        "  ridge={:.3} mountain={:.3} rugged={:.3} river={:.3} alpine={:.3}",
        column_probe.atlas_sample.ridge_factor,
        column_probe.atlas_sample.mountain_mass,
        column_probe.atlas_sample.ruggedness,
        column_probe.atlas_sample.riverine_factor,
        column_probe.atlas_sample.alpine_factor
    );
    println!(
        "column result: profile={} surface_y={} ({:.1}m)",
        column_probe.profile.as_str(),
        column_probe.surface_y,
        column_probe.surface_y as f32 * BLOCK_SIZE_M
    );
    println!(
        "chunk summary: range={}..{} ({:.1}m..{:.1}m), mean={:.2} ({:.2}m), dominant={}",
        chunk_probe.surface_min_y,
        chunk_probe.surface_max_y,
        chunk_probe.surface_min_y as f32 * BLOCK_SIZE_M,
        chunk_probe.surface_max_y as f32 * BLOCK_SIZE_M,
        chunk_probe.surface_mean_y,
        chunk_probe.surface_mean_y * BLOCK_SIZE_M,
        chunk_probe.dominant_profile.as_str()
    );
    println!(
        "profile counts: {}",
        format_profile_counts(chunk_probe.profile_counts)
    );

    Ok(())
}

fn format_profile_counts(counts: TerrainProfileCounts) -> String {
    format!(
        "deep_ocean={} shelf={} coast={} plain={} upland={} ridge={}",
        counts.deep_ocean, counts.shelf, counts.coast, counts.plain, counts.upland, counts.ridge
    )
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
    "usage: cargo run --bin terrain_probe -- <seed> [--chunk-x <i32>] [--chunk-z <i32>] [--local-x <u8>] [--local-z <u8>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}
