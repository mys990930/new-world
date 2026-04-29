use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::{Rgb, RgbImage};
use rayon::prelude::*;

use new_world::world::{
    BlockId, BlockMaterialKind, BlockRegistry, CHUNK_EDGE, CHUNK_EDGE_I32, ChunkCoord,
    ChunkGenerationInputs, HydrologyMode, MaterialPolicyId, RegionArchetype, WORLD_FLOOR_Y,
    WorldBlockCoord, WorldCore, WorldMeta, build_chunk_base_heightfield_prototype,
    build_chunk_corridor_window, build_chunk_hydrology_solve, build_chunk_mesh,
    build_chunk_meso_applied_prototype, build_chunk_realization_field_patch,
    build_chunk_smoothed_prototype, chunk_generation_input_area,
    generate_chunk_from_generation_inputs, prepare_chunk_generation_inputs,
    resolve_chunk_surface_plan, resolve_material_policy_for_archetype, sample_region_classes,
};

#[path = "shared/world_dump_common.rs"]
mod world_dump_common;

use world_dump_common::{CreatedWorldManifest, load_chunk_from_dump, read_manifest};

const DEFAULT_CENTER_X: i32 = 0;
const DEFAULT_CENTER_Z: i32 = 0;
const DEFAULT_RADIUS: i32 = 0;
const DEFAULT_MIN_Y_CHUNK: i32 = -2;
const DEFAULT_MAX_Y_CHUNK: i32 = 3;
const DEFAULT_PIXELS_PER_BLOCK: u32 = 6;
const DEFAULT_STAGE_GENERATION_PADDING: i32 = 2;

