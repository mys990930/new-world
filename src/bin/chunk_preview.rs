use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;

use new_world::ecs::{QUARTER_VIEW_VERTICAL_WORLD_SIZE, quarter_view_basis, quarter_view_eye};
use new_world::renderer::{
    CpuMesh as RenderCpuMesh, OffscreenRenderRequest, RenderCameraState, RenderEnvironment,
    RenderProjectionMode, RenderTextureArraySource, RenderTextureSource, RenderTextureTile,
    RenderViewBasis, render_offscreen, write_offscreen_png,
};
use new_world::world::{
    ATLAS_CELL_SIZE_IN_CHUNKS, AtlasCell, AtlasCoord, BlockFace, BlockMaterialKind, BlockRegistry,
    CHUNK_EDGE, CHUNK_EDGE_I32, ChunkCoord, ChunkGenerationInputs, HydrologyMode, MesoGuideSample,
    RegionClassSample, TerrainProfile, TextureTileSource, WORLD_FLOOR_Y, WorldCore, WorldMeta,
    build_chunk_base_heightfield_prototype, build_chunk_corridor_window,
    build_chunk_generation_scaffold, build_chunk_hydrology_solve, build_chunk_mesh,
    build_chunk_meso_applied_prototype, build_chunk_realization_field_patch,
    build_chunk_smoothed_prototype, chunk_generation_input_area,
    generate_chunk_from_generation_inputs, prepare_chunk_generation_inputs, region_archetype_def,
    sample_chunk_surface_lod, sample_meso_guides, sample_topdown_columns,
};

#[path = "shared/world_dump_common.rs"]
mod world_dump_common;

use world_dump_common::{CreatedWorldManifest, load_chunk_from_dump, read_manifest};

const DEFAULT_RENDER_RADIUS: i32 = 4;
const DEFAULT_RENDER_PADDING: i32 = 2;
const DEFAULT_IMAGE_WIDTH: u32 = 1600;
const DEFAULT_IMAGE_HEIGHT: u32 = 900;
const DEFAULT_MIN_Y_CHUNK: i32 = -2;
const DEFAULT_MAX_Y_CHUNK: i32 = 3;
const DEFAULT_LOD_BLOCKS: u8 = 1;
const DEFAULT_LOD_VERTICAL_EXAGGERATION: f32 = 2.4;
const DEFAULT_PROTOTYPE_VERTICAL_EXAGGERATION: f32 = 1.9;

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

    let mut center_x = 0_i32;
    let mut center_z = 0_i32;
    let mut center_explicit = false;
    let mut radius = DEFAULT_RENDER_RADIUS;
    let mut quarter_turns = 0_u8;
    let mut width = DEFAULT_IMAGE_WIDTH;
    let mut height = DEFAULT_IMAGE_HEIGHT;
    let mut min_y_chunk = DEFAULT_MIN_Y_CHUNK;
    let mut max_y_chunk = DEFAULT_MAX_Y_CHUNK;
    let mut lod_blocks = DEFAULT_LOD_BLOCKS;
    let mut stage = PreviewStage::Full;
    let mut output: Option<PathBuf> = None;

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--center-x" => {
                center_x = parse_required::<i32>(&mut args, "center-x")?;
                center_explicit = true;
            }
            "--center-z" => {
                center_z = parse_required::<i32>(&mut args, "center-z")?;
                center_explicit = true;
            }
            "--radius" => radius = parse_required::<i32>(&mut args, "radius")?,
            "--quarter-turns" => quarter_turns = parse_required::<u8>(&mut args, "quarter-turns")?,
            "--width" => width = parse_required::<u32>(&mut args, "width")?,
            "--height" => height = parse_required::<u32>(&mut args, "height")?,
            "--min-y-chunk" => min_y_chunk = parse_required::<i32>(&mut args, "min-y-chunk")?,
            "--max-y-chunk" => max_y_chunk = parse_required::<i32>(&mut args, "max-y-chunk")?,
            "--lod-blocks" => lod_blocks = parse_required::<u8>(&mut args, "lod-blocks")?,
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
    if lod_blocks == 0 {
        return Err(cli_error("lod-blocks must be >= 1"));
    }
    if CHUNK_EDGE % usize::from(lod_blocks) != 0 {
        return Err(cli_error(format!(
            "lod-blocks must evenly divide CHUNK_EDGE ({CHUNK_EDGE})"
        )));
    }
    if matches!(stage, PreviewStage::Prototype | PreviewStage::Hydrology) && lod_blocks > 1 {
        return Err(cli_error(
            "prototype and hydrology stages currently support only the default block resolution; omit --lod-blocks or use --lod-blocks 1",
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

    let output = output.unwrap_or_else(|| {
        default_output_path(&source, center_x, center_z, radius, lod_blocks, stage)
    });
    let generation_radius = radius + DEFAULT_RENDER_PADDING;
    let center_chunk = ChunkCoord(center_x, 0, center_z);
    let center_generator_diagnostics =
        collect_center_chunk_generator_diagnostics(center_chunk, &meta)?;
    if matches!(stage, PreviewStage::Prototype | PreviewStage::Hydrology) {
        if created_world_dir.is_some() {
            return Err(cli_error(
                "prototype and hydrology stages are currently supported only for direct seed previews",
            ));
        }
    }

    let (render_meshes, center_surface_summary, hydrology_summary) = match stage {
        PreviewStage::Prototype => {
            let grid = build_prototype_preview_grid(&meta, center_x, center_z, generation_radius)?;
            let surface_summary = summarize_prototype_chunk_surface(&grid, center_x, center_z);
            let render_meshes = collect_prototype_render_meshes_from_grid(
                &grid,
                block_registry.as_ref(),
                center_x,
                center_z,
                radius,
            );
            (render_meshes, surface_summary, None)
        }
        PreviewStage::Hydrology => {
            let grid = build_hydrology_preview_grid(&meta, center_x, center_z, generation_radius)?;
            let surface_summary = summarize_prototype_chunk_surface(&grid, center_x, center_z);
            let hydrology_summary = summarize_hydrology_chunk(&grid, center_x, center_z);
            let render_meshes = collect_prototype_render_meshes_from_grid(
                &grid,
                block_registry.as_ref(),
                center_x,
                center_z,
                radius,
            );
            (render_meshes, surface_summary, hydrology_summary)
        }
        PreviewStage::Full if lod_blocks > 1 => {
            if created_world_dir.is_some() {
                return Err(cli_error(
                    "lod-blocks > 1 is currently supported only for direct seed previews",
                ));
            }

            (
                collect_lod_render_meshes(
                    &meta,
                    block_registry.as_ref(),
                    center_x,
                    center_z,
                    radius,
                    min_y_chunk,
                    lod_blocks,
                )?,
                None,
                None,
            )
        }
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
                    load_created_world_preview_chunks(
                        &mut world,
                        world_dir,
                        manifest.min_chunk_coord(),
                        manifest.max_chunk_coord(),
                        center_x,
                        center_z,
                        generation_radius,
                        min_y_chunk,
                        max_y_chunk,
                    )?;
                }
                None => generate_preview_chunks(
                    &mut world,
                    block_registry.as_ref(),
                    center_x,
                    center_z,
                    generation_radius,
                    min_y_chunk,
                    max_y_chunk,
                ),
            }

            let surface_summary = summarize_realized_chunk_surface(
                &world,
                block_registry.as_ref(),
                center_chunk,
                min_y_chunk,
                max_y_chunk,
            );
            let render_meshes = collect_render_meshes(
                &world,
                block_registry.as_ref(),
                center_x,
                center_z,
                radius,
                min_y_chunk,
                max_y_chunk,
            );
            (render_meshes, surface_summary, None)
        }
    };
    if render_meshes.is_empty() {
        return Err(cli_error(
            "no visible meshes were produced for the requested preview area",
        ));
    }

    let bounds = combined_render_bounds(&render_meshes)
        .ok_or_else(|| cli_error("failed to compute preview bounds"))?;
    let camera = build_preview_camera(bounds, width, height, quarter_turns % 4);
    let image = render_offscreen(OffscreenRenderRequest {
        width,
        height,
        camera,
        textures: block_registry_to_render_textures(block_registry.as_ref()),
        environment: preview_environment(lod_blocks > 1),
        chunk_meshes: render_meshes.clone(),
        clear_color_override: None,
    })?;
    write_offscreen_png(&output, &image)?;

    match &source {
        PreviewSource::Seed(seed) => println!("preview source: generated from seed {seed}"),
        PreviewSource::CreatedWorld(world_dir) => {
            println!("preview source: created world {}", world_dir.display())
        }
    }
    println!("preview stage: {}", preview_stage_label(stage));
    println!("center chunk: ({center_x}, {center_z})");
    match stage {
        PreviewStage::Prototype => {
            println!(
                "render footprint: xz radius={}, prototype+meso stage, y bounds ignored",
                radius
            );
        }
        PreviewStage::Hydrology => {
            println!(
                "render footprint: xz radius={}, post-smoothing hydrology stage, y bounds ignored",
                radius
            );
        }
        PreviewStage::Full => {
            println!(
                "render footprint: xz radius={}, y={}..{}, lod_blocks={}",
                radius, min_y_chunk, max_y_chunk, lod_blocks
            );
        }
    }
    print_center_chunk_diagnostics(
        &center_generator_diagnostics,
        center_surface_summary,
        hydrology_summary,
    );
    println!("output: {}", output.display());
    println!(
        "image: {}x{}, meshes={}, draw_calls={}",
        image.width,
        image.height,
        render_meshes.len(),
        image.draw_call_count
    );

    Ok(())
}

