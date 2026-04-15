use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::sync::Arc;

use image::{Rgb, RgbImage};

use new_world::world::{
    CreatedWorldManifest, CreatedWorldSource, BlockId, BlockMaterialKind, BlockRegistry,
    CHUNK_EDGE_I32, ChunkCoord, WORLD_FLOOR_Y, WorldBlockCoord, WorldCore, WorldMeta,
    build_chunk_mesh, generate_chunk,
};

const DEFAULT_CENTER_X: i32 = 0;
const DEFAULT_CENTER_Z: i32 = 0;
const DEFAULT_RADIUS: i32 = 0;
const DEFAULT_MIN_Y_CHUNK: i32 = -2;
const DEFAULT_MAX_Y_CHUNK: i32 = 3;
const DEFAULT_PIXELS_PER_BLOCK: u32 = 6;

#[derive(Debug, Clone)]
enum PreviewSource {
    Seed(u64),
    CreatedWorld(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TopdownCell {
    top_y: Option<i32>,
    block: BlockId,
}

impl TopdownCell {
    const AIR: Self = Self {
        top_y: None,
        block: BlockId::AIR,
    };
}

#[derive(Debug, Clone, Copy)]
struct SurfaceRange {
    min_y: i32,
    max_y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ColumnScan {
    visible: TopdownCell,
    top_solid: TopdownCell,
    top_water_y: Option<i32>,
    water_block_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BlockCount {
    key: String,
    count: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct PreviewDebugSummary {
    total_columns: usize,
    columns_with_any_water: usize,
    columns_with_top_water: usize,
    columns_with_hidden_water: usize,
    total_water_blocks: usize,
    average_water_depth: f32,
    max_water_depth: u16,
    top_visible_blocks: Vec<BlockCount>,
    top_solid_blocks: Vec<BlockCount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeshDebugSummary {
    chunk_count: usize,
    meshed_chunk_count: usize,
    total_face_count: usize,
    water_face_count: usize,
    chunks_with_water_faces: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let source = if args.first().map(String::as_str) == Some("--world-dir") {
        args.remove(0);
        PreviewSource::CreatedWorld(PathBuf::from(parse_required::<String>(&mut args, "world-dir")?))
    } else {
        PreviewSource::Seed(parse_required::<u64>(&mut args, "seed")?)
    };

    let mut center_x = DEFAULT_CENTER_X;
    let mut center_z = DEFAULT_CENTER_Z;
    let mut center_explicit = false;
    let mut radius = DEFAULT_RADIUS;
    let mut min_y_chunk = DEFAULT_MIN_Y_CHUNK;
    let mut max_y_chunk = DEFAULT_MAX_Y_CHUNK;
    let mut pixels_per_block = DEFAULT_PIXELS_PER_BLOCK;
    let mut output: Option<PathBuf> = None;

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--center-x" | "--chunk-x" => {
                let label = if flag == "--chunk-x" {
                    "chunk-x"
                } else {
                    "center-x"
                };
                center_x = parse_required::<i32>(&mut args, label)?;
                center_explicit = true;
            }
            "--center-z" | "--chunk-z" => {
                let label = if flag == "--chunk-z" {
                    "chunk-z"
                } else {
                    "center-z"
                };
                center_z = parse_required::<i32>(&mut args, label)?;
                center_explicit = true;
            }
            "--radius" => radius = parse_required::<i32>(&mut args, "radius")?,
            "--min-y-chunk" => min_y_chunk = parse_required::<i32>(&mut args, "min-y-chunk")?,
            "--max-y-chunk" => max_y_chunk = parse_required::<i32>(&mut args, "max-y-chunk")?,
            "--pixels-per-block" => {
                pixels_per_block = parse_required::<u32>(&mut args, "pixels-per-block")?
            }
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
    if pixels_per_block == 0 {
        return Err(cli_error("pixels-per-block must be >= 1"));
    }

    let block_registry = Arc::new(
        BlockRegistry::load_default()
            .map_err(|error| cli_error(format!("failed to load block registry: {error:?}")))?,
    );

    let (meta, created_world_source) = match &source {
        PreviewSource::Seed(seed) => (WorldMeta::new(*seed), None),
        PreviewSource::CreatedWorld(world_dir) => {
            let created_world_source = CreatedWorldSource::open(world_dir).map_err(|error| {
                cli_error(format!(
                    "failed to open created world {}: {error}",
                    world_dir.display()
                ))
            })?;
            (
                created_world_source.manifest().world_meta(),
                Some(created_world_source),
            )
        }
    };

    let requested_center = if center_explicit {
        (center_x, center_z)
    } else if let Some(created_world_source) = created_world_source.as_ref() {
        choose_created_world_center(
            created_world_source.manifest(),
            radius,
            min_y_chunk,
            max_y_chunk,
        )?
    } else {
        (center_x, center_z)
    };
    center_x = requested_center.0;
    center_z = requested_center.1;

    let output =
        output.unwrap_or_else(|| default_output_path(&source, center_x, center_z, radius));
    let mut world = WorldCore::new(meta, Arc::clone(&block_registry));

    match created_world_source.as_ref() {
        Some(source) => {
            ensure_created_world_bounds_cover_request(
                source.manifest().min_chunk_coord(),
                source.manifest().max_chunk_coord(),
                center_x,
                center_z,
                radius,
                min_y_chunk,
                max_y_chunk,
            )?;
            load_preview_chunks(&mut world, source, center_x, center_z, radius, min_y_chunk, max_y_chunk)?;
        }
        None => generate_preview_chunks(
            &mut world,
            block_registry.as_ref(),
            center_x,
            center_z,
            radius,
            min_y_chunk,
            max_y_chunk,
        ),
    }

    let (image, surface_range, debug_summary) = render_topdown_preview(
        &world,
        block_registry.as_ref(),
        center_x,
        center_z,
        radius,
        min_y_chunk,
        max_y_chunk,
        pixels_per_block,
    )?;
    let mesh_debug = collect_mesh_debug_summary(
        &world,
        block_registry.as_ref(),
        center_x,
        center_z,
        radius,
        min_y_chunk,
        max_y_chunk,
    );
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.save(&output)?;

    match &source {
        PreviewSource::Seed(seed) => println!("preview source: generated from seed {seed}"),
        PreviewSource::CreatedWorld(world_dir) => {
            println!("preview source: created world {}", world_dir.display())
        }
    }
    println!("center chunk: ({center_x}, {center_z})");
    println!(
        "footprint: xz radius={}, y={}..{}, pixels-per-block={}",
        radius, min_y_chunk, max_y_chunk, pixels_per_block
    );
    println!(
        "surface relief: {}..{}",
        surface_range.min_y, surface_range.max_y
    );
    print_preview_debug_summary(&debug_summary);
    print_mesh_debug_summary(mesh_debug);
    println!(
        "water diagnostic: {}",
        diagnose_water_visibility(&debug_summary, mesh_debug)
    );
    println!("output: {}", output.display());
    println!("image: {}x{}", image.width(), image.height());

    Ok(())
}

fn render_topdown_preview(
    world: &WorldCore,
    registry: &BlockRegistry,
    center_x: i32,
    center_z: i32,
    radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
    pixels_per_block: u32,
) -> Result<(RgbImage, SurfaceRange, PreviewDebugSummary), Box<dyn Error>> {
    let chunk_span = u32::try_from(radius.saturating_mul(2).saturating_add(1))
        .map_err(|_| cli_error("radius produced an invalid chunk span"))?;
    let blocks_per_axis = chunk_span
        .checked_mul(CHUNK_EDGE_I32 as u32)
        .ok_or_else(|| cli_error("preview block width overflowed"))?;
    let width = blocks_per_axis
        .checked_mul(pixels_per_block)
        .ok_or_else(|| cli_error("preview image width overflowed"))?;
    let height = blocks_per_axis
        .checked_mul(pixels_per_block)
        .ok_or_else(|| cli_error("preview image height overflowed"))?;

    let min_world_x = (center_x - radius) * CHUNK_EDGE_I32;
    let min_world_z = (center_z - radius) * CHUNK_EDGE_I32;
    let min_world_y = (min_y_chunk * CHUNK_EDGE_I32).max(WORLD_FLOOR_Y);
    let max_world_y = (max_y_chunk + 1) * CHUNK_EDGE_I32 - 1;
    if max_world_y < min_world_y {
        return Err(cli_error(format!(
            "requested vertical window resolves to an empty world-y range: {}..{}",
            min_world_y, max_world_y
        )));
    }

    let grid_width = usize::try_from(blocks_per_axis).map_err(|_| cli_error("grid width overflowed"))?;
    let grid_height = usize::try_from(blocks_per_axis).map_err(|_| cli_error("grid height overflowed"))?;
    let mut cells = Vec::with_capacity(grid_width * grid_height);

    for z_offset in 0..grid_height {
        let world_z = min_world_z + i32::try_from(z_offset).expect("grid z index should fit in i32");
        for x_offset in 0..grid_width {
            let world_x = min_world_x + i32::try_from(x_offset).expect("grid x index should fit in i32");
            cells.push(sample_column_scan(
                world,
                registry,
                world_x,
                world_z,
                min_world_y,
                max_world_y,
            ));
        }
    }

    let surface_range = surface_range_for_cells(&cells).ok_or_else(|| {
        cli_error(format!(
            "no non-air blocks were found inside the requested area x={}..{}, y={}..{}, z={}..{}",
            center_x - radius,
            center_x + radius,
            min_y_chunk,
            max_y_chunk,
            center_z - radius,
            center_z + radius
        ))
    })?;
    let debug_summary = summarize_column_scans(&cells, registry);

    let mut image = RgbImage::new(width, height);
    for z in 0..grid_height {
        for x in 0..grid_width {
            let cell = cells[z * grid_width + x].visible;
            let base = color_for_cell(cell, registry, surface_range);
            let pixel_origin_x =
                u32::try_from(x).expect("grid x index should fit in u32") * pixels_per_block;
            let pixel_origin_y =
                u32::try_from(z).expect("grid z index should fit in u32") * pixels_per_block;

            for local_y in 0..pixels_per_block {
                for local_x in 0..pixels_per_block {
                    let outline = outline_strength(
                        &cells,
                        grid_width,
                        grid_height,
                        x,
                        z,
                        local_x,
                        local_y,
                        pixels_per_block,
                    );
                    image.put_pixel(
                        pixel_origin_x + local_x,
                        pixel_origin_y + local_y,
                        Rgb(darken(base, outline)),
                    );
                }
            }
        }
    }

    Ok((image, surface_range, debug_summary))
}

fn sample_column_scan(
    world: &WorldCore,
    registry: &BlockRegistry,
    world_x: i32,
    world_z: i32,
    min_world_y: i32,
    max_world_y: i32,
) -> ColumnScan {
    let mut visible = TopdownCell::AIR;
    let mut top_solid = TopdownCell::AIR;
    let mut top_water_y = None;
    let mut water_block_count = 0_u16;

    for world_y in (min_world_y..=max_world_y).rev() {
        let Some(block) = world.get_block(WorldBlockCoord(world_x, world_y, world_z)) else {
            continue;
        };
        if block.is_air() {
            continue;
        }

        let block_def = registry.block_or_missing(block);
        if visible.top_y.is_none() {
            visible = TopdownCell {
                top_y: Some(world_y),
                block,
            };
        }
        if top_solid.top_y.is_none() && block_def.solid {
            top_solid = TopdownCell {
                top_y: Some(world_y),
                block,
            };
        }
        if block_def.key == "water" {
            if top_water_y.is_none() {
                top_water_y = Some(world_y);
            }
            water_block_count = water_block_count.saturating_add(1);
        }
    }

    ColumnScan {
        visible,
        top_solid,
        top_water_y,
        water_block_count,
    }
}

fn surface_range_for_cells(cells: &[ColumnScan]) -> Option<SurfaceRange> {
    let mut top_cells = cells.iter().filter_map(|cell| cell.visible.top_y);
    let first = top_cells.next()?;
    let mut min_y = first;
    let mut max_y = first;

    for top_y in top_cells {
        min_y = min_y.min(top_y);
        max_y = max_y.max(top_y);
    }

    Some(SurfaceRange { min_y, max_y })
}

fn summarize_column_scans(cells: &[ColumnScan], registry: &BlockRegistry) -> PreviewDebugSummary {
    let mut columns_with_any_water = 0_usize;
    let mut columns_with_top_water = 0_usize;
    let mut columns_with_hidden_water = 0_usize;
    let mut total_water_blocks = 0_usize;
    let mut max_water_depth = 0_u16;
    let mut top_visible = Vec::<BlockCount>::new();
    let mut top_solid = Vec::<BlockCount>::new();

    for cell in cells {
        let has_water = cell.water_block_count > 0;
        if has_water {
            columns_with_any_water += 1;
            total_water_blocks += usize::from(cell.water_block_count);
            max_water_depth = max_water_depth.max(cell.water_block_count);
        }

        if cell.top_water_y == cell.visible.top_y && cell.top_water_y.is_some() {
            columns_with_top_water += 1;
        } else if has_water {
            columns_with_hidden_water += 1;
        }

        if let Some(key) = block_key_for_cell(cell.visible, registry) {
            increment_block_count(&mut top_visible, key);
        }
        if let Some(key) = block_key_for_cell(cell.top_solid, registry) {
            increment_block_count(&mut top_solid, key);
        }
    }

    sort_block_counts(&mut top_visible);
    sort_block_counts(&mut top_solid);

    let average_water_depth = if columns_with_any_water > 0 {
        total_water_blocks as f32 / columns_with_any_water as f32
    } else {
        0.0
    };

    PreviewDebugSummary {
        total_columns: cells.len(),
        columns_with_any_water,
        columns_with_top_water,
        columns_with_hidden_water,
        total_water_blocks,
        average_water_depth,
        max_water_depth,
        top_visible_blocks: top_visible,
        top_solid_blocks: top_solid,
    }
}

fn collect_mesh_debug_summary(
    world: &WorldCore,
    registry: &BlockRegistry,
    center_x: i32,
    center_z: i32,
    radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
) -> MeshDebugSummary {
    let mut summary = MeshDebugSummary {
        chunk_count: 0,
        meshed_chunk_count: 0,
        total_face_count: 0,
        water_face_count: 0,
        chunks_with_water_faces: 0,
    };

    for chunk_y in min_y_chunk..=max_y_chunk {
        for chunk_z in (center_z - radius)..=(center_z + radius) {
            for chunk_x in (center_x - radius)..=(center_x + radius) {
                let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
                let Some(snapshot) = world.snapshot_chunk(coord) else {
                    continue;
                };
                summary.chunk_count += 1;
                let mesh = build_chunk_mesh(&snapshot, world.query_neighbors(coord), registry);
                if mesh.is_empty() {
                    continue;
                }

                summary.meshed_chunk_count += 1;
                let face_count = mesh.vertices.len() / 4;
                let water_face_count = mesh
                    .vertices
                    .chunks_exact(4)
                    .filter(|face| face[0].material_kind == BlockMaterialKind::Water)
                    .count();
                summary.total_face_count += face_count;
                summary.water_face_count += water_face_count;
                if water_face_count > 0 {
                    summary.chunks_with_water_faces += 1;
                }
            }
        }
    }

    summary
}

fn color_for_cell(cell: TopdownCell, registry: &BlockRegistry, surface_range: SurfaceRange) -> [u8; 3] {
    let Some(top_y) = cell.top_y else {
        return [18, 22, 28];
    };

    let def = registry.block_or_missing(cell.block);
    let base = block_base_color(def);
    let tint = def.tint;
    let modulated = [
        ((u16::from(base[0]) * u16::from(tint[0])) / 255) as u8,
        ((u16::from(base[1]) * u16::from(tint[1])) / 255) as u8,
        ((u16::from(base[2]) * u16::from(tint[2])) / 255) as u8,
    ];

    let relief_t = if surface_range.max_y > surface_range.min_y {
        (top_y - surface_range.min_y) as f32 / (surface_range.max_y - surface_range.min_y) as f32
    } else {
        0.5
    };
    let brightness = match def.material {
        BlockMaterialKind::Water => 0.88 + relief_t * 0.16,
        BlockMaterialKind::Emissive => 0.98 + relief_t * 0.08,
        _ => 0.82 + relief_t * 0.22,
    };

    brighten(modulated, brightness)
}

fn block_base_color(def: &new_world::world::BlockDef) -> [u8; 3] {
    match def.key.as_str() {
        // Snow keeps a dedicated override so preview diagnostics do not read as gray stone.
        "snow" => [244, 248, 255],
        _ => material_base_color(def.material),
    }
}

fn material_base_color(material: BlockMaterialKind) -> [u8; 3] {
    match material {
        BlockMaterialKind::GenericOpaque => [170, 170, 178],
        BlockMaterialKind::Grass => [110, 162, 82],
        BlockMaterialKind::Soil => [122, 90, 60],
        BlockMaterialKind::Stone => [138, 144, 152],
        BlockMaterialKind::Sand => [208, 190, 126],
        BlockMaterialKind::Foliage => [86, 150, 98],
        BlockMaterialKind::Water => [76, 124, 198],
        BlockMaterialKind::Emissive => [236, 194, 88],
    }
}

fn outline_strength(
    cells: &[ColumnScan],
    width: usize,
    height: usize,
    x: usize,
    z: usize,
    local_x: u32,
    local_y: u32,
    pixels_per_block: u32,
) -> f32 {
    if pixels_per_block <= 1 {
        return 0.0;
    }

    let index = z * width + x;
    let cell = cells[index].visible;
    let mut strength = 0.0_f32;

    if local_x == 0 {
        strength = strength.max(edge_strength(
            cell,
            x.checked_sub(1).map(|nx| cells[z * width + nx].visible),
        ));
    }
    if local_y == 0 {
        strength = strength.max(edge_strength(
            cell,
            z.checked_sub(1).map(|nz| cells[nz * width + x].visible),
        ));
    }
    if local_x + 1 == pixels_per_block {
        let right = if x + 1 < width {
            Some(cells[z * width + (x + 1)].visible)
        } else {
            None
        };
        strength = strength.max(edge_strength(cell, right));
    }
    if local_y + 1 == pixels_per_block {
        let bottom = if z + 1 < height {
            Some(cells[(z + 1) * width + x].visible)
        } else {
            None
        };
        strength = strength.max(edge_strength(cell, bottom));
    }

    strength
}

fn edge_strength(cell: TopdownCell, neighbor: Option<TopdownCell>) -> f32 {
    match neighbor {
        None => 0.24,
        Some(other) if other == cell => 0.10,
        Some(other) if other.top_y == cell.top_y => 0.14,
        Some(_) => 0.24,
    }
}

fn brighten(color: [u8; 3], factor: f32) -> [u8; 3] {
    [
        scale_channel(color[0], factor),
        scale_channel(color[1], factor),
        scale_channel(color[2], factor),
    ]
}

fn darken(color: [u8; 3], amount: f32) -> [u8; 3] {
    let factor = (1.0 - amount).clamp(0.0, 1.0);
    brighten(color, factor)
}

fn scale_channel(channel: u8, factor: f32) -> u8 {
    (channel as f32 * factor).round().clamp(0.0, 255.0) as u8
}

fn block_key_for_cell(cell: TopdownCell, registry: &BlockRegistry) -> Option<String> {
    cell.top_y
        .map(|_| registry.block_or_missing(cell.block).key.clone())
}

fn increment_block_count(counts: &mut Vec<BlockCount>, key: String) {
    if let Some(existing) = counts.iter_mut().find(|count| count.key == key) {
        existing.count += 1;
    } else {
        counts.push(BlockCount { key, count: 1 });
    }
}

fn sort_block_counts(counts: &mut [BlockCount]) {
    counts.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
}

fn print_preview_debug_summary(summary: &PreviewDebugSummary) {
    println!("column debug:");
    println!("  total columns: {}", summary.total_columns);
    println!(
        "  columns with any water: {} ({:.1}%)",
        summary.columns_with_any_water,
        percent(summary.columns_with_any_water, summary.total_columns)
    );
    println!(
        "  columns with top-visible water: {} ({:.1}%)",
        summary.columns_with_top_water,
        percent(summary.columns_with_top_water, summary.total_columns)
    );
    println!(
        "  columns with hidden water: {} ({:.1}%)",
        summary.columns_with_hidden_water,
        percent(summary.columns_with_hidden_water, summary.total_columns)
    );
    println!("  total water blocks in scanned volume: {}", summary.total_water_blocks);
    println!(
        "  water depth per wet column: avg {:.2}, max {}",
        summary.average_water_depth, summary.max_water_depth
    );
    print_block_counts("  top visible blocks:", &summary.top_visible_blocks);
    print_block_counts("  top solid blocks:", &summary.top_solid_blocks);
}

fn print_mesh_debug_summary(summary: MeshDebugSummary) {
    println!("mesh debug:");
    println!("  loaded chunks in preview: {}", summary.chunk_count);
    println!("  non-empty chunk meshes: {}", summary.meshed_chunk_count);
    println!("  total emitted faces: {}", summary.total_face_count);
    println!(
        "  water faces: {} ({:.1}%), chunks with water faces: {}",
        summary.water_face_count,
        percent(summary.water_face_count, summary.total_face_count),
        summary.chunks_with_water_faces
    );
}

fn print_block_counts(label: &str, counts: &[BlockCount]) {
    println!("{label}");
    for count in counts.iter().take(8) {
        println!("    {}: {}", count.key, count.count);
    }
}

fn percent(part: usize, whole: usize) -> f32 {
    if whole == 0 {
        0.0
    } else {
        (part as f32 / whole as f32) * 100.0
    }
}

fn diagnose_water_visibility(summary: &PreviewDebugSummary, mesh_debug: MeshDebugSummary) -> &'static str {
    if summary.columns_with_any_water == 0 {
        "no water blocks were realized in the scanned chunk volume, so this preview points to generation rather than renderer visibility"
    } else if mesh_debug.water_face_count == 0 {
        "water blocks exist in chunk data but produced no water faces in meshing, so inspect world meshing or block render-kind/opacity rules"
    } else if summary.columns_with_top_water == 0 {
        "water exists and survives meshing, but it is not the topmost non-air block in this top-down projection"
    } else {
        "water exists in realized chunk data and in the generated chunk meshes; if it is still invisible in the live game view, the remaining suspect is renderer presentation rather than chunk generation"
    }
}

fn generate_preview_chunks(
    world: &mut WorldCore,
    registry: &BlockRegistry,
    center_x: i32,
    center_z: i32,
    radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
) {
    for chunk_y in min_y_chunk..=max_y_chunk {
        for chunk_z in (center_z - radius)..=(center_z + radius) {
            for chunk_x in (center_x - radius)..=(center_x + radius) {
                let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
                let chunk = generate_chunk(coord, world.meta(), registry);
                world.insert_chunk(coord, chunk);
            }
        }
    }
}

fn load_preview_chunks(
    world: &mut WorldCore,
    source: &CreatedWorldSource,
    center_x: i32,
    center_z: i32,
    radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
) -> Result<(), Box<dyn Error>> {
    for chunk_y in min_y_chunk..=max_y_chunk {
        for chunk_z in (center_z - radius)..=(center_z + radius) {
            for chunk_x in (center_x - radius)..=(center_x + radius) {
                let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
                let chunk = source.load_chunk(coord).map_err(|error| {
                    cli_error(format!(
                        "failed to load created-world chunk ({}, {}, {}): {error}",
                        coord.0, coord.1, coord.2
                    ))
                })?;
                world.insert_chunk(coord, chunk);
            }
        }
    }

    Ok(())
}

fn ensure_created_world_bounds_cover_request(
    min_chunk: ChunkCoord,
    max_chunk: ChunkCoord,
    center_x: i32,
    center_z: i32,
    radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
) -> Result<(), Box<dyn Error>> {
    let requested_min = ChunkCoord(center_x - radius, min_y_chunk, center_z - radius);
    let requested_max = ChunkCoord(center_x + radius, max_y_chunk, center_z + radius);
    if requested_min.0 < min_chunk.0
        || requested_min.1 < min_chunk.1
        || requested_min.2 < min_chunk.2
        || requested_max.0 > max_chunk.0
        || requested_max.1 > max_chunk.1
        || requested_max.2 > max_chunk.2
    {
        return Err(cli_error(format!(
            "requested preview area x={}..{}, y={}..{}, z={}..{} is outside created-world bounds x={}..{}, y={}..{}, z={}..{}",
            requested_min.0,
            requested_max.0,
            requested_min.1,
            requested_max.1,
            requested_min.2,
            requested_max.2,
            min_chunk.0,
            max_chunk.0,
            min_chunk.1,
            max_chunk.1,
            min_chunk.2,
            max_chunk.2
        )));
    }

    Ok(())
}

fn choose_created_world_center(
    manifest: &CreatedWorldManifest,
    radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
) -> Result<(i32, i32), Box<dyn Error>> {
    let min_chunk = manifest.min_chunk_coord();
    let max_chunk = manifest.max_chunk_coord();

    for summary in &manifest.stacks {
        let requested_min = ChunkCoord(summary.center_x - radius, min_y_chunk, summary.center_z - radius);
        let requested_max = ChunkCoord(summary.center_x + radius, max_y_chunk, summary.center_z + radius);
        if requested_min.0 >= min_chunk.0
            && requested_min.1 >= min_chunk.1
            && requested_min.2 >= min_chunk.2
            && requested_max.0 <= max_chunk.0
            && requested_max.1 <= max_chunk.1
            && requested_max.2 <= max_chunk.2
        {
            return Ok((summary.center_x, summary.center_z));
        }
    }

    Err(cli_error(format!(
        "no created-world preview center fits radius {} inside created-world bounds x={}..{}, y={}..{}, z={}..{}; try a smaller radius or pass --center-x/--center-z",
        radius,
        min_chunk.0,
        max_chunk.0,
        min_chunk.1,
        max_chunk.1,
        min_chunk.2,
        max_chunk.2
    )))
}

fn default_output_path(source: &PreviewSource, center_x: i32, center_z: i32, radius: i32) -> PathBuf {
    match source {
        PreviewSource::Seed(seed) => PathBuf::from(format!(
            "target/chunk-topdown-preview/seed_{seed}_cx{center_x}_cz{center_z}_r{radius}.png"
        )),
        PreviewSource::CreatedWorld(world_dir) => {
            world_dir.join(format!("topdown_cx{center_x}_cz{center_z}_r{radius}.png"))
        }
    }
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
    "usage: cargo run --bin chunk_topdown_preview -- <seed> [--center-x <i32> | --chunk-x <i32>] [--center-z <i32> | --chunk-z <i32>] [--radius <i32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--pixels-per-block <u32>] [--output <path>]\n       cargo run --bin chunk_topdown_preview -- --world-dir <path> [--center-x <i32> | --chunk-x <i32>] [--center-z <i32> | --chunk-z <i32>] [--radius <i32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--pixels-per-block <u32>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_count(key: &str, count: usize) -> BlockCount {
        BlockCount {
            key: key.to_string(),
            count,
        }
    }

    #[test]
    fn water_diagnostic_points_at_generation_when_no_water_exists() {
        let summary = PreviewDebugSummary {
            total_columns: 64,
            columns_with_any_water: 0,
            columns_with_top_water: 0,
            columns_with_hidden_water: 0,
            total_water_blocks: 0,
            average_water_depth: 0.0,
            max_water_depth: 0,
            top_visible_blocks: vec![block_count("stone", 64)],
            top_solid_blocks: vec![block_count("stone", 64)],
        };
        let mesh = MeshDebugSummary {
            chunk_count: 1,
            meshed_chunk_count: 1,
            total_face_count: 128,
            water_face_count: 0,
            chunks_with_water_faces: 0,
        };

        assert!(diagnose_water_visibility(&summary, mesh).contains("generation"));
    }

    #[test]
    fn water_diagnostic_points_at_renderer_when_water_is_visible_and_meshed() {
        let summary = PreviewDebugSummary {
            total_columns: 64,
            columns_with_any_water: 12,
            columns_with_top_water: 12,
            columns_with_hidden_water: 0,
            total_water_blocks: 18,
            average_water_depth: 1.5,
            max_water_depth: 3,
            top_visible_blocks: vec![block_count("water", 12), block_count("sand", 52)],
            top_solid_blocks: vec![block_count("sand", 64)],
        };
        let mesh = MeshDebugSummary {
            chunk_count: 1,
            meshed_chunk_count: 1,
            total_face_count: 180,
            water_face_count: 24,
            chunks_with_water_faces: 1,
        };

        assert!(diagnose_water_visibility(&summary, mesh).contains("renderer"));
    }

    #[test]
    fn snow_preview_color_uses_white_override() {
        let registry = BlockRegistry::load_default().expect("default registry should load");
        let snow = registry
            .block(registry.block_id("snow").expect("snow block should exist"))
            .expect("snow def should exist");

        assert_eq!(block_base_color(snow), [244, 248, 255]);
    }
}