#[derive(Debug, Clone)]
enum PreviewSource {
    Seed(u64),
    CreatedWorld(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewStage {
    Full,
    Prototype,
    Hydrology,
    HardMaterial,
    SurfaceMaterial,
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

#[derive(Debug, Clone, Copy)]
struct StagePreviewCell {
    top_y: i32,
    height: f32,
    relief_budget: f32,
    water_surface_height: Option<f32>,
    hydrology_mode: HydrologyMode,
    saturation: f32,
}

#[derive(Debug, Clone)]
struct StagePreviewGrid {
    origin_x: i32,
    origin_z: i32,
    width: usize,
    depth: usize,
    cells: Vec<StagePreviewCell>,
}

impl StagePreviewGrid {
    fn index_of(&self, world_x: i32, world_z: i32) -> Option<usize> {
        if world_x < self.origin_x || world_z < self.origin_z {
            return None;
        }

        let dx = usize::try_from(world_x - self.origin_x).ok()?;
        let dz = usize::try_from(world_z - self.origin_z).ok()?;
        if dx >= self.width || dz >= self.depth {
            return None;
        }

        Some(dz * self.width + dx)
    }

    fn cell(&self, world_x: i32, world_z: i32) -> Option<StagePreviewCell> {
        self.index_of(world_x, world_z)
            .map(|index| self.cells[index])
    }
}

#[derive(Debug, Clone, Copy)]
struct StagePreviewColumn {
    height: f32,
    relief_budget: f32,
    water_surface_height: Option<f32>,
    hydrology_mode: HydrologyMode,
    saturation: f32,
}

#[derive(Debug, Clone)]
struct StagePreviewChunk {
    coord: ChunkCoord,
    columns: Vec<StagePreviewColumn>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct StageDebugSummary {
    total_columns: usize,
    min_y: i32,
    max_y: i32,
    mean_y: f32,
    water_columns: usize,
    channel_columns: usize,
    floodplain_columns: usize,
    lake_columns: usize,
    wetland_columns: usize,
    mean_saturation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MaterialPreviewCell {
    policy: MaterialPolicyId,
    owner_archetype: RegionArchetype,
    top_y: i32,
}

#[derive(Debug, Clone)]
struct MaterialPreviewGrid {
    origin_x: i32,
    origin_z: i32,
    width: usize,
    depth: usize,
    cells: Vec<MaterialPreviewCell>,
}

impl MaterialPreviewGrid {
    fn index_of(&self, world_x: i32, world_z: i32) -> Option<usize> {
        if world_x < self.origin_x || world_z < self.origin_z {
            return None;
        }

        let dx = usize::try_from(world_x - self.origin_x).ok()?;
        let dz = usize::try_from(world_z - self.origin_z).ok()?;
        if dx >= self.width || dz >= self.depth {
            return None;
        }

        Some(dz * self.width + dx)
    }

    fn cell(&self, world_x: i32, world_z: i32) -> Option<MaterialPreviewCell> {
        self.index_of(world_x, world_z)
            .map(|index| self.cells[index])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MaterialPolicyCount {
    policy: MaterialPolicyId,
    count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MaterialDebugSummary {
    total_columns: usize,
    policy_counts: Vec<MaterialPolicyCount>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let source = if args.first().map(String::as_str) == Some("--world-dir") {
        args.remove(0);
        PreviewSource::CreatedWorld(PathBuf::from(parse_required::<String>(
            &mut args,
            "world-dir",
        )?))
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
    let mut stage = PreviewStage::Full;
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
            "--stage" => {
                stage = parse_preview_stage(parse_required::<String>(&mut args, "stage")?)?;
            }
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
    if pixels_per_block == 0 {
        return Err(cli_error("pixels-per-block must be >= 1"));
    }
    if created_world_stage_requested(stage) && matches!(source, PreviewSource::CreatedWorld(_)) {
        return Err(cli_error(
            "prototype and hydrology stages are supported only for direct seed previews",
        ));
    }

    let block_registry = Arc::new(
        BlockRegistry::load_default()
            .map_err(|error| cli_error(format!("failed to load block registry: {error:?}")))?,
    );

    let (meta, created_world_dir, created_world_manifest) = match &source {
        PreviewSource::Seed(seed) => (WorldMeta::new(*seed), None, None),
        PreviewSource::CreatedWorld(world_dir) => {
            let manifest = read_manifest(world_dir)?;
            let meta = WorldMeta {
                seed: manifest.seed,
                world_version: WorldMeta::CURRENT_WORLD_VERSION,
                generator_version: manifest.generator_version,
                save_format_version: manifest.save_format_version,
            };
            (meta, Some(world_dir.clone()), Some(manifest))
        }
    };

    let requested_center = if center_explicit {
        (center_x, center_z)
    } else if let Some(manifest) = created_world_manifest.as_ref() {
        choose_created_world_center(manifest, radius, min_y_chunk, max_y_chunk)?
    } else {
        (center_x, center_z)
    };
    center_x = requested_center.0;
    center_z = requested_center.1;

    let output =
        output.unwrap_or_else(|| default_output_path(&source, center_x, center_z, radius, stage));
    let (image, surface_range, debug_summary, mesh_debug, stage_debug, material_debug) = match stage
    {
        PreviewStage::Full => {
            let mut world = WorldCore::new(meta, Arc::clone(&block_registry));

            match created_world_dir.as_deref() {
                Some(world_dir) => {
                    let manifest = created_world_manifest
                        .as_ref()
                        .expect("created-world preview metadata should exist");
                    ensure_created_world_bounds_cover_request(
                        manifest.min_chunk_coord(),
                        manifest.max_chunk_coord(),
                        center_x,
                        center_z,
                        radius,
                        min_y_chunk,
                        max_y_chunk,
                    )?;
                    load_preview_chunks(
                        &mut world,
                        world_dir,
                        manifest.min_chunk_coord(),
                        manifest.max_chunk_coord(),
                        center_x,
                        center_z,
                        radius,
                        min_y_chunk,
                        max_y_chunk,
                    )?;
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
            (
                image,
                surface_range,
                Some(debug_summary),
                Some(mesh_debug),
                None,
                None,
            )
        }
        PreviewStage::Prototype | PreviewStage::Hydrology => {
            let generation_radius = radius + DEFAULT_STAGE_GENERATION_PADDING;
            let grid =
                build_stage_preview_grid(&meta, center_x, center_z, generation_radius, stage)?;
            let (image, surface_range, stage_debug) =
                render_stage_topdown_preview(&grid, center_x, center_z, radius, pixels_per_block)?;
            (image, surface_range, None, None, Some(stage_debug), None)
        }
        PreviewStage::HardMaterial | PreviewStage::SurfaceMaterial => {
            let generation_radius = radius + DEFAULT_STAGE_GENERATION_PADDING;
            let grid =
                build_material_preview_grid(&meta, center_x, center_z, generation_radius, stage)?;
            let (image, surface_range, material_debug) = render_material_topdown_preview(
                &grid,
                center_x,
                center_z,
                radius,
                pixels_per_block,
            )?;
            (image, surface_range, None, None, None, Some(material_debug))
        }
    };
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
    println!("preview stage: {}", preview_stage_label(stage));
    println!("center chunk: ({center_x}, {center_z})");
    println!(
        "footprint: xz radius={}, y={}..{}, pixels-per-block={}",
        radius, min_y_chunk, max_y_chunk, pixels_per_block
    );
    println!(
        "surface relief: {}..{}",
        surface_range.min_y, surface_range.max_y
    );
    if let Some(debug_summary) = debug_summary.as_ref() {
        print_preview_debug_summary(debug_summary);
    }
    if let Some(mesh_debug) = mesh_debug {
        print_mesh_debug_summary(mesh_debug);
        println!(
            "water diagnostic: {}",
            diagnose_water_visibility(
                debug_summary
                    .as_ref()
                    .expect("full preview should include column debug"),
                mesh_debug
            )
        );
    }
    if let Some(stage_debug) = stage_debug {
        print_stage_debug_summary(stage_debug);
    }
    if let Some(material_debug) = material_debug.as_ref() {
        print_material_debug_summary(material_debug);
    }
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

    let grid_width =
        usize::try_from(blocks_per_axis).map_err(|_| cli_error("grid width overflowed"))?;
    let grid_height =
        usize::try_from(blocks_per_axis).map_err(|_| cli_error("grid height overflowed"))?;
    let mut cells = Vec::with_capacity(grid_width * grid_height);

    for z_offset in 0..grid_height {
        let world_z =
            min_world_z + i32::try_from(z_offset).expect("grid z index should fit in i32");
        for x_offset in 0..grid_width {
            let world_x =
                min_world_x + i32::try_from(x_offset).expect("grid x index should fit in i32");
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

fn build_stage_preview_grid(
    meta: &WorldMeta,
    center_x: i32,
    center_z: i32,
    generation_radius: i32,
    stage: PreviewStage,
) -> Result<StagePreviewGrid, Box<dyn Error>> {
    let min_chunk_x = center_x - generation_radius;
    let max_chunk_x = center_x + generation_radius;
    let min_chunk_z = center_z - generation_radius;
    let max_chunk_z = center_z + generation_radius;
    let width_chunks = usize::try_from(max_chunk_x - min_chunk_x + 1)
        .map_err(|_| cli_error("invalid stage preview width"))?;
    let depth_chunks = usize::try_from(max_chunk_z - min_chunk_z + 1)
        .map_err(|_| cli_error("invalid stage preview depth"))?;
    let width = width_chunks * CHUNK_EDGE;
    let depth = depth_chunks * CHUNK_EDGE;
    let origin_x = min_chunk_x * CHUNK_EDGE_I32;
    let origin_z = min_chunk_z * CHUNK_EDGE_I32;
    let mut cells = vec![
        StagePreviewCell {
            top_y: WORLD_FLOOR_Y,
            height: WORLD_FLOOR_Y as f32,
            relief_budget: 0.0,
            water_surface_height: None,
            hydrology_mode: HydrologyMode::Dry,
            saturation: 0.0,
        };
        width * depth
    ];
    let input_cache =
        PreviewInputCache::for_preview_window(meta, center_x, center_z, generation_radius);
    let chunks = preview_chunk_xz_coords(center_x, center_z, generation_radius)
        .into_par_iter()
        .map(|(chunk_x, chunk_z)| {
            let chunk = ChunkCoord(chunk_x, 0, chunk_z);
            let inputs = input_cache.inputs_for_chunk(chunk);
            let realization = build_chunk_realization_field_patch(chunk, &inputs);
            let corridors = build_chunk_corridor_window(chunk, &inputs);
            let prototype =
                build_chunk_base_heightfield_prototype(chunk, &inputs, &realization, &corridors);
            let meso = build_chunk_meso_applied_prototype(chunk, &inputs, &corridors, &prototype);
            let columns = match stage {
                PreviewStage::Prototype => meso
                    .columns
                    .into_iter()
                    .map(|column| StagePreviewColumn {
                        height: column.height,
                        relief_budget: column.remaining_relief_budget,
                        water_surface_height: None,
                        hydrology_mode: HydrologyMode::Dry,
                        saturation: 0.0,
                    })
                    .collect(),
                PreviewStage::Hydrology => {
                    let smoothed = build_chunk_smoothed_prototype(chunk, &corridors, &meso);
                    let hydrology =
                        build_chunk_hydrology_solve(chunk, &inputs, &corridors, &smoothed);

                    hydrology
                        .columns
                        .into_iter()
                        .zip(smoothed.columns)
                        .map(|(column, smoothed)| StagePreviewColumn {
                            height: column.terrain_height,
                            relief_budget: smoothed.remaining_relief_budget,
                            water_surface_height: column.water_surface_height,
                            hydrology_mode: column.mode,
                            saturation: column.saturation,
                        })
                        .collect()
                }
                PreviewStage::Full => unreachable!(
                    "stage preview grids are only used for prototype and hydrology previews"
                ),
                PreviewStage::HardMaterial | PreviewStage::SurfaceMaterial => {
                    unreachable!("material preview stages use the material preview grid")
                }
            };

            StagePreviewChunk {
                coord: chunk,
                columns,
            }
        })
        .collect::<Vec<_>>();

    for chunk in chunks {
        let chunk_row = usize::try_from(chunk.coord.2 - min_chunk_z)
            .map_err(|_| cli_error("invalid stage preview chunk row"))?;
        let chunk_col = usize::try_from(chunk.coord.0 - min_chunk_x)
            .map_err(|_| cli_error("invalid stage preview chunk column"))?;

        for local_z in 0..CHUNK_EDGE_I32 {
            let global_z = chunk_row * CHUNK_EDGE
                + usize::try_from(local_z).map_err(|_| cli_error("invalid local z"))?;
            let row_offset = global_z * width;
            for local_x in 0..CHUNK_EDGE_I32 {
                let column_index = usize::try_from(local_z * CHUNK_EDGE_I32 + local_x)
                    .map_err(|_| cli_error("invalid stage column index"))?;
                let column = chunk.columns[column_index];
                let global_x = chunk_col * CHUNK_EDGE
                    + usize::try_from(local_x).map_err(|_| cli_error("invalid local x"))?;
                let top_y = column.height.round() as i32;
                cells[row_offset + global_x] = StagePreviewCell {
                    top_y,
                    height: column.height,
                    relief_budget: column.relief_budget,
                    water_surface_height: column.water_surface_height,
                    hydrology_mode: column.hydrology_mode,
                    saturation: column.saturation,
                };
            }
        }
    }

    Ok(StagePreviewGrid {
        origin_x,
        origin_z,
        width,
        depth,
        cells,
    })
}

fn render_stage_topdown_preview(
    grid: &StagePreviewGrid,
    center_x: i32,
    center_z: i32,
    radius: i32,
    pixels_per_block: u32,
) -> Result<(RgbImage, SurfaceRange, StageDebugSummary), Box<dyn Error>> {
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
    let grid_width =
        usize::try_from(blocks_per_axis).map_err(|_| cli_error("grid width overflowed"))?;
    let grid_height =
        usize::try_from(blocks_per_axis).map_err(|_| cli_error("grid height overflowed"))?;
    let min_world_x = (center_x - radius) * CHUNK_EDGE_I32;
    let min_world_z = (center_z - radius) * CHUNK_EDGE_I32;
    let mut cells = Vec::with_capacity(grid_width * grid_height);

    for z_offset in 0..grid_height {
        let world_z =
            min_world_z + i32::try_from(z_offset).expect("grid z index should fit in i32");
        for x_offset in 0..grid_width {
            let world_x =
                min_world_x + i32::try_from(x_offset).expect("grid x index should fit in i32");
            cells.push(
                grid.cell(world_x, world_z)
                    .ok_or_else(|| cli_error("stage grid did not cover requested render area"))?,
            );
        }
    }

    let surface_range = surface_range_for_stage_cells(&cells)
        .ok_or_else(|| cli_error("stage preview did not produce any cells"))?;
    let stage_debug = summarize_stage_cells(&cells);
    let mut image = RgbImage::new(width, height);

    for z in 0..grid_height {
        for x in 0..grid_width {
            let cell = cells[z * grid_width + x];
            let base = color_for_stage_cell(cell, surface_range);
            let pixel_origin_x =
                u32::try_from(x).expect("grid x index should fit in u32") * pixels_per_block;
            let pixel_origin_y =
                u32::try_from(z).expect("grid z index should fit in u32") * pixels_per_block;

            for local_y in 0..pixels_per_block {
                for local_x in 0..pixels_per_block {
                    let outline = stage_outline_strength(
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

    Ok((image, surface_range, stage_debug))
}

fn build_material_preview_grid(
    meta: &WorldMeta,
    center_x: i32,
    center_z: i32,
    generation_radius: i32,
    stage: PreviewStage,
) -> Result<MaterialPreviewGrid, Box<dyn Error>> {
    let min_chunk_x = center_x - generation_radius;
    let max_chunk_x = center_x + generation_radius;
    let min_chunk_z = center_z - generation_radius;
    let max_chunk_z = center_z + generation_radius;
    let width_chunks = usize::try_from(max_chunk_x - min_chunk_x + 1)
        .map_err(|_| cli_error("invalid material preview width"))?;
    let depth_chunks = usize::try_from(max_chunk_z - min_chunk_z + 1)
        .map_err(|_| cli_error("invalid material preview depth"))?;
    let width = width_chunks * CHUNK_EDGE;
    let depth = depth_chunks * CHUNK_EDGE;
    let origin_x = min_chunk_x * CHUNK_EDGE_I32;
    let origin_z = min_chunk_z * CHUNK_EDGE_I32;
    let mut cells = vec![
        MaterialPreviewCell {
            policy: MaterialPolicyId::TemperateGrassland,
            owner_archetype: RegionArchetype::TemperatePlain,
            top_y: WORLD_FLOOR_Y,
        };
        width * depth
    ];
    let input_cache =
        PreviewInputCache::for_preview_window(meta, center_x, center_z, generation_radius);
    let chunks = preview_chunk_xz_coords(center_x, center_z, generation_radius)
        .into_par_iter()
        .map(|(chunk_x, chunk_z)| {
            let chunk = ChunkCoord(chunk_x, 0, chunk_z);
            let inputs = input_cache.inputs_for_chunk(chunk);
            let columns = match stage {
                PreviewStage::HardMaterial => build_hard_material_preview_chunk(chunk, &inputs),
                PreviewStage::SurfaceMaterial => {
                    build_surface_material_preview_chunk(chunk, &inputs)
                }
                _ => unreachable!("material preview grid only supports material stages"),
            };

            (chunk, columns)
        })
        .collect::<Vec<_>>();

    for (chunk, preview_cells) in chunks {
        let chunk_row = usize::try_from(chunk.2 - min_chunk_z)
            .map_err(|_| cli_error("invalid material preview chunk row"))?;
        let chunk_col = usize::try_from(chunk.0 - min_chunk_x)
            .map_err(|_| cli_error("invalid material preview chunk column"))?;

        for local_z in 0..CHUNK_EDGE_I32 {
            let global_z = chunk_row * CHUNK_EDGE
                + usize::try_from(local_z).map_err(|_| cli_error("invalid local z"))?;
            let row_offset = global_z * width;
            for local_x in 0..CHUNK_EDGE_I32 {
                let column_index = usize::try_from(local_z * CHUNK_EDGE_I32 + local_x)
                    .map_err(|_| cli_error("invalid material column index"))?;
                let global_x = chunk_col * CHUNK_EDGE
                    + usize::try_from(local_x).map_err(|_| cli_error("invalid local x"))?;
                cells[row_offset + global_x] = preview_cells[column_index];
            }
        }
    }

    Ok(MaterialPreviewGrid {
        origin_x,
        origin_z,
        width,
        depth,
        cells,
    })
}

fn build_hard_material_preview_chunk(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
) -> Vec<MaterialPreviewCell> {
    let chunk_origin_x = chunk.0 * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk.2 * CHUNK_EDGE_I32;
    let mut cells = Vec::with_capacity(CHUNK_EDGE * CHUNK_EDGE);

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let region = sample_region_classes(&inputs.region_classes, world_x, world_z);
            cells.push(MaterialPreviewCell {
                policy: resolve_material_policy_for_archetype(region.archetype),
                owner_archetype: region.archetype,
                top_y: WORLD_FLOOR_Y,
            });
        }
    }

    cells
}

fn build_surface_material_preview_chunk(
    chunk: ChunkCoord,
    inputs: &ChunkGenerationInputs,
) -> Vec<MaterialPreviewCell> {
    let realization = build_chunk_realization_field_patch(chunk, inputs);
    let corridors = build_chunk_corridor_window(chunk, inputs);
    let prototype = build_chunk_base_heightfield_prototype(chunk, inputs, &realization, &corridors);
    let meso = build_chunk_meso_applied_prototype(chunk, inputs, &corridors, &prototype);
    let smoothed = build_chunk_smoothed_prototype(chunk, &corridors, &meso);
    let hydrology = build_chunk_hydrology_solve(chunk, inputs, &corridors, &smoothed);
    let surface = resolve_chunk_surface_plan(chunk, inputs, &smoothed, &hydrology);

    surface
        .columns
        .into_iter()
        .map(|column| MaterialPreviewCell {
            policy: column.material_policy,
            owner_archetype: column.owner_archetype,
            top_y: column.terrain_top_y,
        })
        .collect()
}

fn render_material_topdown_preview(
    grid: &MaterialPreviewGrid,
    center_x: i32,
    center_z: i32,
    radius: i32,
    pixels_per_block: u32,
) -> Result<(RgbImage, SurfaceRange, MaterialDebugSummary), Box<dyn Error>> {
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
    let grid_width =
        usize::try_from(blocks_per_axis).map_err(|_| cli_error("grid width overflowed"))?;
    let grid_height =
        usize::try_from(blocks_per_axis).map_err(|_| cli_error("grid height overflowed"))?;
    let min_world_x = (center_x - radius) * CHUNK_EDGE_I32;
    let min_world_z = (center_z - radius) * CHUNK_EDGE_I32;
    let mut cells = Vec::with_capacity(grid_width * grid_height);

    for z_offset in 0..grid_height {
        let world_z =
            min_world_z + i32::try_from(z_offset).expect("grid z index should fit in i32");
        for x_offset in 0..grid_width {
            let world_x =
                min_world_x + i32::try_from(x_offset).expect("grid x index should fit in i32");
            cells.push(
                grid.cell(world_x, world_z).ok_or_else(|| {
                    cli_error("material grid did not cover requested render area")
                })?,
            );
        }
    }

    let surface_range = surface_range_for_material_cells(&cells)
        .ok_or_else(|| cli_error("material preview did not produce any cells"))?;
    let material_debug = summarize_material_cells(&cells);
    let mut image = RgbImage::new(width, height);

    for z in 0..grid_height {
        for x in 0..grid_width {
            let cell = cells[z * grid_width + x];
            let base = color_for_material_policy(cell.policy);
            let pixel_origin_x =
                u32::try_from(x).expect("grid x index should fit in u32") * pixels_per_block;
            let pixel_origin_y =
                u32::try_from(z).expect("grid z index should fit in u32") * pixels_per_block;

            for local_y in 0..pixels_per_block {
                for local_x in 0..pixels_per_block {
                    let outline = material_outline_strength(
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

    Ok((image, surface_range, material_debug))
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

fn surface_range_for_stage_cells(cells: &[StagePreviewCell]) -> Option<SurfaceRange> {
    let mut heights = cells.iter().map(|cell| {
        cell.water_surface_height
            .map(|water| water.ceil() as i32)
            .unwrap_or(cell.top_y)
            .max(cell.top_y)
    });
    let first = heights.next()?;
    let mut min_y = first;
    let mut max_y = first;

    for top_y in heights {
        min_y = min_y.min(top_y);
        max_y = max_y.max(top_y);
    }

    Some(SurfaceRange { min_y, max_y })
}

fn surface_range_for_material_cells(cells: &[MaterialPreviewCell]) -> Option<SurfaceRange> {
    let mut heights = cells.iter().map(|cell| cell.top_y);
    let first = heights.next()?;
    let mut min_y = first;
    let mut max_y = first;

    for top_y in heights {
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

fn summarize_material_cells(cells: &[MaterialPreviewCell]) -> MaterialDebugSummary {
    let mut policy_counts = Vec::<MaterialPolicyCount>::new();

    for cell in cells {
        if let Some(existing) = policy_counts
            .iter_mut()
            .find(|count| count.policy == cell.policy)
        {
            existing.count += 1;
        } else {
            policy_counts.push(MaterialPolicyCount {
                policy: cell.policy,
                count: 1,
            });
        }
    }

    policy_counts.sort_by(|left, right| {
        right.count.cmp(&left.count).then_with(|| {
            material_policy_label(left.policy).cmp(material_policy_label(right.policy))
        })
    });

    MaterialDebugSummary {
        total_columns: cells.len(),
        policy_counts,
    }
}

fn summarize_stage_cells(cells: &[StagePreviewCell]) -> StageDebugSummary {
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    let mut sum_y = 0_i64;
    let mut water_columns = 0_usize;
    let mut channel_columns = 0_usize;
    let mut floodplain_columns = 0_usize;
    let mut lake_columns = 0_usize;
    let mut wetland_columns = 0_usize;
    let mut sum_saturation = 0.0_f32;

    for cell in cells {
        min_y = min_y.min(cell.top_y);
        max_y = max_y.max(cell.top_y);
        sum_y += i64::from(cell.top_y);
        sum_saturation += cell.saturation;
        if cell.water_surface_height.is_some() {
            water_columns += 1;
        }

        match cell.hydrology_mode {
            HydrologyMode::Dry => {}
            HydrologyMode::Channel => channel_columns += 1,
            HydrologyMode::Floodplain => floodplain_columns += 1,
            HydrologyMode::Lake => lake_columns += 1,
            HydrologyMode::Wetland => wetland_columns += 1,
        }
    }

    let total_columns = cells.len();
    StageDebugSummary {
        total_columns,
        min_y: if total_columns == 0 { 0 } else { min_y },
        max_y: if total_columns == 0 { 0 } else { max_y },
        mean_y: if total_columns == 0 {
            0.0
        } else {
            sum_y as f32 / total_columns as f32
        },
        water_columns,
        channel_columns,
        floodplain_columns,
        lake_columns,
        wetland_columns,
        mean_saturation: if total_columns == 0 {
            0.0
        } else {
            sum_saturation / total_columns as f32
        },
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

fn color_for_cell(
    cell: TopdownCell,
    registry: &BlockRegistry,
    surface_range: SurfaceRange,
) -> [u8; 3] {
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

fn color_for_stage_cell(cell: StagePreviewCell, surface_range: SurfaceRange) -> [u8; 3] {
    if let Some(water_height) = cell.water_surface_height {
        let water_t = ((water_height - cell.height) / 8.0).clamp(0.0, 1.0);
        return brighten([68, 122, 192], 0.86 + water_t * 0.18);
    }

    let relief_t = if surface_range.max_y > surface_range.min_y {
        (cell.top_y - surface_range.min_y) as f32
            / (surface_range.max_y - surface_range.min_y) as f32
    } else {
        0.5
    };
    let relief_budget_t = ((cell.relief_budget - 4.0) / 36.0).clamp(0.0, 1.0);
    let saturation_t = cell.saturation.clamp(0.0, 1.0);
    let base = match cell.hydrology_mode {
        HydrologyMode::Dry => [126, 105, 74],
        HydrologyMode::Wetland => [74, 118, 92],
        HydrologyMode::Floodplain => [92, 126, 102],
        HydrologyMode::Channel => [62, 104, 150],
        HydrologyMode::Lake => [70, 118, 176],
    };
    let color = [
        (base[0] as f32 * (0.74 + relief_t * 0.20) + relief_budget_t * 28.0) as u8,
        (base[1] as f32 * (0.78 + relief_t * 0.16) + saturation_t * 24.0) as u8,
        (base[2] as f32 * (0.84 + relief_budget_t * 0.10) + saturation_t * 18.0) as u8,
    ];

    brighten(color, 0.92)
}

fn color_for_material_policy(policy: MaterialPolicyId) -> [u8; 3] {
    match policy {
        MaterialPolicyId::OceanicShelf => [54, 92, 154],
        MaterialPolicyId::SandyBeach => [204, 184, 112],
        MaterialPolicyId::CoastalCliff => [116, 112, 106],
        MaterialPolicyId::TemperateGrassland => [92, 150, 78],
        MaterialPolicyId::TemperatePlateau => [120, 142, 86],
        MaterialPolicyId::SteppeGrassland => [156, 138, 76],
        MaterialPolicyId::SavannaGrassland => [174, 146, 72],
        MaterialPolicyId::TropicalLowland => [58, 136, 82],
        MaterialPolicyId::TropicalHills => [54, 118, 78],
        MaterialPolicyId::DesertSurface => [194, 154, 84],
        MaterialPolicyId::ColdWetland => [112, 78, 54],
        MaterialPolicyId::AlpineExposed => [142, 150, 158],
        MaterialPolicyId::TundraExposure => [132, 148, 132],
    }
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

fn stage_outline_strength(
    cells: &[StagePreviewCell],
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
    let cell = cells[index];
    let mut strength = 0.0_f32;

    if local_x == 0 {
        strength = strength.max(stage_edge_strength(
            cell,
            x.checked_sub(1).map(|nx| cells[z * width + nx]),
        ));
    }
    if local_y == 0 {
        strength = strength.max(stage_edge_strength(
            cell,
            z.checked_sub(1).map(|nz| cells[nz * width + x]),
        ));
    }
    if local_x + 1 == pixels_per_block {
        let right = if x + 1 < width {
            Some(cells[z * width + (x + 1)])
        } else {
            None
        };
        strength = strength.max(stage_edge_strength(cell, right));
    }
    if local_y + 1 == pixels_per_block {
        let bottom = if z + 1 < height {
            Some(cells[(z + 1) * width + x])
        } else {
            None
        };
        strength = strength.max(stage_edge_strength(cell, bottom));
    }

    strength
}

fn stage_edge_strength(cell: StagePreviewCell, neighbor: Option<StagePreviewCell>) -> f32 {
    match neighbor {
        None => 0.24,
        Some(other) if other.hydrology_mode != cell.hydrology_mode => 0.24,
        Some(other) if other.top_y.abs_diff(cell.top_y) >= 2 => 0.20,
        Some(_) => 0.08,
    }
}

fn material_outline_strength(
    cells: &[MaterialPreviewCell],
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
    let cell = cells[index];
    let mut strength = 0.0_f32;

    if local_x == 0 {
        strength = strength.max(material_edge_strength(
            cell,
            x.checked_sub(1).map(|nx| cells[z * width + nx]),
        ));
    }
    if local_y == 0 {
        strength = strength.max(material_edge_strength(
            cell,
            z.checked_sub(1).map(|nz| cells[nz * width + x]),
        ));
    }
    if local_x + 1 == pixels_per_block {
        let right = if x + 1 < width {
            Some(cells[z * width + (x + 1)])
        } else {
            None
        };
        strength = strength.max(material_edge_strength(cell, right));
    }
    if local_y + 1 == pixels_per_block {
        let bottom = if z + 1 < height {
            Some(cells[(z + 1) * width + x])
        } else {
            None
        };
        strength = strength.max(material_edge_strength(cell, bottom));
    }

    strength
}

fn material_edge_strength(cell: MaterialPreviewCell, neighbor: Option<MaterialPreviewCell>) -> f32 {
    match neighbor {
        None => 0.24,
        Some(other) if other.policy != cell.policy => 0.28,
        Some(other) if other.owner_archetype != cell.owner_archetype => 0.18,
        Some(_) => 0.06,
    }
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
    println!(
        "  total water blocks in scanned volume: {}",
        summary.total_water_blocks
    );
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

fn print_stage_debug_summary(summary: StageDebugSummary) {
    println!("stage debug:");
    println!("  total columns: {}", summary.total_columns);
    println!(
        "  surface_y: avg={:.2}, min={}, max={}",
        summary.mean_y, summary.min_y, summary.max_y
    );
    println!(
        "  hydrology: water_columns={} ({:.1}%), channels={}, floodplain={}, lakes={}, wetlands={}, mean_saturation={:.3}",
        summary.water_columns,
        percent(summary.water_columns, summary.total_columns),
        summary.channel_columns,
        summary.floodplain_columns,
        summary.lake_columns,
        summary.wetland_columns,
        summary.mean_saturation,
    );
}

fn print_material_debug_summary(summary: &MaterialDebugSummary) {
    println!("material debug:");
    println!("  total columns: {}", summary.total_columns);
    println!("  material policies:");
    for count in summary.policy_counts.iter().take(12) {
        println!(
            "    {}: {} ({:.1}%)",
            material_policy_label(count.policy),
            count.count,
            percent(count.count, summary.total_columns)
        );
    }
}

fn print_block_counts(label: &str, counts: &[BlockCount]) {
    println!("{label}");
    for count in counts.iter().take(8) {
        println!("    {}: {}", count.key, count.count);
    }
}

fn material_policy_label(policy: MaterialPolicyId) -> &'static str {
    match policy {
        MaterialPolicyId::OceanicShelf => "oceanic_shelf",
        MaterialPolicyId::SandyBeach => "sandy_beach",
        MaterialPolicyId::CoastalCliff => "coastal_cliff",
        MaterialPolicyId::TemperateGrassland => "temperate_grassland",
        MaterialPolicyId::TemperatePlateau => "temperate_plateau",
        MaterialPolicyId::SteppeGrassland => "steppe_grassland",
        MaterialPolicyId::SavannaGrassland => "savanna_grassland",
        MaterialPolicyId::TropicalLowland => "tropical_lowland",
        MaterialPolicyId::TropicalHills => "tropical_hills",
        MaterialPolicyId::DesertSurface => "desert_surface",
        MaterialPolicyId::ColdWetland => "cold_wetland",
        MaterialPolicyId::AlpineExposed => "alpine_exposed",
        MaterialPolicyId::TundraExposure => "tundra_exposure",
    }
}

fn percent(part: usize, whole: usize) -> f32 {
    if whole == 0 {
        0.0
    } else {
        (part as f32 / whole as f32) * 100.0
    }
}

fn diagnose_water_visibility(
    summary: &PreviewDebugSummary,
    mesh_debug: MeshDebugSummary,
) -> &'static str {
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
    let meta = *world.meta();
    let input_cache = PreviewInputCache::for_preview_window(&meta, center_x, center_z, radius);
    let generated_chunks =
        preview_chunk_coords(center_x, center_z, radius, min_y_chunk, max_y_chunk)
            .into_par_iter()
            .map(|coord| {
                let inputs = input_cache.inputs_for_chunk(coord);
                (
                    coord,
                    generate_chunk_from_generation_inputs(&inputs, registry),
                )
            })
            .collect::<Vec<_>>();

    for (coord, chunk) in generated_chunks {
        world.insert_chunk(coord, chunk);
    }
}

fn load_preview_chunks(
    world: &mut WorldCore,
    world_dir: &Path,
    min_chunk: ChunkCoord,
    max_chunk: ChunkCoord,
    center_x: i32,
    center_z: i32,
    radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
) -> Result<(), Box<dyn Error>> {
    let load_min_x = (center_x - radius).max(min_chunk.0);
    let load_max_x = (center_x + radius).min(max_chunk.0);
    let load_min_y = min_y_chunk.max(min_chunk.1);
    let load_max_y = max_y_chunk.min(max_chunk.1);
    let load_min_z = (center_z - radius).max(min_chunk.2);
    let load_max_z = (center_z + radius).min(max_chunk.2);
    let loaded_chunks = preview_chunk_coords_for_bounds(
        load_min_x, load_max_x, load_min_y, load_max_y, load_min_z, load_max_z,
    )
    .into_par_iter()
    .map(|coord| {
        load_chunk_from_dump(world_dir, coord)
            .map(|chunk| (coord, chunk))
            .map_err(|error| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "failed to load preview chunk {coord:?} from {}: {error}",
                        world_dir.display()
                    ),
                )
            })
    })
    .collect::<Result<Vec<_>, _>>()?;

    for (coord, chunk) in loaded_chunks {
        world.insert_chunk(coord, chunk);
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
        let requested_min = ChunkCoord(
            summary.center_x - radius,
            min_y_chunk,
            summary.center_z - radius,
        );
        let requested_max = ChunkCoord(
            summary.center_x + radius,
            max_y_chunk,
            summary.center_z + radius,
        );
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
        radius, min_chunk.0, max_chunk.0, min_chunk.1, max_chunk.1, min_chunk.2, max_chunk.2
    )))
}

fn default_output_path(
    source: &PreviewSource,
    center_x: i32,
    center_z: i32,
    radius: i32,
    stage: PreviewStage,
) -> PathBuf {
    match source {
        PreviewSource::Seed(seed) => {
            let stage_suffix = match stage {
                PreviewStage::Full => String::new(),
                PreviewStage::Prototype => String::from("_prototype"),
                PreviewStage::Hydrology => String::from("_hydrology"),
                PreviewStage::HardMaterial => String::from("_hard_material"),
                PreviewStage::SurfaceMaterial => String::from("_surface_material"),
            };
            PathBuf::from(format!(
                "target/chunk-topdown-preview/seed_{seed}{stage_suffix}_cx{center_x}_cz{center_z}_r{radius}.png"
            ))
        }
        PreviewSource::CreatedWorld(world_dir) => {
            world_dir.join(format!("topdown_cx{center_x}_cz{center_z}_r{radius}.png"))
        }
    }
}

fn preview_chunk_xz_coords(center_x: i32, center_z: i32, radius: i32) -> Vec<(i32, i32)> {
    let mut coords = Vec::new();
    for chunk_z in (center_z - radius)..=(center_z + radius) {
        for chunk_x in (center_x - radius)..=(center_x + radius) {
            coords.push((chunk_x, chunk_z));
        }
    }
    coords
}

fn preview_chunk_coords(
    center_x: i32,
    center_z: i32,
    radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
) -> Vec<ChunkCoord> {
    preview_chunk_coords_for_bounds(
        center_x - radius,
        center_x + radius,
        min_y_chunk,
        max_y_chunk,
        center_z - radius,
        center_z + radius,
    )
}

fn preview_chunk_coords_for_bounds(
    min_chunk_x: i32,
    max_chunk_x: i32,
    min_chunk_y: i32,
    max_chunk_y: i32,
    min_chunk_z: i32,
    max_chunk_z: i32,
) -> Vec<ChunkCoord> {
    let mut coords = Vec::new();
    for chunk_y in min_chunk_y..=max_chunk_y {
        for chunk_z in min_chunk_z..=max_chunk_z {
            for chunk_x in min_chunk_x..=max_chunk_x {
                coords.push(ChunkCoord(chunk_x, chunk_y, chunk_z));
            }
        }
    }
    coords
}

#[derive(Debug, Clone)]
struct PreviewInputCache {
    entries: HashMap<PreviewInputCacheKey, ChunkGenerationInputs>,
}

impl PreviewInputCache {
    fn for_preview_window(meta: &WorldMeta, center_x: i32, center_z: i32, radius: i32) -> Self {
        let mut representatives = HashMap::<PreviewInputCacheKey, ChunkCoord>::new();
        for (chunk_x, chunk_z) in preview_chunk_xz_coords(center_x, center_z, radius) {
            let coord = ChunkCoord(chunk_x, 0, chunk_z);
            representatives
                .entry(PreviewInputCacheKey::for_chunk(coord))
                .or_insert(coord);
        }

        let entries = representatives
            .into_par_iter()
            .map(|(key, coord)| (key, prepare_chunk_generation_inputs(coord, meta)))
            .collect::<HashMap<_, _>>();

        Self { entries }
    }

    fn inputs_for_chunk(&self, coord: ChunkCoord) -> ChunkGenerationInputs {
        let key = PreviewInputCacheKey::for_chunk(coord);
        let mut inputs = self
            .entries
            .get(&key)
            .expect("preview input cache should cover every requested chunk")
            .clone();
        inputs.chunk = coord;
        inputs
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PreviewInputCacheKey {
    origin_x: i32,
    origin_z: i32,
    width: u32,
    height: u32,
}

impl PreviewInputCacheKey {
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

fn parse_preview_stage(value: String) -> Result<PreviewStage, Box<dyn Error>> {
    match value.to_ascii_lowercase().as_str() {
        "full" | "default" => Ok(PreviewStage::Full),
        "prototype" => Ok(PreviewStage::Prototype),
        "hydrology" => Ok(PreviewStage::Hydrology),
        "hard-material" | "hard_material" => Ok(PreviewStage::HardMaterial),
        "surface-material" | "surface_material" | "material" => Ok(PreviewStage::SurfaceMaterial),
        other => Err(cli_error(format!(
            "unknown stage '{other}'; expected 'full', 'prototype', 'hydrology', 'hard-material', or 'surface-material'"
        ))),
    }
}

fn preview_stage_label(stage: PreviewStage) -> &'static str {
    match stage {
        PreviewStage::Full => "full",
        PreviewStage::Prototype => "prototype",
        PreviewStage::Hydrology => "hydrology",
        PreviewStage::HardMaterial => "hard-material",
        PreviewStage::SurfaceMaterial => "surface-material",
    }
}

fn created_world_stage_requested(stage: PreviewStage) -> bool {
    matches!(
        stage,
        PreviewStage::Prototype
            | PreviewStage::Hydrology
            | PreviewStage::HardMaterial
            | PreviewStage::SurfaceMaterial
    )
}

fn usage() -> &'static str {
    "usage: cargo run --bin chunk_topdown_preview -- <seed> [--stage <full|prototype|hydrology|hard-material|surface-material>] [--center-x <i32> | --chunk-x <i32>] [--center-z <i32> | --chunk-z <i32>] [--radius <i32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--pixels-per-block <u32>] [--output <path>]\n       cargo run --bin chunk_topdown_preview -- --world-dir <path> [--stage full] [--center-x <i32> | --chunk-x <i32>] [--center-z <i32> | --chunk-z <i32>] [--radius <i32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--pixels-per-block <u32>] [--output <path>]"
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

    #[test]
    fn parse_preview_stage_accepts_full_prototype_and_hydrology() {
        assert_eq!(
            parse_preview_stage("full".to_string()).unwrap(),
            PreviewStage::Full
        );
        assert_eq!(
            parse_preview_stage("prototype".to_string()).unwrap(),
            PreviewStage::Prototype
        );
        assert_eq!(
            parse_preview_stage("hydrology".to_string()).unwrap(),
            PreviewStage::Hydrology
        );
        assert_eq!(
            parse_preview_stage("hard-material".to_string()).unwrap(),
            PreviewStage::HardMaterial
        );
        assert_eq!(
            parse_preview_stage("material".to_string()).unwrap(),
            PreviewStage::SurfaceMaterial
        );
        assert!(parse_preview_stage("nonsense".to_string()).is_err());
    }

    #[test]
    fn stage_default_output_path_marks_stage() {
        let source = PreviewSource::Seed(42);
        let output = default_output_path(&source, -200, -80, 4, PreviewStage::Hydrology);

        assert!(
            output
                .to_string_lossy()
                .contains("seed_42_hydrology_cx-200_cz-80_r4.png")
        );
    }
}