fn default_output_path(
    source: &PreviewSource,
    center_x: i32,
    center_z: i32,
    radius: i32,
    lod_blocks: u8,
    stage: PreviewStage,
) -> PathBuf {
    match source {
        PreviewSource::Seed(seed) => {
            let stage_suffix = match stage {
                PreviewStage::Full => String::new(),
                PreviewStage::Prototype => String::from("_prototype"),
                PreviewStage::Hydrology => String::from("_hydrology"),
            };
            PathBuf::from(format!(
                "target/chunk-preview/seed_{seed}{stage_suffix}_cx{center_x}_cz{center_z}_r{radius}{}.png",
                if lod_blocks > 1 {
                    format!("_lod{lod_blocks}")
                } else {
                    String::new()
                }
            ))
        }
        PreviewSource::CreatedWorld(world_dir) => world_dir.join(format!(
            "preview_cx{center_x}_cz{center_z}_r{radius}{}.png",
            if lod_blocks > 1 {
                format!("_lod{lod_blocks}")
            } else {
                String::new()
            }
        )),
    }
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

fn load_created_world_preview_chunks(
    world: &mut WorldCore,
    world_dir: &Path,
    min_chunk: ChunkCoord,
    max_chunk: ChunkCoord,
    center_x: i32,
    center_z: i32,
    generation_radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
) -> Result<(), Box<dyn Error>> {
    let load_min_x = (center_x - generation_radius).max(min_chunk.0);
    let load_max_x = (center_x + generation_radius).min(max_chunk.0);
    let load_min_y = min_y_chunk.max(min_chunk.1);
    let load_max_y = max_y_chunk.min(max_chunk.1);
    let load_min_z = (center_z - generation_radius).max(min_chunk.2);
    let load_max_z = (center_z + generation_radius).min(max_chunk.2);
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

fn generate_preview_chunks(
    world: &mut WorldCore,
    registry: &BlockRegistry,
    center_x: i32,
    center_z: i32,
    generation_radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
) {
    let meta = *world.meta();
    let input_cache =
        PreviewInputCache::for_preview_window(&meta, center_x, center_z, generation_radius);
    let generated_chunks = preview_chunk_coords(
        center_x,
        center_z,
        generation_radius,
        min_y_chunk,
        max_y_chunk,
    )
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
    fn for_preview_window(
        meta: &WorldMeta,
        center_x: i32,
        center_z: i32,
        generation_radius: i32,
    ) -> Self {
        let mut representatives = HashMap::<PreviewInputCacheKey, ChunkCoord>::new();
        for (chunk_x, chunk_z) in preview_chunk_xz_coords(center_x, center_z, generation_radius) {
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

#[derive(Debug, Clone, Copy)]
struct PrototypePreviewCell {
    min_x: i32,
    min_z: i32,
    top_y: i32,
    base_height: f32,
    relief_budget: f32,
    water_surface_height: Option<f32>,
    hydrology_mode: HydrologyMode,
    saturation: f32,
}

#[derive(Debug, Clone)]
struct PrototypePreviewGrid {
    origin_x: i32,
    origin_z: i32,
    width: usize,
    depth: usize,
    base_y: i32,
    cells: Vec<PrototypePreviewCell>,
}

impl PrototypePreviewGrid {
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

    fn cell(&self, world_x: i32, world_z: i32) -> Option<PrototypePreviewCell> {
        self.index_of(world_x, world_z)
            .map(|index| self.cells[index])
    }

    fn top_y_or_base(&self, world_x: i32, world_z: i32) -> i32 {
        self.cell(world_x, world_z)
            .map(|cell| cell.top_y.max(self.base_y))
            .unwrap_or(self.base_y)
    }

    fn water_top_y_or_terrain(&self, world_x: i32, world_z: i32) -> i32 {
        self.cell(world_x, world_z)
            .map(|cell| {
                if let Some(water_surface_height) = cell.water_surface_height {
                    water_surface_preview_top_y(cell, water_surface_height, self.base_y)
                } else {
                    cell.top_y.max(self.base_y)
                }
            })
            .unwrap_or(self.base_y)
    }
}

#[derive(Debug, Clone)]
struct PrototypePreviewChunk {
    coord: ChunkCoord,
    columns: Vec<PrototypePreviewColumn>,
}

#[derive(Debug, Clone, Copy)]
struct PrototypePreviewColumn {
    height: f32,
    remaining_relief_budget: f32,
    water_surface_height: Option<f32>,
    hydrology_mode: HydrologyMode,
    saturation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ChunkSurfaceSummary {
    min_y: i32,
    max_y: i32,
    mean_y: f32,
    column_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct HydrologyChunkSummary {
    water_columns: usize,
    channel_columns: usize,
    floodplain_columns: usize,
    lake_columns: usize,
    wetland_columns: usize,
}

#[derive(Debug, Clone, Copy)]
struct CenterChunkGeneratorDiagnostics {
    atlas_coord: AtlasCoord,
    atlas_cell: AtlasCell,
    region: RegionClassSample,
    meso: MesoGuideSample,
    archetype_summary: Option<&'static str>,
    allowed_meso_keys: &'static [&'static str],
}

fn collect_prototype_render_meshes_from_grid(
    grid: &PrototypePreviewGrid,
    registry: &BlockRegistry,
    center_x: i32,
    center_z: i32,
    radius: i32,
) -> Vec<RenderCpuMesh> {
    preview_chunk_xz_coords(center_x, center_z, radius)
        .into_par_iter()
        .filter_map(|(chunk_x, chunk_z)| {
            let mesh = build_prototype_heightfield_mesh(&grid, chunk_x, chunk_z, registry);
            (!mesh.vertices.is_empty() && !mesh.indices.is_empty()).then_some(mesh)
        })
        .collect()
}

fn build_prototype_preview_grid(
    meta: &WorldMeta,
    center_x: i32,
    center_z: i32,
    generation_radius: i32,
) -> Result<PrototypePreviewGrid, Box<dyn Error>> {
    build_stage_preview_grid(
        meta,
        center_x,
        center_z,
        generation_radius,
        PreviewStage::Prototype,
    )
}

fn build_hydrology_preview_grid(
    meta: &WorldMeta,
    center_x: i32,
    center_z: i32,
    generation_radius: i32,
) -> Result<PrototypePreviewGrid, Box<dyn Error>> {
    build_stage_preview_grid(
        meta,
        center_x,
        center_z,
        generation_radius,
        PreviewStage::Hydrology,
    )
}

fn build_stage_preview_grid(
    meta: &WorldMeta,
    center_x: i32,
    center_z: i32,
    generation_radius: i32,
    stage: PreviewStage,
) -> Result<PrototypePreviewGrid, Box<dyn Error>> {
    let min_chunk_x = center_x - generation_radius;
    let max_chunk_x = center_x + generation_radius;
    let min_chunk_z = center_z - generation_radius;
    let max_chunk_z = center_z + generation_radius;
    let width_chunks = usize::try_from(max_chunk_x - min_chunk_x + 1)
        .map_err(|_| cli_error("invalid prototype preview width"))?;
    let depth_chunks = usize::try_from(max_chunk_z - min_chunk_z + 1)
        .map_err(|_| cli_error("invalid prototype preview depth"))?;
    let width = width_chunks * CHUNK_EDGE;
    let depth = depth_chunks * CHUNK_EDGE;
    let origin_x = min_chunk_x * CHUNK_EDGE_I32;
    let origin_z = min_chunk_z * CHUNK_EDGE_I32;
    let mut cells = vec![
        PrototypePreviewCell {
            min_x: origin_x,
            min_z: origin_z,
            top_y: WORLD_FLOOR_Y,
            base_height: WORLD_FLOOR_Y as f32,
            relief_budget: 0.0,
            water_surface_height: None,
            hydrology_mode: HydrologyMode::Dry,
            saturation: 0.0,
        };
        width * depth
    ];
    let mut base_y = i32::MAX;
    let input_cache =
        PreviewInputCache::for_preview_window(meta, center_x, center_z, generation_radius);
    let prototypes = preview_chunk_xz_coords(center_x, center_z, generation_radius)
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
                    .map(|column| PrototypePreviewColumn {
                        height: column.height,
                        remaining_relief_budget: column.remaining_relief_budget,
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
                        .zip(smoothed.columns.into_iter())
                        .map(|(column, smoothed)| PrototypePreviewColumn {
                            height: column.terrain_height,
                            remaining_relief_budget: smoothed.remaining_relief_budget,
                            water_surface_height: column.water_surface_height,
                            hydrology_mode: column.mode,
                            saturation: column.saturation,
                        })
                        .collect()
                }
                PreviewStage::Full => unreachable!(
                    "stage preview grids are only used for prototype and hydrology previews"
                ),
            };

            PrototypePreviewChunk {
                coord: chunk,
                columns,
            }
        })
        .collect::<Vec<_>>();

    for preview_chunk in prototypes {
        let chunk_row = usize::try_from(preview_chunk.coord.2 - min_chunk_z)
            .map_err(|_| cli_error("invalid prototype preview chunk row"))?;
        let chunk_col = usize::try_from(preview_chunk.coord.0 - min_chunk_x)
            .map_err(|_| cli_error("invalid prototype preview chunk column"))?;
        let chunk_origin_x = preview_chunk.coord.0 * CHUNK_EDGE_I32;
        let chunk_origin_z = preview_chunk.coord.2 * CHUNK_EDGE_I32;

        for local_z in 0..CHUNK_EDGE_I32 {
            let global_z = chunk_row * CHUNK_EDGE
                + usize::try_from(local_z)
                    .map_err(|_| cli_error("invalid prototype preview local z"))?;
            let row_offset = global_z * width;
            for local_x in 0..CHUNK_EDGE_I32 {
                let column_index = usize::try_from(local_z * CHUNK_EDGE_I32 + local_x)
                    .map_err(|_| cli_error("invalid prototype preview column index"))?;
                let column = preview_chunk.columns[column_index];
                let global_x = chunk_col * CHUNK_EDGE
                    + usize::try_from(local_x)
                        .map_err(|_| cli_error("invalid prototype preview local x"))?;
                let index = row_offset + global_x;
                let top_y = column.height.round() as i32;
                base_y = base_y.min(top_y);
                cells[index] = PrototypePreviewCell {
                    min_x: chunk_origin_x + local_x,
                    min_z: chunk_origin_z + local_z,
                    top_y,
                    base_height: column.height,
                    relief_budget: column.remaining_relief_budget,
                    water_surface_height: column.water_surface_height,
                    hydrology_mode: column.hydrology_mode,
                    saturation: column.saturation,
                };
            }
        }
    }

    Ok(PrototypePreviewGrid {
        origin_x,
        origin_z,
        width,
        depth,
        base_y: base_y.saturating_sub(16).max(WORLD_FLOOR_Y),
        cells,
    })
}

fn build_prototype_heightfield_mesh(
    grid: &PrototypePreviewGrid,
    chunk_x: i32,
    chunk_z: i32,
    registry: &BlockRegistry,
) -> RenderCpuMesh {
    let mut mesh = RenderCpuMesh::default();
    let Some(block_id) = registry.block_id("terrain_debug") else {
        return mesh;
    };
    let block_def = registry.block_or_missing(block_id);
    let texture_layer = u32::from(block_def.texture_for_face(BlockFace::PosY).0);
    let material_kind = render_material_kind_from_world(block_def.material).as_u32();
    let water_block = registry
        .block_id("water")
        .map(|id| registry.block_or_missing(id))
        .map(|block| {
            (
                u32::from(block.texture_for_face(BlockFace::PosY).0),
                render_material_kind_from_world(block.material).as_u32(),
                block.tint_as_linear_rgba(),
            )
        });
    let chunk_origin_x = chunk_x * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk_z * CHUNK_EDGE_I32;

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let Some(cell) = grid.cell(world_x, world_z) else {
                continue;
            };
            let top_y = cell.top_y.max(grid.base_y + 1);
            if top_y <= grid.base_y {
                continue;
            }

            let color = prototype_column_color(block_def.tint_as_linear_rgba(), cell, grid.base_y);
            append_box_face(
                &mut mesh,
                cell.min_x,
                grid.base_y,
                cell.min_z,
                cell.min_x + 1,
                top_y,
                cell.min_z + 1,
                BlockFace::PosY,
                color,
                texture_layer,
                material_kind,
                grid.base_y,
                DEFAULT_PROTOTYPE_VERTICAL_EXAGGERATION,
            );

            let left_top = grid.top_y_or_base(world_x - 1, world_z);
            if top_y > left_top {
                append_box_face(
                    &mut mesh,
                    cell.min_x,
                    left_top,
                    cell.min_z,
                    cell.min_x + 1,
                    top_y,
                    cell.min_z + 1,
                    BlockFace::NegX,
                    color,
                    texture_layer,
                    material_kind,
                    grid.base_y,
                    DEFAULT_PROTOTYPE_VERTICAL_EXAGGERATION,
                );
            }

            let right_top = grid.top_y_or_base(world_x + 1, world_z);
            if top_y > right_top {
                append_box_face(
                    &mut mesh,
                    cell.min_x,
                    right_top,
                    cell.min_z,
                    cell.min_x + 1,
                    top_y,
                    cell.min_z + 1,
                    BlockFace::PosX,
                    color,
                    texture_layer,
                    material_kind,
                    grid.base_y,
                    DEFAULT_PROTOTYPE_VERTICAL_EXAGGERATION,
                );
            }

            let back_top = grid.top_y_or_base(world_x, world_z - 1);
            if top_y > back_top {
                append_box_face(
                    &mut mesh,
                    cell.min_x,
                    back_top,
                    cell.min_z,
                    cell.min_x + 1,
                    top_y,
                    cell.min_z + 1,
                    BlockFace::NegZ,
                    color,
                    texture_layer,
                    material_kind,
                    grid.base_y,
                    DEFAULT_PROTOTYPE_VERTICAL_EXAGGERATION,
                );
            }

            let front_top = grid.top_y_or_base(world_x, world_z + 1);
            if top_y > front_top {
                append_box_face(
                    &mut mesh,
                    cell.min_x,
                    front_top,
                    cell.min_z,
                    cell.min_x + 1,
                    top_y,
                    cell.min_z + 1,
                    BlockFace::PosZ,
                    color,
                    texture_layer,
                    material_kind,
                    grid.base_y,
                    DEFAULT_PROTOTYPE_VERTICAL_EXAGGERATION,
                );
            }

            if let Some(water_surface_height) = cell.water_surface_height {
                let water_top =
                    water_surface_preview_top_y(cell, water_surface_height, grid.base_y);
                if let Some((water_texture_layer, water_material_kind, water_base)) = water_block {
                    let water_color = water_preview_color(water_base, cell, water_surface_height);
                    append_box_face(
                        &mut mesh,
                        cell.min_x,
                        top_y,
                        cell.min_z,
                        cell.min_x + 1,
                        water_top,
                        cell.min_z + 1,
                        BlockFace::PosY,
                        water_color,
                        water_texture_layer,
                        water_material_kind,
                        grid.base_y,
                        DEFAULT_PROTOTYPE_VERTICAL_EXAGGERATION,
                    );

                    let left_top = grid.water_top_y_or_terrain(world_x - 1, world_z);
                    if water_top > left_top {
                        append_box_face(
                            &mut mesh,
                            cell.min_x,
                            left_top,
                            cell.min_z,
                            cell.min_x + 1,
                            water_top,
                            cell.min_z + 1,
                            BlockFace::NegX,
                            water_color,
                            water_texture_layer,
                            water_material_kind,
                            grid.base_y,
                            DEFAULT_PROTOTYPE_VERTICAL_EXAGGERATION,
                        );
                    }

                    let right_top = grid.water_top_y_or_terrain(world_x + 1, world_z);
                    if water_top > right_top {
                        append_box_face(
                            &mut mesh,
                            cell.min_x,
                            right_top,
                            cell.min_z,
                            cell.min_x + 1,
                            water_top,
                            cell.min_z + 1,
                            BlockFace::PosX,
                            water_color,
                            water_texture_layer,
                            water_material_kind,
                            grid.base_y,
                            DEFAULT_PROTOTYPE_VERTICAL_EXAGGERATION,
                        );
                    }

                    let back_top = grid.water_top_y_or_terrain(world_x, world_z - 1);
                    if water_top > back_top {
                        append_box_face(
                            &mut mesh,
                            cell.min_x,
                            back_top,
                            cell.min_z,
                            cell.min_x + 1,
                            water_top,
                            cell.min_z + 1,
                            BlockFace::NegZ,
                            water_color,
                            water_texture_layer,
                            water_material_kind,
                            grid.base_y,
                            DEFAULT_PROTOTYPE_VERTICAL_EXAGGERATION,
                        );
                    }

                    let front_top = grid.water_top_y_or_terrain(world_x, world_z + 1);
                    if water_top > front_top {
                        append_box_face(
                            &mut mesh,
                            cell.min_x,
                            front_top,
                            cell.min_z,
                            cell.min_x + 1,
                            water_top,
                            cell.min_z + 1,
                            BlockFace::PosZ,
                            water_color,
                            water_texture_layer,
                            water_material_kind,
                            grid.base_y,
                            DEFAULT_PROTOTYPE_VERTICAL_EXAGGERATION,
                        );
                    }
                }
            }
        }
    }

    mesh
}

fn prototype_column_color(base: [f32; 4], cell: PrototypePreviewCell, base_y: i32) -> [f32; 4] {
    let height_ratio = ((cell.base_height - base_y as f32) / 256.0).clamp(0.0, 1.0);
    let relief_ratio = ((cell.relief_budget - 4.0) / 36.0).clamp(0.0, 1.0);
    let mut color = base;
    color[0] = (color[0] * (0.78 + height_ratio * 0.18) + relief_ratio * 0.08).clamp(0.0, 1.0);
    color[1] = (color[1] * (0.82 + height_ratio * 0.10) + relief_ratio * 0.05).clamp(0.0, 1.0);
    color[2] = (color[2] * (0.86 + relief_ratio * 0.08)).clamp(0.0, 1.0);

    match cell.hydrology_mode {
        HydrologyMode::Dry => {}
        HydrologyMode::Channel => {
            color[0] *= 0.88;
            color[1] *= 0.94;
            color[2] = (color[2] * 1.08 + 0.04).clamp(0.0, 1.0);
        }
        HydrologyMode::Floodplain => {
            color[0] *= 0.92;
            color[1] = (color[1] * 0.96 + 0.03).clamp(0.0, 1.0);
            color[2] = (color[2] * 1.02 + 0.02).clamp(0.0, 1.0);
        }
        HydrologyMode::Lake | HydrologyMode::Wetland => {
            color[0] *= 0.86;
            color[1] = (color[1] * 0.94 + 0.04).clamp(0.0, 1.0);
            color[2] = (color[2] * 1.10 + 0.06).clamp(0.0, 1.0);
        }
    }
    color[1] = (color[1] + cell.saturation * 0.04).clamp(0.0, 1.0);
    color[2] = (color[2] + cell.saturation * 0.06).clamp(0.0, 1.0);
    color
}

fn water_preview_color(
    base: [f32; 4],
    cell: PrototypePreviewCell,
    water_surface_height: f32,
) -> [f32; 4] {
    let mut color = base;
    let altitude_ratio = ((water_surface_height - cell.base_height) / 8.0).clamp(0.0, 1.0);

    match cell.hydrology_mode {
        HydrologyMode::Channel => {
            color[0] = (color[0] * 0.78).clamp(0.0, 1.0);
            color[1] = (color[1] * 0.90 + 0.05).clamp(0.0, 1.0);
            color[2] = (color[2] * 1.12 + 0.08).clamp(0.0, 1.0);
        }
        HydrologyMode::Floodplain => {
            color[0] = (color[0] * 0.86).clamp(0.0, 1.0);
            color[1] = (color[1] * 0.96 + 0.04).clamp(0.0, 1.0);
            color[2] = (color[2] * 1.08 + 0.05).clamp(0.0, 1.0);
        }
        HydrologyMode::Lake => {
            color[0] = (color[0] * 0.72).clamp(0.0, 1.0);
            color[1] = (color[1] * 0.88 + 0.06).clamp(0.0, 1.0);
            color[2] = (color[2] * 1.15 + 0.10).clamp(0.0, 1.0);
        }
        HydrologyMode::Wetland => {
            color[0] = (color[0] * 0.84).clamp(0.0, 1.0);
            color[1] = (color[1] * 0.98 + 0.05).clamp(0.0, 1.0);
            color[2] = (color[2] * 1.04 + 0.04).clamp(0.0, 1.0);
        }
        HydrologyMode::Dry => {}
    }

    color[0] = (color[0] + altitude_ratio * 0.02).clamp(0.0, 1.0);
    color[1] = (color[1] + altitude_ratio * 0.03).clamp(0.0, 1.0);
    color[2] = (color[2] + altitude_ratio * 0.05).clamp(0.0, 1.0);
    color[3] = 0.92;
    color
}

fn water_surface_preview_top_y(
    cell: PrototypePreviewCell,
    water_surface_height: f32,
    base_y: i32,
) -> i32 {
    let terrain_top = cell.top_y.max(base_y);
    (water_surface_height.ceil() as i32)
        .max(terrain_top + 1)
        .max(base_y + 1)
}

fn collect_center_chunk_generator_diagnostics(
    chunk: ChunkCoord,
    meta: &WorldMeta,
) -> Result<CenterChunkGeneratorDiagnostics, Box<dyn Error>> {
    let scaffold = build_chunk_generation_scaffold(chunk, meta);
    let (center_world_x, center_world_z) = center_chunk_sample_world_xz(chunk);
    let atlas_coord = atlas_coord_for_world_xz(center_world_x, center_world_z);
    let atlas_cell = scaffold
        .inputs
        .atlas_fields
        .get(atlas_coord)
        .copied()
        .ok_or_else(|| {
            cli_error(format!(
                "center atlas coord ({}, {}) fell outside the scaffold atlas field map",
                atlas_coord.x, atlas_coord.z
            ))
        })?;
    let meso = sample_meso_guides(&scaffold.inputs.meso_guides, center_world_x, center_world_z);
    let archetype_def = region_archetype_def(scaffold.center_region.archetype);

    Ok(CenterChunkGeneratorDiagnostics {
        atlas_coord,
        atlas_cell,
        region: scaffold.center_region,
        meso,
        archetype_summary: archetype_def.map(|def| def.summary),
        allowed_meso_keys: archetype_def
            .map(|def| def.allowed_meso_keys)
            .unwrap_or(&[]),
    })
}

fn summarize_prototype_chunk_surface(
    grid: &PrototypePreviewGrid,
    chunk_x: i32,
    chunk_z: i32,
) -> Option<ChunkSurfaceSummary> {
    let chunk_origin_x = chunk_x * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk_z * CHUNK_EDGE_I32;
    let mut heights = Vec::with_capacity(CHUNK_EDGE * CHUNK_EDGE);

    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            heights.push(grid.cell(world_x, world_z)?.top_y);
        }
    }

    summarize_surface_heights(heights)
}

