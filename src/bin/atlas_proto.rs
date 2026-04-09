use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

use new_world::world::{
    AtlasArea, AtlasCoord, AtlasDebugOptions, WorldMeta, generate_atlas_fields, resolve_atlas,
    write_debug_images_with_options,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 {
        return Err(cli_error(usage()));
    }

    let seed = parse_required::<u64>(&mut args, "seed")?;
    let width = parse_required::<u32>(&mut args, "width")?;
    let mut height = width;

    if args.first().is_some_and(|value| !value.starts_with("--")) {
        height = parse_required::<u32>(&mut args, "height")?;
    }

    let mut origin_x = -(width as i32 / 2);
    let mut origin_z = -(height as i32 / 2);
    let mut pixels_per_cell = 4_u32;
    let mut output_dir = PathBuf::from(format!(
        "target/atlas-debug/seed_{seed}_{width}x{height}"
    ));

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--origin-x" => origin_x = parse_required::<i32>(&mut args, "origin-x")?,
            "--origin-z" => origin_z = parse_required::<i32>(&mut args, "origin-z")?,
            "--pixels" => pixels_per_cell = parse_required::<u32>(&mut args, "pixels")?,
            "--output" => output_dir = PathBuf::from(parse_required::<String>(&mut args, "output")?),
            _ => {
                return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage())));
            }
        }
    }

    let meta = WorldMeta::new(seed);
    let area = AtlasArea::new(AtlasCoord::new(origin_x, origin_z), width, height)?;
    let fields = generate_atlas_fields(&meta, area);
    let resolved = resolve_atlas(&fields);
    let files = write_debug_images_with_options(
        &fields,
        &resolved,
        &output_dir,
        AtlasDebugOptions { pixels_per_cell },
    )?;

    println!("atlas seed: {seed}");
    println!(
        "atlas area: origin=({}, {}), size={}x{}, pixels_per_cell={}",
        origin_x, origin_z, width, height, pixels_per_cell
    );
    println!("output dir: {}", output_dir.display());
    println!();
    println!("generated files:");
    for file in files {
        println!("{}", file.display());
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
    "usage: cargo run --bin atlas_proto -- <seed> <width> [height] [--origin-x <i32>] [--origin-z <i32>] [--pixels <u32>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}
