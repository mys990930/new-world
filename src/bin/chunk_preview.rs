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
    BlockMaterialKind, BlockRegistry, CHUNK_EDGE_I32, ChunkCoord, TextureTileSource, WORLD_FLOOR_Y,
    WorldCore, WorldMeta, build_chunk_mesh, generate_chunk,
};

#[path = "shared/world_dump_common.rs"]
mod world_dump_common;

use world_dump_common::{BakedWorldManifest, load_chunk_from_dump, read_manifest};

const DEFAULT_RENDER_RADIUS: i32 = 4;
const DEFAULT_RENDER_PADDING: i32 = 2;
const DEFAULT_IMAGE_WIDTH: u32 = 1600;
const DEFAULT_IMAGE_HEIGHT: u32 = 900;
const DEFAULT_MAX_Y_CHUNK: i32 = 3;

#[derive(Debug, Clone)]
enum PreviewSource {
    Seed(u64),
    BakedWorld(PathBuf),
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let source = if args.first().map(String::as_str) == Some("--world-dir") {
        args.remove(0);
        PreviewSource::BakedWorld(PathBuf::from(parse_required::<String>(&mut args, "world-dir")?))
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
    let mut min_y_chunk = WORLD_FLOOR_Y.div_euclid(CHUNK_EDGE_I32);
    let mut max_y_chunk = DEFAULT_MAX_Y_CHUNK;
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

    let block_registry = Arc::new(
        BlockRegistry::load_default()
            .map_err(|error| cli_error(format!("failed to load block registry: {error:?}")))?,
    );

    let (meta, baked_world_dir, baked_manifest) = match &source {
        PreviewSource::Seed(seed) => (WorldMeta::new(*seed), None, None),
        PreviewSource::BakedWorld(world_dir) => {
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
    } else if let Some(manifest) = baked_manifest.as_ref() {
        choose_baked_center(manifest, radius, min_y_chunk, max_y_chunk)?
    } else {
        (center_x, center_z)
    };
    center_x = requested_center.0;
    center_z = requested_center.1;

    let output = output.unwrap_or_else(|| default_output_path(&source, center_x, center_z, radius));
    let generation_radius = radius + DEFAULT_RENDER_PADDING;
    let mut world = WorldCore::new(meta, Arc::clone(&block_registry));

    match baked_world_dir.as_deref() {
        Some(world_dir) => {
            let manifest = baked_manifest
                .as_ref()
                .expect("baked preview metadata should exist");
            ensure_baked_bounds_cover_request(
                manifest.min_chunk_coord(),
                manifest.max_chunk_coord(),
                center_x,
                center_z,
                radius,
                min_y_chunk,
                max_y_chunk,
            )?;
            load_baked_preview_chunks(
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

    let render_meshes =
        collect_render_meshes(&world, block_registry.as_ref(), center_x, center_z, radius, min_y_chunk, max_y_chunk);
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
        environment: preview_environment(),
        chunk_meshes: render_meshes.clone(),
        clear_color_override: None,
    })?;
    write_offscreen_png(&output, &image)?;

    match &source {
        PreviewSource::Seed(seed) => println!("preview source: generated from seed {seed}"),
        PreviewSource::BakedWorld(world_dir) => {
            println!("preview source: baked world {}", world_dir.display())
        }
    }
    println!("center chunk: ({center_x}, {center_z})");
    println!(
        "render footprint: xz radius={}, y={}..{}",
        radius, min_y_chunk, max_y_chunk
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

fn default_output_path(source: &PreviewSource, center_x: i32, center_z: i32, radius: i32) -> PathBuf {
    match source {
        PreviewSource::Seed(seed) => PathBuf::from(format!(
            "target/chunk-preview/seed_{seed}_cx{center_x}_cz{center_z}_r{radius}.png"
        )),
        PreviewSource::BakedWorld(world_dir) => world_dir.join(format!(
            "preview_cx{center_x}_cz{center_z}_r{radius}.png"
        )),
    }
}

fn ensure_baked_bounds_cover_request(
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
            "requested preview area x={}..{}, y={}..{}, z={}..{} is outside baked bounds x={}..{}, y={}..{}, z={}..{}",
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

fn load_baked_preview_chunks(
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

fn choose_baked_center(
    manifest: &BakedWorldManifest,
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
        "no baked preview center fits radius {} inside baked bounds x={}..{}, y={}..{}, z={}..{}; try a smaller radius or pass --center-x/--center-z",
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

fn preview_environment() -> RenderEnvironment {
    let mut environment = RenderEnvironment::sunset_quarter_view();
    environment.time_of_day_hours = 14.0;
    environment.sun_direction = normalize3([0.34, 0.91, -0.22]);
    environment.sun_color = [1.0, 0.97, 0.92];
    environment.sun_intensity = 1.15;
    environment.ambient_color = [0.60, 0.66, 0.74];
    environment.ambient_intensity = 0.96;
    environment.fog_color = [0.82, 0.90, 0.98];
    environment.fog_density = 0.003;
    environment.fog_height_falloff = 0.025;
    environment.sky_color = [0.58, 0.76, 0.96];
    environment.horizon_color = [0.86, 0.92, 0.99];
    environment.overcast = 0.04;
    environment.climate_tint = [1.0, 1.0, 1.0];
    environment.top_face_boost = 0.18;
    environment.side_shadow_strength = 0.38;
    environment.silhouette_boost = 0.16;
    environment.saturation_boost = 0.14;
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
    "usage: cargo run --bin chunk_preview -- <seed> [--center-x <i32>] [--center-z <i32>] [--radius <i32>] [--quarter-turns <u8>] [--width <u32>] [--height <u32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--output <path>]\n   or: cargo run --bin chunk_preview -- --world-dir <path> [--center-x <i32>] [--center-z <i32>] [--radius <i32>] [--quarter-turns <u8>] [--width <u32>] [--height <u32>] [--min-y-chunk <i32>] [--max-y-chunk <i32>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}