fn summarize_hydrology_chunk(
    grid: &PrototypePreviewGrid,
    chunk_x: i32,
    chunk_z: i32,
) -> Option<HydrologyChunkSummary> {
    let chunk_origin_x = chunk_x * CHUNK_EDGE_I32;
    let chunk_origin_z = chunk_z * CHUNK_EDGE_I32;
    let mut water_columns = 0usize;
    let mut channel_columns = 0usize;
    let mut floodplain_columns = 0usize;
    let mut lake_columns = 0usize;
    let mut wetland_columns = 0usize;
    for local_z in 0..CHUNK_EDGE_I32 {
        for local_x in 0..CHUNK_EDGE_I32 {
            let world_x = chunk_origin_x + local_x;
            let world_z = chunk_origin_z + local_z;
            let cell = grid.cell(world_x, world_z)?;

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
    }

    Some(HydrologyChunkSummary {
        water_columns,
        channel_columns,
        floodplain_columns,
        lake_columns,
        wetland_columns,
    })
}

fn summarize_realized_chunk_surface(
    world: &WorldCore,
    registry: &BlockRegistry,
    chunk: ChunkCoord,
    min_y_chunk: i32,
    max_y_chunk: i32,
) -> Option<ChunkSurfaceSummary> {
    let min_world_x = chunk.0 * CHUNK_EDGE_I32;
    let min_world_z = chunk.2 * CHUNK_EDGE_I32;
    let min_world_y = (min_y_chunk * CHUNK_EDGE_I32).max(WORLD_FLOOR_Y);
    let max_world_y = max_y_chunk
        .saturating_add(1)
        .saturating_mul(CHUNK_EDGE_I32)
        .saturating_sub(1)
        .max(min_world_y);
    let columns = sample_topdown_columns(
        world,
        registry,
        min_world_x,
        min_world_z,
        CHUNK_EDGE as u32,
        CHUNK_EDGE as u32,
        min_world_y,
        max_world_y,
    );

    summarize_surface_heights(
        columns
            .into_iter()
            .filter_map(|column| column.visible.top_y),
    )
}

fn summarize_surface_heights<I>(heights: I) -> Option<ChunkSurfaceSummary>
where
    I: IntoIterator<Item = i32>,
{
    let mut heights = heights.into_iter();
    let first = heights.next()?;
    let mut min_y = first;
    let mut max_y = first;
    let mut sum_y = i64::from(first);
    let mut column_count = 1_usize;

    for height in heights {
        min_y = min_y.min(height);
        max_y = max_y.max(height);
        sum_y += i64::from(height);
        column_count += 1;
    }

    Some(ChunkSurfaceSummary {
        min_y,
        max_y,
        mean_y: sum_y as f32 / column_count as f32,
        column_count,
    })
}

fn center_chunk_sample_world_xz(chunk: ChunkCoord) -> (i32, i32) {
    (
        chunk.0 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32.div_euclid(2),
        chunk.2 * CHUNK_EDGE_I32 + CHUNK_EDGE_I32.div_euclid(2),
    )
}

fn atlas_coord_for_world_xz(world_x: i32, world_z: i32) -> AtlasCoord {
    let atlas_span_blocks = (ATLAS_CELL_SIZE_IN_CHUNKS as i32 * CHUNK_EDGE_I32).max(1);
    AtlasCoord::new(
        world_x.div_euclid(atlas_span_blocks),
        world_z.div_euclid(atlas_span_blocks),
    )
}

fn print_center_chunk_diagnostics(
    diagnostics: &CenterChunkGeneratorDiagnostics,
    surface_summary: Option<ChunkSurfaceSummary>,
    hydrology_summary: Option<HydrologyChunkSummary>,
) {
    println!("center chunk diagnostics:");
    match surface_summary {
        Some(surface) => println!(
            "  surface_y: avg={:.2}, min={}, max={}, sampled_columns={}",
            surface.mean_y, surface.min_y, surface.max_y, surface.column_count
        ),
        None => println!("  surface_y: unavailable for the current preview path"),
    }
    if let Some(hydrology) = hydrology_summary {
        println!(
            "  hydrology: water_columns={}, channels={}, floodplain={}, lakes={}, wetlands={}",
            hydrology.water_columns,
            hydrology.channel_columns,
            hydrology.floodplain_columns,
            hydrology.lake_columns,
            hydrology.wetland_columns,
        );
    }
    println!(
        "  region: archetype={:?}, biome={:?}, terrain={:?}, climate={:?}, hydrology={:?}, coastal={:?}",
        diagnostics.region.archetype,
        diagnostics.region.biome_family,
        diagnostics.region.terrain_form_family,
        diagnostics.region.climate_regime,
        diagnostics.region.hydrology_context,
        diagnostics.region.coastal_context,
    );
    println!(
        "  region axes: temperature={:?}, moisture={:?}, elevation={:?}, relief={:?}",
        diagnostics.region.temperature_band,
        diagnostics.region.moisture_band,
        diagnostics.region.elevation_band,
        diagnostics.region.relief_class,
    );
    if let Some(summary) = diagnostics.archetype_summary {
        println!("  archetype summary: {summary}");
    }
    if !diagnostics.allowed_meso_keys.is_empty() {
        println!(
            "  allowed meso: {}",
            diagnostics.allowed_meso_keys.join(", ")
        );
    }
    println!(
        "  atlas cell: ({}, {}), landness={:.3}, macro={:.3}, coast_distance={:.3}, ridge={:.3}, mountain={:.3}, basin={:.3}, river_flow={:.3}",
        diagnostics.atlas_coord.x,
        diagnostics.atlas_coord.z,
        diagnostics.atlas_cell.landness,
        diagnostics.atlas_cell.macro_elevation,
        diagnostics.atlas_cell.coast_distance,
        diagnostics.atlas_cell.ridge_factor,
        diagnostics.atlas_cell.mountain_mass,
        diagnostics.atlas_cell.basinness,
        diagnostics.atlas_cell.river_flow_potential,
    );
    println!(
        "  atlas climate: temperature={:.3}, humidity={:.3}, inlandness={:.3}, aridity={:.3}, wetness={:.3}",
        diagnostics.atlas_cell.temperature,
        diagnostics.atlas_cell.humidity,
        diagnostics.atlas_cell.inlandness,
        diagnostics.atlas_cell.aridity,
        diagnostics.atlas_cell.wetness,
    );
    println!(
        "  meso sample: hilliness={:.3}, hill_height={:.2}, basin_weight={:.3}, basin_depth={:.2}, escarpment_weight={:.3}, escarpment_height={:.2}, terrace_weight={:.3}, terrace_step_height={:.2}",
        diagnostics.meso.hilliness,
        diagnostics.meso.hill_height,
        diagnostics.meso.basin_weight,
        diagnostics.meso.basin_depth,
        diagnostics.meso.escarpment_weight,
        diagnostics.meso.escarpment_height,
        diagnostics.meso.terrace_weight,
        diagnostics.meso.terrace_step_height,
    );
}

fn collect_render_meshes(
    world: &WorldCore,
    registry: &BlockRegistry,
    center_x: i32,
    center_z: i32,
    radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
) -> Vec<RenderCpuMesh> {
    preview_chunk_coords(center_x, center_z, radius, min_y_chunk, max_y_chunk)
        .into_par_iter()
        .filter_map(|coord| {
            let snapshot = world.snapshot_chunk(coord)?;
            let mesh = build_chunk_mesh(&snapshot, world.query_neighbors(coord), registry);
            (!mesh.vertices.is_empty() && !mesh.indices.is_empty())
                .then(|| world_mesh_to_render(mesh))
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct LodSurfaceCell {
    min_x: i32,
    min_z: i32,
    top_y: i32,
    profile: TerrainProfile,
}

fn collect_lod_render_meshes(
    meta: &WorldMeta,
    registry: &BlockRegistry,
    center_x: i32,
    center_z: i32,
    radius: i32,
    min_y_chunk: i32,
    lod_blocks: u8,
) -> Result<Vec<RenderCpuMesh>, Box<dyn Error>> {
    let chunk_span = radius * 2 + 1;
    let samples_per_chunk = CHUNK_EDGE / usize::from(lod_blocks);
    let grid_width = usize::try_from(chunk_span)
        .map_err(|_| cli_error("radius produced an invalid chunk span"))?
        * samples_per_chunk;
    let grid_depth = grid_width;
    let base_y = (min_y_chunk * CHUNK_EDGE_I32).max(WORLD_FLOOR_Y);
    let mut cells = Vec::with_capacity(grid_width * grid_depth);
    let row_grids = preview_chunk_xz_coords(center_x, center_z, radius)
        .into_par_iter()
        .map(|(chunk_x, chunk_z)| {
            sample_chunk_surface_lod(ChunkCoord(chunk_x, 0, chunk_z), lod_blocks, meta)
        })
        .collect::<Vec<_>>();

    for sample_z in 0..samples_per_chunk {
        for grid in &row_grids {
            let row_start = sample_z * usize::from(grid.samples_per_axis);
            let row_end = row_start + usize::from(grid.samples_per_axis);

            for sample in &grid.samples[row_start..row_end] {
                cells.push(LodSurfaceCell {
                    min_x: sample.world_min_x,
                    min_z: sample.world_min_z,
                    top_y: sample.surface_y + 1,
                    profile: sample.profile,
                });
            }
        }
    }

    if cells.is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![build_lod_heightfield_mesh(
        &cells,
        grid_width,
        grid_depth,
        i32::from(lod_blocks),
        base_y,
        DEFAULT_LOD_VERTICAL_EXAGGERATION,
        registry,
    )])
}

fn build_lod_heightfield_mesh(
    cells: &[LodSurfaceCell],
    grid_width: usize,
    grid_depth: usize,
    cell_span_blocks: i32,
    base_y: i32,
    vertical_exaggeration: f32,
    registry: &BlockRegistry,
) -> RenderCpuMesh {
    let mut mesh = RenderCpuMesh::default();
    let Some(block_id) = registry.block_id("terrain_debug") else {
        return mesh;
    };
    let block_def = registry.block_or_missing(block_id);
    let texture_layer = u32::from(block_def.texture_for_face(BlockFace::PosY).0);
    let material_kind = render_material_kind_from_world(block_def.material).as_u32();

    for z in 0..grid_depth {
        for x in 0..grid_width {
            let cell = cells[z * grid_width + x];
            let top_y = cell.top_y.max(base_y + 1);
            if top_y <= base_y {
                continue;
            }

            let color = lod_profile_color(block_def.tint_as_linear_rgba(), cell.profile, top_y - 1);
            append_box_face(
                &mut mesh,
                cell.min_x,
                base_y,
                cell.min_z,
                cell.min_x + cell_span_blocks,
                top_y,
                cell.min_z + cell_span_blocks,
                BlockFace::PosY,
                color,
                texture_layer,
                material_kind,
                base_y,
                vertical_exaggeration,
            );

            if x > 0 {
                let neighbor_top = cells[z * grid_width + (x - 1)].top_y.max(base_y);
                if top_y > neighbor_top {
                    append_box_face(
                        &mut mesh,
                        cell.min_x,
                        neighbor_top,
                        cell.min_z,
                        cell.min_x + cell_span_blocks,
                        top_y,
                        cell.min_z + cell_span_blocks,
                        BlockFace::NegX,
                        color,
                        texture_layer,
                        material_kind,
                        base_y,
                        vertical_exaggeration,
                    );
                }
            }
            if x + 1 < grid_width {
                let neighbor_top = cells[z * grid_width + (x + 1)].top_y.max(base_y);
                if top_y > neighbor_top {
                    append_box_face(
                        &mut mesh,
                        cell.min_x,
                        neighbor_top,
                        cell.min_z,
                        cell.min_x + cell_span_blocks,
                        top_y,
                        cell.min_z + cell_span_blocks,
                        BlockFace::PosX,
                        color,
                        texture_layer,
                        material_kind,
                        base_y,
                        vertical_exaggeration,
                    );
                }
            }
            if z > 0 {
                let neighbor_top = cells[(z - 1) * grid_width + x].top_y.max(base_y);
                if top_y > neighbor_top {
                    append_box_face(
                        &mut mesh,
                        cell.min_x,
                        neighbor_top,
                        cell.min_z,
                        cell.min_x + cell_span_blocks,
                        top_y,
                        cell.min_z + cell_span_blocks,
                        BlockFace::NegZ,
                        color,
                        texture_layer,
                        material_kind,
                        base_y,
                        vertical_exaggeration,
                    );
                }
            }
            if z + 1 < grid_depth {
                let neighbor_top = cells[(z + 1) * grid_width + x].top_y.max(base_y);
                if top_y > neighbor_top {
                    append_box_face(
                        &mut mesh,
                        cell.min_x,
                        neighbor_top,
                        cell.min_z,
                        cell.min_x + cell_span_blocks,
                        top_y,
                        cell.min_z + cell_span_blocks,
                        BlockFace::PosZ,
                        color,
                        texture_layer,
                        material_kind,
                        base_y,
                        vertical_exaggeration,
                    );
                }
            }
        }
    }

    mesh
}

fn parse_preview_stage(value: String) -> Result<PreviewStage, Box<dyn Error>> {
    match value.to_ascii_lowercase().as_str() {
        "full" | "default" => Ok(PreviewStage::Full),
        "prototype" => Ok(PreviewStage::Prototype),
        "hydrology" => Ok(PreviewStage::Hydrology),
        other => Err(cli_error(format!(
            "unknown stage '{other}'; expected 'full', 'prototype', or 'hydrology'"
        ))),
    }
}

fn preview_stage_label(stage: PreviewStage) -> &'static str {
    match stage {
        PreviewStage::Full => "full",
        PreviewStage::Prototype => "prototype",
        PreviewStage::Hydrology => "hydrology",
    }
}

fn lod_profile_color(base: [f32; 4], profile: TerrainProfile, surface_y: i32) -> [f32; 4] {
    let profile_tint = match profile {
        TerrainProfile::DeepOcean => [0.64, 0.71, 0.82, 1.0],
        TerrainProfile::Shelf => [0.71, 0.77, 0.84, 1.0],
        TerrainProfile::Coast => [0.82, 0.79, 0.69, 1.0],
        TerrainProfile::Plain => [0.77, 0.80, 0.73, 1.0],
        TerrainProfile::Upland => [0.73, 0.74, 0.70, 1.0],
        TerrainProfile::Ridge => [0.86, 0.86, 0.89, 1.0],
    };
    let altitude_darkening = (surface_y as f32 / 96.0).clamp(-0.2, 0.3);
    let profile_mix = 0.26;
    let mut out = [0.0_f32; 4];
    for index in 0..3 {
        let mixed = base[index] * (1.0 - profile_mix) + profile_tint[index] * profile_mix;
        out[index] = (mixed * 0.72 * (1.0 - altitude_darkening * 0.20)).clamp(0.0, 1.0);
    }
    out[3] = base[3];
    out
}

fn append_box_face(
    mesh: &mut RenderCpuMesh,
    min_x: i32,
    min_y: i32,
    min_z: i32,
    max_x: i32,
    max_y: i32,
    max_z: i32,
    face: BlockFace,
    color: [f32; 4],
    texture_layer: u32,
    material_kind: u32,
    y_origin: i32,
    y_scale: f32,
) {
    if min_y >= max_y || min_x >= max_x || min_z >= max_z {
        return;
    }

    let positions = scaled_face_positions(
        min_x, min_y, min_z, max_x, max_y, max_z, face, y_origin, y_scale,
    );
    let base_index = mesh.vertices.len() as u32;
    let normal = face_normal(face);
    let uv = face_uvs();
    extend_render_bounds(&mut mesh.bounds, &positions);

    for (position, uv) in positions.into_iter().zip(uv) {
        mesh.vertices.push(new_world::renderer::MeshVertex {
            position,
            color,
            normal,
            uv,
            texture_layer,
            material_kind,
            contour_edges: 0,
        });
    }

    mesh.indices.extend_from_slice(&[
        base_index,
        base_index + 1,
        base_index + 2,
        base_index,
        base_index + 2,
        base_index + 3,
    ]);
}

fn face_uvs() -> [[f32; 2]; 4] {
    [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]
}

fn scaled_face_positions(
    min_x: i32,
    min_y: i32,
    min_z: i32,
    max_x: i32,
    max_y: i32,
    max_z: i32,
    face: BlockFace,
    y_origin: i32,
    y_scale: f32,
) -> [[f32; 3]; 4] {
    let transform_y = |y: i32| y_origin as f32 + (y - y_origin) as f32 * y_scale;
    let min = [min_x as f32, transform_y(min_y), min_z as f32];
    let max = [max_x as f32, transform_y(max_y), max_z as f32];

    match face {
        BlockFace::NegX => [
            [min[0], min[1], min[2]],
            [min[0], min[1], max[2]],
            [min[0], max[1], max[2]],
            [min[0], max[1], min[2]],
        ],
        BlockFace::PosX => [
            [max[0], min[1], max[2]],
            [max[0], min[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], max[1], max[2]],
        ],
        BlockFace::NegY => [
            [min[0], min[1], max[2]],
            [max[0], min[1], max[2]],
            [max[0], min[1], min[2]],
            [min[0], min[1], min[2]],
        ],
        BlockFace::PosY => [
            [min[0], max[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ],
        BlockFace::NegZ => [
            [max[0], min[1], min[2]],
            [min[0], min[1], min[2]],
            [min[0], max[1], min[2]],
            [max[0], max[1], min[2]],
        ],
        BlockFace::PosZ => [
            [min[0], min[1], max[2]],
            [max[0], min[1], max[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ],
    }
}

fn face_normal(face: BlockFace) -> [f32; 3] {
    match face {
        BlockFace::NegX => [-1.0, 0.0, 0.0],
        BlockFace::PosX => [1.0, 0.0, 0.0],
        BlockFace::NegY => [0.0, -1.0, 0.0],
        BlockFace::PosY => [0.0, 1.0, 0.0],
        BlockFace::NegZ => [0.0, 0.0, -1.0],
        BlockFace::PosZ => [0.0, 0.0, 1.0],
    }
}

fn extend_render_bounds(
    bounds: &mut Option<new_world::renderer::RenderBounds>,
    positions: &[[f32; 3]; 4],
) {
    for position in positions {
        match bounds {
            Some(bounds) => {
                bounds.min[0] = bounds.min[0].min(position[0]);
                bounds.min[1] = bounds.min[1].min(position[1]);
                bounds.min[2] = bounds.min[2].min(position[2]);
                bounds.max[0] = bounds.max[0].max(position[0]);
                bounds.max[1] = bounds.max[1].max(position[1]);
                bounds.max[2] = bounds.max[2].max(position[2]);
            }
            None => {
                *bounds = Some(new_world::renderer::RenderBounds {
                    min: *position,
                    max: *position,
                });
            }
        }
    }
}

fn build_preview_camera(
    bounds: new_world::renderer::RenderBounds,
    width: u32,
    height: u32,
    quarter_turns: u8,
) -> RenderCameraState {
    let aspect = if height == 0 {
        1.0
    } else {
        width as f32 / height as f32
    };
    let basis = quarter_view_basis(quarter_turns);
    let target = [
        (bounds.min[0] + bounds.max[0]) * 0.5,
        bounds.min[1] + (bounds.max[1] - bounds.min[1]) * 0.4,
        (bounds.min[2] + bounds.max[2]) * 0.5,
    ];
    let mut right_extent = 0.0_f32;
    let mut up_extent = 0.0_f32;

    for corner in bounds_corners(bounds) {
        let delta = [
            corner[0] - target[0],
            corner[1] - target[1],
            corner[2] - target[2],
        ];
        right_extent = right_extent.max(dot3(delta, basis.right).abs());
        up_extent = up_extent.max(dot3(delta, basis.up).abs());
    }

    let half_height =
        (up_extent.max(right_extent / aspect) * 1.15).max(QUARTER_VIEW_VERTICAL_WORLD_SIZE * 0.5);

    RenderCameraState {
        eye: quarter_view_eye(target, quarter_turns),
        target,
        up: basis.up,
        aspect_override: Some(aspect),
        projection_mode: RenderProjectionMode::Orthographic {
            vertical_world_size: half_height * 2.0,
        },
        basis_override: Some(RenderViewBasis {
            right: basis.right,
            up: basis.up,
            forward: basis.forward,
        }),
    }
}

fn block_registry_to_render_textures(registry: &BlockRegistry) -> RenderTextureArraySource {
    RenderTextureArraySource {
        tile_size: registry.tile_size(),
        tiles: registry
            .texture_tiles()
            .iter()
            .map(|tile| RenderTextureTile {
                layer: u32::from(tile.id.0),
                key: tile.key.clone(),
                source: match &tile.source {
                    TextureTileSource::BuiltinWhite => RenderTextureSource::BuiltinWhite,
                    TextureTileSource::File(path) => RenderTextureSource::File(path.clone()),
                },
            })
            .collect(),
    }
}

fn world_mesh_to_render(mesh: new_world::world::CpuMesh) -> RenderCpuMesh {
    RenderCpuMesh {
        vertices: mesh
            .vertices
            .into_iter()
            .map(world_vertex_to_render)
            .collect(),
        indices: mesh.indices,
        bounds: mesh.bounds.map(|bounds| new_world::renderer::RenderBounds {
            min: bounds.min,
            max: bounds.max,
        }),
    }
}

fn world_vertex_to_render(vertex: new_world::world::MeshVertex) -> new_world::renderer::MeshVertex {
    new_world::renderer::MeshVertex {
        position: vertex.position,
        color: vertex.color,
        normal: vertex.normal,
        uv: vertex.uv,
        texture_layer: vertex.texture_layer,
        material_kind: render_material_kind_from_world(vertex.material_kind).as_u32(),
        contour_edges: vertex.contour_edges,
    }
}

fn render_material_kind_from_world(
    kind: BlockMaterialKind,
) -> new_world::renderer::RenderMaterialKind {
    match kind {
        BlockMaterialKind::GenericOpaque => new_world::renderer::RenderMaterialKind::GenericOpaque,
        BlockMaterialKind::Grass => new_world::renderer::RenderMaterialKind::Grass,
        BlockMaterialKind::Soil => new_world::renderer::RenderMaterialKind::Soil,
        BlockMaterialKind::Stone => new_world::renderer::RenderMaterialKind::Stone,
        BlockMaterialKind::Sand => new_world::renderer::RenderMaterialKind::Sand,
        BlockMaterialKind::Foliage => new_world::renderer::RenderMaterialKind::Foliage,
        BlockMaterialKind::Water => new_world::renderer::RenderMaterialKind::Water,
        BlockMaterialKind::Emissive => new_world::renderer::RenderMaterialKind::Emissive,
    }
}

fn combined_render_bounds(meshes: &[RenderCpuMesh]) -> Option<new_world::renderer::RenderBounds> {
    let mut combined: Option<new_world::renderer::RenderBounds> = None;

    for mesh in meshes {
        let Some(bounds) = mesh.bounds else {
            continue;
        };
        combined = Some(match combined {
            Some(current) => new_world::renderer::RenderBounds {
                min: [
                    current.min[0].min(bounds.min[0]),
                    current.min[1].min(bounds.min[1]),
                    current.min[2].min(bounds.min[2]),
                ],
                max: [
                    current.max[0].max(bounds.max[0]),
                    current.max[1].max(bounds.max[1]),
                    current.max[2].max(bounds.max[2]),
                ],
            },
            None => bounds,
        });
    }

    combined
}

fn preview_environment(lod_preview: bool) -> RenderEnvironment {
    let mut environment = RenderEnvironment::sunset_quarter_view();
    environment.time_of_day_hours = 16.5;
    environment.sun_direction = normalize3([0.46, 0.72, -0.52]);
    environment.sun_color = [1.0, 0.95, 0.88];
    environment.sun_intensity = if lod_preview { 1.22 } else { 1.08 };
    environment.ambient_color = if lod_preview {
        [0.34, 0.39, 0.47]
    } else {
        [0.42, 0.48, 0.56]
    };
    environment.ambient_intensity = if lod_preview { 0.46 } else { 0.64 };
    environment.fog_color = if lod_preview {
        [0.70, 0.78, 0.88]
    } else {
        [0.74, 0.82, 0.92]
    };
    environment.fog_density = if lod_preview { 0.0014 } else { 0.0022 };
    environment.fog_height_falloff = if lod_preview { 0.014 } else { 0.020 };
    environment.sky_color = if lod_preview {
        [0.44, 0.58, 0.78]
    } else {
        [0.50, 0.66, 0.86]
    };
    environment.horizon_color = if lod_preview {
        [0.68, 0.76, 0.86]
    } else {
        [0.76, 0.84, 0.92]
    };
    environment.overcast = if lod_preview { 0.06 } else { 0.10 };
    environment.climate_tint = [1.0, 1.0, 1.0];
    environment.top_face_boost = if lod_preview { 0.01 } else { 0.04 };
    environment.side_shadow_strength = if lod_preview { 0.82 } else { 0.62 };
    environment.silhouette_boost = if lod_preview { 0.36 } else { 0.24 };
    environment.saturation_boost = if lod_preview { 0.12 } else { 0.08 };
    environment
}

fn bounds_corners(bounds: new_world::renderer::RenderBounds) -> [[f32; 3]; 8] {
    [
        [bounds.min[0], bounds.min[1], bounds.min[2]],
        [bounds.min[0], bounds.min[1], bounds.max[2]],
        [bounds.min[0], bounds.max[1], bounds.min[2]],
        [bounds.min[0], bounds.max[1], bounds.max[2]],
        [bounds.max[0], bounds.min[1], bounds.min[2]],
        [bounds.max[0], bounds.min[1], bounds.max[2]],
        [bounds.max[0], bounds.max[1], bounds.min[2]],
        [bounds.max[0], bounds.max[1], bounds.max[2]],
    ]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length_sq = dot3(vector, vector);
    if length_sq <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        let inv_length = length_sq.sqrt().recip();
        [
            vector[0] * inv_length,
            vector[1] * inv_length,
            vector[2] * inv_length,
        ]
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
    "usage: cargo run --bin chunk_preview -- <seed> [--stage <full|prototype|hydrology>] [--center-x <i32>] [--center-z <i32>] [--radius <i32>] [--quarter-turns <u8>] [--width <u32>] [--height <u32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--lod-blocks <u8>] [--output <path>]\n   or: cargo run --bin chunk_preview -- --world-dir <path> [--stage full] [--center-x <i32>] [--center-z <i32>] [--radius <i32>] [--quarter-turns <u8>] [--width <u32>] [--height <u32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--lod-blocks <u8>] [--output <path>]\n\nPrototype and hydrology stages are seed-only; prototype renders the post-prototype meso-applied heightfield, while hydrology renders the post-smoothing hydrology carve with water overlays."
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use new_world::world::{BlockRegistry, WorldMeta};

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
        assert!(parse_preview_stage("probe".to_string()).is_err());
    }

    #[test]
    fn prototype_stage_default_output_path_marks_the_stage() {
        let source = PreviewSource::Seed(42);
        let output = default_output_path(&source, 3, -2, 5, 1, PreviewStage::Prototype);
        let output = output.to_string_lossy();

        assert!(output.contains("seed_42_prototype_cx3_cz-2_r5.png"));
    }

    #[test]
    fn hydrology_stage_default_output_path_marks_the_stage() {
        let source = PreviewSource::Seed(42);
        let output = default_output_path(&source, 3, -2, 5, 1, PreviewStage::Hydrology);
        let output = output.to_string_lossy();

        assert!(output.contains("seed_42_hydrology_cx3_cz-2_r5.png"));
    }

    #[test]
    #[ignore = "slow preview smoke test that builds generation terrain meshes"]
    fn prototype_stage_collects_visible_meshes_from_a_seed() {
        let meta = WorldMeta::new(42);
        let registry = BlockRegistry::load_default().expect("default registry should load");
        let grid = build_prototype_preview_grid(&meta, 4, -3, 1)
            .expect("prototype preview grid should build");
        let meshes = collect_prototype_render_meshes_from_grid(&grid, &registry, 4, -3, 0);

        assert!(!meshes.is_empty());
        assert!(
            meshes
                .iter()
                .any(|mesh| !mesh.vertices.is_empty() && !mesh.indices.is_empty())
        );
        assert!(meshes.iter().all(|mesh| mesh.bounds.is_some()));
    }

    #[test]
    #[ignore = "slow preview smoke test that builds generation terrain grids"]
    fn prototype_stage_grid_matches_meso_applied_heights() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(3, 0, 24);
        let grid = build_prototype_preview_grid(&meta, chunk.0, chunk.2, 0)
            .expect("prototype grid should build");
        let scaffold = build_chunk_generation_scaffold(chunk, &meta);
        let prototype = build_chunk_base_heightfield_prototype(
            scaffold.chunk,
            &scaffold.inputs,
            &scaffold.realization_field_patch,
            &scaffold.corridor_window,
        );
        let meso = build_chunk_meso_applied_prototype(
            scaffold.chunk,
            &scaffold.inputs,
            &scaffold.corridor_window,
            &prototype,
        );
        let mut found_meso_delta = false;

        for local_z in 0..CHUNK_EDGE_I32 {
            for local_x in 0..CHUNK_EDGE_I32 {
                let index = local_z as usize * CHUNK_EDGE_I32 as usize + local_x as usize;
                let world_x = chunk.0 * CHUNK_EDGE_I32 + local_x;
                let world_z = chunk.2 * CHUNK_EDGE_I32 + local_z;
                let cell = grid
                    .cell(world_x, world_z)
                    .expect("prototype grid should cover the sampled chunk");
                let applied = meso.columns[index];
                let base = prototype.columns[index];

                assert!(
                    (cell.base_height - applied.height).abs() <= 0.001,
                    "prototype preview cell height drifted from meso-applied height at ({world_x}, {world_z})"
                );
                assert_eq!(cell.top_y, applied.height.round() as i32);
                assert!(
                    (cell.relief_budget - applied.remaining_relief_budget).abs() <= 0.001,
                    "prototype preview relief budget drifted at ({world_x}, {world_z})"
                );

                if (applied.height - base.base_height).abs() >= 0.10 {
                    found_meso_delta = true;
                }
            }
        }

        assert!(
            found_meso_delta,
            "expected the sampled prototype preview chunk to include meso deformation"
        );
    }

    #[test]
    #[ignore = "slow preview smoke test that builds generation terrain grids"]
    fn prototype_surface_summary_matches_grid_cells() {
        let meta = WorldMeta::new(42);
        let chunk = ChunkCoord(3, 0, 24);
        let grid = build_prototype_preview_grid(&meta, chunk.0, chunk.2, 0)
            .expect("prototype grid should build");
        let summary = summarize_prototype_chunk_surface(&grid, chunk.0, chunk.2)
            .expect("chunk should have a surface");
        let mut heights = Vec::with_capacity(CHUNK_EDGE * CHUNK_EDGE);

        for local_z in 0..CHUNK_EDGE_I32 {
            for local_x in 0..CHUNK_EDGE_I32 {
                let world_x = chunk.0 * CHUNK_EDGE_I32 + local_x;
                let world_z = chunk.2 * CHUNK_EDGE_I32 + local_z;
                heights.push(
                    grid.cell(world_x, world_z)
                        .expect("prototype grid should cover the sampled chunk")
                        .top_y,
                );
            }
        }

        let expected_min = *heights
            .iter()
            .min()
            .expect("height list should not be empty");
        let expected_max = *heights
            .iter()
            .max()
            .expect("height list should not be empty");
        let expected_mean =
            heights.iter().map(|height| *height as f32).sum::<f32>() / heights.len() as f32;

        assert_eq!(summary.min_y, expected_min);
        assert_eq!(summary.max_y, expected_max);
        assert_eq!(summary.column_count, CHUNK_EDGE * CHUNK_EDGE);
        assert!(
            (summary.mean_y - expected_mean).abs() <= 0.001,
            "prototype surface mean drifted from the preview grid"
        );
    }

    #[test]
    #[ignore = "slow preview hydrology search smoke test"]
    fn hydrology_stage_grid_contains_water_for_a_corridor_chunk() {
        let meta = WorldMeta::new(42);
        let seed_chunks = [
            ChunkCoord(40, 0, -29),
            ChunkCoord(39, 0, -29),
            ChunkCoord(40, 0, -30),
            ChunkCoord(40, 0, -28),
            ChunkCoord(28, 0, -22),
            ChunkCoord(29, 0, -22),
            ChunkCoord(15, 0, 15),
            ChunkCoord(16, 0, 16),
            ChunkCoord(0, 0, 0),
            ChunkCoord(31, 0, -20),
        ];

        for seed in seed_chunks {
            for offset_z in -2..=2 {
                for offset_x in -2..=2 {
                    let chunk = ChunkCoord(seed.0 + offset_x, 0, seed.2 + offset_z);
                    let grid = build_hydrology_preview_grid(&meta, chunk.0, chunk.2, 0)
                        .expect("hydrology grid should build");
                    let mut water_columns = 0usize;

                    for local_z in 0..CHUNK_EDGE_I32 {
                        for local_x in 0..CHUNK_EDGE_I32 {
                            let world_x = chunk.0 * CHUNK_EDGE_I32 + local_x;
                            let world_z = chunk.2 * CHUNK_EDGE_I32 + local_z;
                            let cell = grid
                                .cell(world_x, world_z)
                                .expect("hydrology grid should cover the sampled chunk");
                            if cell.water_surface_height.is_some() {
                                water_columns += 1;
                            }
                        }
                    }

                    if water_columns > 0 {
                        return;
                    }
                }
            }
        }

        panic!("expected at least one sampled hydrology preview chunk to contain visible water");
    }

    #[test]
    #[ignore = "slow preview diagnostics smoke test that builds generation scaffold"]
    fn center_chunk_generator_diagnostics_include_atlas_region_and_meso_samples() {
        let diagnostics =
            collect_center_chunk_generator_diagnostics(ChunkCoord(0, 0, 0), &WorldMeta::new(42))
                .expect("center chunk diagnostics should resolve");

        assert_eq!(diagnostics.atlas_coord, AtlasCoord::new(0, 0));
        assert!(diagnostics.atlas_cell.landness >= 0.0);
        assert!(diagnostics.atlas_cell.landness <= 1.0);
        assert!(diagnostics.archetype_summary.is_some());
        assert!(!diagnostics.allowed_meso_keys.is_empty());
        assert!(diagnostics.meso.hilliness >= 0.0);
        assert!(diagnostics.meso.terrace_weight >= 0.0);
    }
}
