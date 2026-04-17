use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use new_world::ecs::{
    QUARTER_VIEW_VERTICAL_WORLD_SIZE, quarter_view_basis, quarter_view_eye,
};
use new_world::renderer::{
    CpuMesh as RenderCpuMesh, OffscreenRenderRequest, RenderCameraState, RenderEnvironment,
    RenderProjectionMode, RenderTextureArraySource, RenderTextureSource, RenderTextureTile,
    RenderViewBasis, render_offscreen, write_offscreen_png,
};
use new_world::world::{
    BlockFace, BlockMaterialKind, BlockRegistry, CHUNK_EDGE, CHUNK_EDGE_I32, ChunkCoord,
    TerrainProfile, TextureTileSource, WORLD_FLOOR_Y, WorldCore, WorldMeta,
    build_chunk_base_heightfield_prototype, build_chunk_mesh, build_chunk_v2_scaffold,
    generate_chunk, sample_chunk_surface_lod,
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
    if lod_blocks == 0 {
        return Err(cli_error("lod-blocks must be >= 1"));
    }
    if CHUNK_EDGE % usize::from(lod_blocks) != 0 {
        return Err(cli_error(format!(
            "lod-blocks must evenly divide CHUNK_EDGE ({CHUNK_EDGE})"
        )));
    }
    if stage == PreviewStage::Prototype && lod_blocks > 1 {
        return Err(cli_error(
            "prototype stage currently supports only the default block resolution; omit --lod-blocks or use --lod-blocks 1",
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
    if stage == PreviewStage::Prototype {
        if created_world_dir.is_some() {
            return Err(cli_error(
                "prototype stage is currently supported only for direct seed previews",
            ));
        }
    }

    let render_meshes = match stage {
        PreviewStage::Prototype => collect_prototype_render_meshes(
            &meta,
            block_registry.as_ref(),
            center_x,
            center_z,
            radius,
            generation_radius,
        )?,
        PreviewStage::Full if lod_blocks > 1 => {
            if created_world_dir.is_some() {
                return Err(cli_error(
                    "lod-blocks > 1 is currently supported only for direct seed previews",
                ));
            }

            collect_lod_render_meshes(
                &meta,
                block_registry.as_ref(),
                center_x,
                center_z,
                radius,
                min_y_chunk,
                lod_blocks,
            )?
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

            collect_render_meshes(
                &world,
                block_registry.as_ref(),
                center_x,
                center_z,
                radius,
                min_y_chunk,
                max_y_chunk,
            )
        }
    };
    if render_meshes.is_empty() {
        return Err(cli_error("no visible meshes were produced for the requested preview area"));
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
            println!("render footprint: xz radius={}, prototype stage, y bounds ignored", radius);
        }
        PreviewStage::Full => {
            println!(
                "render footprint: xz radius={}, y={}..{}, lod_blocks={}",
                radius, min_y_chunk, max_y_chunk, lod_blocks
            );
        }
    }
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

    for chunk_y in load_min_y..=load_max_y {
        for chunk_z in load_min_z..=load_max_z {
            for chunk_x in load_min_x..=load_max_x {
                let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
                let chunk = load_chunk_from_dump(world_dir, coord)?;
                world.insert_chunk(coord, chunk);
            }
        }
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

fn generate_preview_chunks(
    world: &mut WorldCore,
    registry: &BlockRegistry,
    center_x: i32,
    center_z: i32,
    generation_radius: i32,
    min_y_chunk: i32,
    max_y_chunk: i32,
) {
    for chunk_y in min_y_chunk..=max_y_chunk {
        for chunk_z in (center_z - generation_radius)..=(center_z + generation_radius) {
            for chunk_x in (center_x - generation_radius)..=(center_x + generation_radius) {
                let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
                let chunk = generate_chunk(coord, world.meta(), registry);
                world.insert_chunk(coord, chunk);
            }
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
        self.index_of(world_x, world_z).map(|index| self.cells[index])
    }

    fn top_y_or_base(&self, world_x: i32, world_z: i32) -> i32 {
        self.cell(world_x, world_z)
            .map(|cell| cell.top_y.max(self.base_y + 1))
            .unwrap_or(self.base_y)
    }
}

fn collect_prototype_render_meshes(
    meta: &WorldMeta,
    registry: &BlockRegistry,
    center_x: i32,
    center_z: i32,
    radius: i32,
    generation_radius: i32,
) -> Result<Vec<RenderCpuMesh>, Box<dyn Error>> {
    let grid = build_prototype_preview_grid(meta, center_x, center_z, generation_radius)?;
    let mut render_meshes = Vec::new();

    for chunk_z in (center_z - radius)..=(center_z + radius) {
        for chunk_x in (center_x - radius)..=(center_x + radius) {
            let mesh = build_prototype_heightfield_mesh(&grid, chunk_x, chunk_z, registry);
            if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                continue;
            }
            render_meshes.push(mesh);
        }
    }

    Ok(render_meshes)
}

fn build_prototype_preview_grid(
    meta: &WorldMeta,
    center_x: i32,
    center_z: i32,
    generation_radius: i32,
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
        };
        width * depth
    ];
    let mut base_y = i32::MAX;

    for chunk_z in min_chunk_z..=max_chunk_z {
        let chunk_row = usize::try_from(chunk_z - min_chunk_z)
            .map_err(|_| cli_error("invalid prototype preview chunk row"))?;
        for chunk_x in min_chunk_x..=max_chunk_x {
            let scaffold = build_chunk_v2_scaffold(ChunkCoord(chunk_x, 0, chunk_z), meta);
            let prototype = build_chunk_base_heightfield_prototype(
                scaffold.chunk,
                &scaffold.inputs,
                &scaffold.corridor_window,
            );
            let chunk_col = usize::try_from(chunk_x - min_chunk_x)
                .map_err(|_| cli_error("invalid prototype preview chunk column"))?;
            let chunk_origin_x = chunk_x * CHUNK_EDGE_I32;
            let chunk_origin_z = chunk_z * CHUNK_EDGE_I32;

            for local_z in 0..CHUNK_EDGE_I32 {
                let global_z = chunk_row * CHUNK_EDGE + usize::try_from(local_z)
                    .map_err(|_| cli_error("invalid prototype preview local z"))?;
                let row_offset = global_z * width;
                for local_x in 0..CHUNK_EDGE_I32 {
                    let column_index = usize::try_from(local_z * CHUNK_EDGE_I32 + local_x)
                        .map_err(|_| cli_error("invalid prototype preview column index"))?;
                    let column = prototype.columns[column_index];
                    let global_x = chunk_col * CHUNK_EDGE + usize::try_from(local_x)
                        .map_err(|_| cli_error("invalid prototype preview local x"))?;
                    let index = row_offset + global_x;
                    let top_y = column.base_height.ceil() as i32;
                    base_y = base_y.min(top_y);
                    cells[index] = PrototypePreviewCell {
                        min_x: chunk_origin_x + local_x,
                        min_z: chunk_origin_z + local_z,
                        top_y,
                        base_height: column.base_height,
                        relief_budget: column.relief_budget,
                    };
                }
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
    color
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
    let mut render_meshes = Vec::new();

    for chunk_y in min_y_chunk..=max_y_chunk {
        for chunk_z in (center_z - radius)..=(center_z + radius) {
            for chunk_x in (center_x - radius)..=(center_x + radius) {
                let coord = ChunkCoord(chunk_x, chunk_y, chunk_z);
                let Some(snapshot) = world.snapshot_chunk(coord) else {
                    continue;
                };
                let mesh = build_chunk_mesh(&snapshot, world.query_neighbors(coord), registry);
                if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                    continue;
                }
                render_meshes.push(world_mesh_to_render(mesh));
            }
        }
    }

    render_meshes
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

    for chunk_z in (center_z - radius)..=(center_z + radius) {
        let row_grids = ((center_x - radius)..=(center_x + radius))
            .map(|chunk_x| sample_chunk_surface_lod(ChunkCoord(chunk_x, 0, chunk_z), lod_blocks, meta))
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
        other => Err(cli_error(format!(
            "unknown stage '{other}'; expected 'full' or 'prototype'"
        ))),
    }
}

fn preview_stage_label(stage: PreviewStage) -> &'static str {
    match stage {
        PreviewStage::Full => "full",
        PreviewStage::Prototype => "prototype",
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

fn extend_render_bounds(bounds: &mut Option<new_world::renderer::RenderBounds>, positions: &[[f32; 3]; 4]) {
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

    let half_height = (up_extent.max(right_extent / aspect) * 1.15)
        .max(QUARTER_VIEW_VERTICAL_WORLD_SIZE * 0.5);

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
        vertices: mesh.vertices.into_iter().map(world_vertex_to_render).collect(),
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

fn render_material_kind_from_world(kind: BlockMaterialKind) -> new_world::renderer::RenderMaterialKind {
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
    "usage: cargo run --bin chunk_preview -- <seed> [--stage <full|prototype>] [--center-x <i32>] [--center-z <i32>] [--radius <i32>] [--quarter-turns <u8>] [--width <u32>] [--height <u32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--lod-blocks <u8>] [--output <path>]\n   or: cargo run --bin chunk_preview -- --world-dir <path> [--stage full] [--center-x <i32>] [--center-z <i32>] [--radius <i32>] [--quarter-turns <u8>] [--width <u32>] [--height <u32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--lod-blocks <u8>] [--output <path>]\n\nPrototype stage is seed-only and renders the base heightfield / prototype solve instead of realized chunk meshes."
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use new_world::world::{BlockRegistry, WorldMeta};

    #[test]
    fn parse_preview_stage_accepts_full_and_prototype() {
        assert_eq!(parse_preview_stage("full".to_string()).unwrap(), PreviewStage::Full);
        assert_eq!(
            parse_preview_stage("prototype".to_string()).unwrap(),
            PreviewStage::Prototype
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
    fn prototype_stage_collects_visible_meshes_from_a_seed() {
        let meta = WorldMeta::new(42);
        let registry = BlockRegistry::load_default().expect("default registry should load");
        let meshes = collect_prototype_render_meshes(&meta, &registry, 4, -3, 0, 1)
            .expect("prototype preview should build");

        assert!(!meshes.is_empty());
        assert!(meshes
            .iter()
            .any(|mesh| !mesh.vertices.is_empty() && !mesh.indices.is_empty()));
        assert!(meshes.iter().all(|mesh| mesh.bounds.is_some()));
    }
}
