use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

use image::{ImageBuffer, Rgba};
use new_world::world::{
    ATLAS_CELL_SIZE_IN_CHUNKS, ATLAS_CELL_SIZE_M, AtlasArea, AtlasCell, AtlasCoord, AtlasFieldMap,
    BLOCK_SIZE_M, ChunkCoord, TerrainProfile, WorldMeta, generate_atlas_fields, probe_chunk,
};

const DEFAULT_RADIUS_CELLS: i32 = 64;
const DEFAULT_MIN_LENGTH_CELLS: u32 = 10;
const DEFAULT_MAX_LENGTH_CELLS: u32 = 18;
const DEFAULT_OUTPUT: &str = "target/terrain-corridor";
const SAMPLE_PIXELS_PER_CHUNK: u32 = 3;
const CHART_HEIGHT: u32 = 360;
const CHART_MARGIN: u32 = 24;
const SEA_LEVEL_COLOR: Rgba<u8> = Rgba([72, 138, 196, 255]);
const SKY_COLOR: Rgba<u8> = Rgba([194, 210, 232, 255]);
const GRID_COLOR: Rgba<u8> = Rgba([158, 174, 196, 255]);
const SILHOUETTE_COLOR: Rgba<u8> = Rgba([38, 48, 66, 255]);
const RANGE_COLOR: Rgba<u8> = Rgba([62, 74, 92, 180]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CorridorAxis {
    EastWest,
    NorthSouth,
}

#[derive(Debug, Clone, Copy)]
struct CorridorCandidate {
    start: AtlasCoord,
    end: AtlasCoord,
    axis: CorridorAxis,
    score: f32,
    length_cells: u32,
    ocean_score: f32,
    coast_score: f32,
    mountain_score: f32,
    macro_delta: f32,
}

#[derive(Debug, Clone, Copy)]
struct ChunkSample {
    coord: ChunkCoord,
    probe: new_world::world::ChunkGenerationProbe,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let seed = parse_required::<u64>(&mut args, "seed")?;
    let mut radius_cells = DEFAULT_RADIUS_CELLS;
    let mut min_length_cells = DEFAULT_MIN_LENGTH_CELLS;
    let mut max_length_cells = DEFAULT_MAX_LENGTH_CELLS;
    let mut output = PathBuf::from(DEFAULT_OUTPUT);

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--radius-cells" => radius_cells = parse_required::<i32>(&mut args, "radius-cells")?,
            "--min-length-cells" => {
                min_length_cells = parse_required::<u32>(&mut args, "min-length-cells")?
            }
            "--max-length-cells" => {
                max_length_cells = parse_required::<u32>(&mut args, "max-length-cells")?
            }
            "--output" => output = PathBuf::from(parse_required::<String>(&mut args, "output")?),
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    if radius_cells <= 0 {
        return Err(cli_error("radius-cells must be positive"));
    }
    if min_length_cells == 0 || min_length_cells > max_length_cells {
        return Err(cli_error("min-length-cells must be > 0 and <= max-length-cells"));
    }

    let meta = WorldMeta::new(seed);
    let area = AtlasArea::new(
        AtlasCoord::new(-radius_cells, -radius_cells),
        (radius_cells * 2 + 1) as u32,
        (radius_cells * 2 + 1) as u32,
    )
    .expect("corridor atlas area should be valid");
    let atlas = generate_atlas_fields(&meta, area);
    let candidate = find_best_corridor(&atlas, min_length_cells, max_length_cells)
        .ok_or_else(|| cli_error("no sea-to-mountain corridor was found in the requested atlas search area"))?;
    let samples = sample_corridor_chunks(candidate, &meta);
    let chart_path = output.join(format!(
        "seed_{seed}_corridor_{:?}_{},{}_to_{},{}.png",
        candidate.axis,
        candidate.start.x,
        candidate.start.z,
        candidate.end.x,
        candidate.end.z
    ));
    std::fs::create_dir_all(&output)?;
    write_corridor_chart(&chart_path, &samples)?;

    println!("seed: {seed}");
    println!(
        "atlas search area: origin=({}, {}), size={}x{} cells",
        area.origin().x,
        area.origin().z,
        area.width(),
        area.height()
    );
    println!(
        "best corridor: {:?} from atlas ({}, {}) to ({}, {}), length={} cells ({:.1}km), score={:.2}",
        candidate.axis,
        candidate.start.x,
        candidate.start.z,
        candidate.end.x,
        candidate.end.z,
        candidate.length_cells,
        candidate.length_cells as f32 * ATLAS_CELL_SIZE_M as f32 / 1000.0,
        candidate.score
    );
    println!(
        "signals: ocean={:.2}, coast={:.2}, mountain={:.2}, macro_delta={:.2}",
        candidate.ocean_score,
        candidate.coast_score,
        candidate.mountain_score,
        candidate.macro_delta
    );
    let first = samples.first().expect("corridor samples should exist");
    let last = samples.last().expect("corridor samples should exist");
    println!(
        "chunk span: ({}, {}) -> ({}, {}), {} samples, {}m per sample",
        first.coord.0,
        first.coord.2,
        last.coord.0,
        last.coord.2,
        samples.len(),
        ATLAS_CELL_SIZE_M as usize / ATLAS_CELL_SIZE_IN_CHUNKS as usize
    );
    println!(
        "surface summary: start mean={:.2} ({:.2}m), end mean={:.2} ({:.2}m), highest max_y={} ({:.1}m)",
        first.probe.surface_mean_y,
        first.probe.surface_mean_y * BLOCK_SIZE_M,
        last.probe.surface_mean_y,
        last.probe.surface_mean_y * BLOCK_SIZE_M,
        samples.iter().map(|sample| sample.probe.surface_max_y).max().unwrap_or(0),
        samples
            .iter()
            .map(|sample| sample.probe.surface_max_y)
            .max()
            .unwrap_or(0) as f32
            * BLOCK_SIZE_M
    );
    println!("chart: {}", chart_path.display());

    let midpoint = samples[samples.len() / 2].coord;
    println!(
        "suggested preview command: cargo run --bin chunk_preview -- {seed} --center-x {} --center-z {} --radius 12",
        midpoint.0, midpoint.2
    );

    Ok(())
}

fn find_best_corridor(
    atlas: &AtlasFieldMap,
    min_length_cells: u32,
    max_length_cells: u32,
) -> Option<CorridorCandidate> {
    let area = atlas.area();
    let mut best: Option<CorridorCandidate> = None;

    for length in min_length_cells..=max_length_cells {
        let span = length as i32 - 1;
        for z in area.origin().z..=(area.origin().z + area.height() as i32 - 1) {
            for x in area.origin().x..=(area.origin().x + area.width() as i32 - 1 - span) {
                let start = AtlasCoord::new(x, z);
                let end = AtlasCoord::new(x + span, z);
                if let Some(candidate) = evaluate_corridor(atlas, start, end, CorridorAxis::EastWest) {
                    if best.map(|current| candidate.score > current.score).unwrap_or(true) {
                        best = Some(candidate);
                    }
                }
            }
        }

        for x in area.origin().x..=(area.origin().x + area.width() as i32 - 1) {
            for z in area.origin().z..=(area.origin().z + area.height() as i32 - 1 - span) {
                let start = AtlasCoord::new(x, z);
                let end = AtlasCoord::new(x, z + span);
                if let Some(candidate) = evaluate_corridor(atlas, start, end, CorridorAxis::NorthSouth) {
                    if best.map(|current| candidate.score > current.score).unwrap_or(true) {
                        best = Some(candidate);
                    }
                }
            }
        }
    }

    best
}

fn evaluate_corridor(
    atlas: &AtlasFieldMap,
    a: AtlasCoord,
    b: AtlasCoord,
    axis: CorridorAxis,
) -> Option<CorridorCandidate> {
    let forward = corridor_cells(atlas, a, b, axis)?;
    let reverse = {
        let mut cells = forward.clone();
        cells.reverse();
        cells
    };

    corridor_score(a, b, axis, &forward)
        .or_else(|| corridor_score(b, a, axis, &reverse))
}

fn corridor_score(
    start: AtlasCoord,
    end: AtlasCoord,
    axis: CorridorAxis,
    cells: &[(AtlasCoord, AtlasCell)],
) -> Option<CorridorCandidate> {
    let first = cells.first()?.1;
    let last = cells.last()?.1;
    let ocean_score = ocean_like(first);
    let coast_score = cells
        .iter()
        .map(|(_, cell)| cell.coast_factor)
        .fold(0.0_f32, f32::max);
    let mountain_score = mountain_like(last);
    let macro_delta = last.macro_elevation - first.macro_elevation;

    if ocean_score < 0.72 || coast_score < 0.10 || mountain_score < 0.78 || macro_delta < 0.22 {
        return None;
    }

    let mut positive_gain = 0.0_f32;
    let mut negative_drop = 0.0_f32;
    let mut previous = first.macro_elevation;
    for (_, cell) in cells.iter().skip(1) {
        let delta = cell.macro_elevation - previous;
        if delta >= 0.0 {
            positive_gain += delta;
        } else {
            negative_drop += -delta;
        }
        previous = cell.macro_elevation;
    }

    let length_cells = cells.len() as u32;
    let score = ocean_score * 18.0
        + coast_score * 10.0
        + mountain_score * 26.0
        + macro_delta * 24.0
        + positive_gain * 18.0
        - negative_drop * 28.0;

    Some(CorridorCandidate {
        start,
        end,
        axis,
        score,
        length_cells,
        ocean_score,
        coast_score,
        mountain_score,
        macro_delta,
    })
}

fn corridor_cells(
    atlas: &AtlasFieldMap,
    start: AtlasCoord,
    end: AtlasCoord,
    axis: CorridorAxis,
) -> Option<Vec<(AtlasCoord, AtlasCell)>> {
    let mut cells = Vec::new();
    match axis {
        CorridorAxis::EastWest => {
            for x in start.x..=end.x {
                let coord = AtlasCoord::new(x, start.z);
                cells.push((coord, *atlas.get(coord)?));
            }
        }
        CorridorAxis::NorthSouth => {
            for z in start.z..=end.z {
                let coord = AtlasCoord::new(start.x, z);
                cells.push((coord, *atlas.get(coord)?));
            }
        }
    }
    Some(cells)
}

fn ocean_like(cell: AtlasCell) -> f32 {
    (cell.overlay.ocean * 0.70 + (1.0 - cell.landness) * 0.30).clamp(0.0, 1.0)
}

fn mountain_like(cell: AtlasCell) -> f32 {
    (cell.mountain_mass * 0.55 + cell.alpine_factor * 0.25 + cell.form.mountain * 0.20).clamp(0.0, 1.0)
}

fn sample_corridor_chunks(
    corridor: CorridorCandidate,
    meta: &WorldMeta,
) -> Vec<ChunkSample> {
    let mut samples = Vec::new();
    match corridor.axis {
        CorridorAxis::EastWest => {
            let z = corridor.start.z * ATLAS_CELL_SIZE_IN_CHUNKS as i32 + ATLAS_CELL_SIZE_IN_CHUNKS as i32 / 2;
            let start_x = corridor.start.x * ATLAS_CELL_SIZE_IN_CHUNKS as i32;
            let end_x = if corridor.end.x >= corridor.start.x {
                (corridor.end.x + 1) * ATLAS_CELL_SIZE_IN_CHUNKS as i32 - 1
            } else {
                corridor.end.x * ATLAS_CELL_SIZE_IN_CHUNKS as i32
            };
            let step = if end_x >= start_x { 1 } else { -1 };
            let mut chunk_x = start_x;
            loop {
                let coord = ChunkCoord(chunk_x, 0, z);
                samples.push(ChunkSample {
                    coord,
                    probe: probe_chunk(coord, meta),
                });
                if chunk_x == end_x {
                    break;
                }
                chunk_x += step;
            }
        }
        CorridorAxis::NorthSouth => {
            let x = corridor.start.x * ATLAS_CELL_SIZE_IN_CHUNKS as i32 + ATLAS_CELL_SIZE_IN_CHUNKS as i32 / 2;
            let start_z = corridor.start.z * ATLAS_CELL_SIZE_IN_CHUNKS as i32;
            let end_z = if corridor.end.z >= corridor.start.z {
                (corridor.end.z + 1) * ATLAS_CELL_SIZE_IN_CHUNKS as i32 - 1
            } else {
                corridor.end.z * ATLAS_CELL_SIZE_IN_CHUNKS as i32
            };
            let step = if end_z >= start_z { 1 } else { -1 };
            let mut chunk_z = start_z;
            loop {
                let coord = ChunkCoord(x, 0, chunk_z);
                samples.push(ChunkSample {
                    coord,
                    probe: probe_chunk(coord, meta),
                });
                if chunk_z == end_z {
                    break;
                }
                chunk_z += step;
            }
        }
    }
    samples
}

fn write_corridor_chart(path: &std::path::Path, samples: &[ChunkSample]) -> Result<(), Box<dyn Error>> {
    let width = (samples.len() as u32 * SAMPLE_PIXELS_PER_CHUNK).max(1) + CHART_MARGIN * 2;
    let height = CHART_HEIGHT;
    let mut image = ImageBuffer::from_pixel(width, height, SKY_COLOR);
    let min_y = samples
        .iter()
        .map(|sample| sample.probe.surface_min_y)
        .min()
        .unwrap_or(0)
        .min(-16);
    let max_y = samples
        .iter()
        .map(|sample| sample.probe.surface_max_y)
        .max()
        .unwrap_or(0)
        .max(8);
    let y_span = (max_y - min_y).max(1) as f32;
    let chart_bottom = height - CHART_MARGIN - 1;
    let chart_top = CHART_MARGIN;

    for world_y in [0, min_y, max_y] {
        let y = map_y(world_y as f32, min_y, max_y, chart_top, chart_bottom);
        if y >= chart_top && y <= chart_bottom {
            let color = if world_y == 0 { SEA_LEVEL_COLOR } else { GRID_COLOR };
            for x in CHART_MARGIN..(width - CHART_MARGIN) {
                image.put_pixel(x, y, color);
            }
        }
    }

    for (index, sample) in samples.iter().enumerate() {
        let x0 = CHART_MARGIN + index as u32 * SAMPLE_PIXELS_PER_CHUNK;
        let x1 = (x0 + SAMPLE_PIXELS_PER_CHUNK).min(width - CHART_MARGIN);
        let tint = profile_color(sample.probe.dominant_profile);
        let mean_y = sample.probe.surface_mean_y;
        let top = map_y(mean_y, min_y, max_y, chart_top, chart_bottom);
        let min_sample_y = map_y(sample.probe.surface_min_y as f32, min_y, max_y, chart_top, chart_bottom);
        let max_sample_y = map_y(sample.probe.surface_max_y as f32, min_y, max_y, chart_top, chart_bottom);

        for x in x0..x1 {
            for y in top..=chart_bottom {
                let pixel = blend(image.get_pixel(x, y), tint, 0.72);
                image.put_pixel(x, y, pixel);
            }
            for y in max_sample_y..=min_sample_y {
                let pixel = blend(image.get_pixel(x, y), RANGE_COLOR, 0.55);
                image.put_pixel(x, y, pixel);
            }
            image.put_pixel(x, top, SILHOUETTE_COLOR);
        }
    }

    image.save(path)?;
    let _ = y_span;
    Ok(())
}

fn map_y(value: f32, min_y: i32, max_y: i32, chart_top: u32, chart_bottom: u32) -> u32 {
    let span = (max_y - min_y).max(1) as f32;
    let t = (value - min_y as f32) / span;
    let y = chart_bottom as f32 - t.clamp(0.0, 1.0) * (chart_bottom - chart_top) as f32;
    y.round() as u32
}

fn profile_color(profile: TerrainProfile) -> Rgba<u8> {
    match profile {
        TerrainProfile::DeepOcean => Rgba([26, 74, 138, 255]),
        TerrainProfile::Shelf => Rgba([74, 126, 186, 255]),
        TerrainProfile::Coast => Rgba([210, 196, 142, 255]),
        TerrainProfile::Plain => Rgba([102, 148, 88, 255]),
        TerrainProfile::Upland => Rgba([122, 130, 92, 255]),
        TerrainProfile::Ridge => Rgba([110, 110, 118, 255]),
    }
}

fn blend(base: &Rgba<u8>, overlay: Rgba<u8>, alpha: f32) -> Rgba<u8> {
    let alpha = alpha.clamp(0.0, 1.0);
    let inv = 1.0 - alpha;
    let mut out = [0_u8; 4];
    for index in 0..4 {
        out[index] = (base[index] as f32 * inv + overlay[index] as f32 * alpha)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    Rgba(out)
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
    "usage: cargo run --bin terrain_corridor -- <seed> [--radius-cells <i32>] [--min-length-cells <u32>] [--max-length-cells <u32>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mountain_like_prefers_mountain_mass_and_alpine() {
        let low = AtlasCell::default();
        let high = AtlasCell {
            mountain_mass: 0.9,
            alpine_factor: 0.7,
            ..AtlasCell::default()
        };

        assert!(mountain_like(high) > mountain_like(low));
    }

    #[test]
    fn ocean_like_prefers_ocean_overlay_and_low_landness() {
        let land = AtlasCell {
            landness: 0.8,
            ..AtlasCell::default()
        };
        let ocean = AtlasCell {
            landness: 0.1,
            ..AtlasCell::default()
        };

        assert!(ocean_like(ocean) > ocean_like(land));
    }
}
