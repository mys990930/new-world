use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
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
    BlockMaterialKind, BlockRegistry, CHUNK_EDGE_I32, ChunkCoord, SEA_LEVEL_Y, TextureTileSource,
    WORLD_FLOOR_Y, WorldCore, WorldMeta, build_chunk_mesh, generate_chunk,
};

const DEFAULT_RENDER_RADIUS: i32 = 4;
const DEFAULT_RENDER_PADDING: i32 = 2;
const DEFAULT_IMAGE_WIDTH: u32 = 1600;
const DEFAULT_IMAGE_HEIGHT: u32 = 900;
const DEFAULT_RENDER_MIN_Y_CHUNK: i32 = -2;
const DEFAULT_RENDER_MAX_Y_CHUNK: i32 = 3;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let seed = parse_required::<u64>(&mut args, "seed")?;
    let mut center_x = 0_i32;
    let mut center_z = 0_i32;
    let mut radius = DEFAULT_RENDER_RADIUS;
    let mut quarter_turns = 0_u8;
    let mut width = DEFAULT_IMAGE_WIDTH;
    let mut height = DEFAULT_IMAGE_HEIGHT;
    let mut output = PathBuf::from(format!(
        "target/chunk-preview/seed_{seed}_cx0_cz0_r{DEFAULT_RENDER_RADIUS}.png"
    ));

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--center-x" => center_x = parse_required::<i32>(&mut args, "center-x")?,
            "--center-z" => center_z = parse_required::<i32>(&mut args, "center-z")?,
            "--radius" => radius = parse_required::<i32>(&mut args, "radius")?,
            "--quarter-turns" => quarter_turns = parse_required::<u8>(&mut args, "quarter-turns")?,
            "--width" => width = parse_required::<u32>(&mut args, "width")?,
            "--height" => height = parse_required::<u32>(&mut args, "height")?,
            "--output" => output = PathBuf::from(parse_required::<String>(&mut args, "output")?),
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    if radius < 0 {
        return Err(cli_error("radius must be non-negative"));
    }

    if output == PathBuf::from(format!(
        "target/chunk-preview/seed_{seed}_cx0_cz0_r{DEFAULT_RENDER_RADIUS}.png"
    )) {
        output = PathBuf::from(format!(
            "target/chunk-preview/seed_{seed}_cx{center_x}_cz{center_z}_r{radius}.png"
        ));
    }

    let block_registry = Arc::new(
        BlockRegistry::load_default()
            .map_err(|error| cli_error(format!("failed to load block registry: {error:?}")))?,
    );
    let meta = WorldMeta::new(seed);
    let mut world = WorldCore::new(meta, Arc::clone(&block_registry));

    let generation_radius = radius + DEFAULT_RENDER_PADDING;
    let generation_min_y = WORLD_FLOOR_Y.div_euclid(CHUNK_EDGE_I32);
    let generation_max_y = DEFAULT_RENDER_MAX_Y_CHUNK;

    for y in generation_min_y..=generation_max_y {
        for z in (center_z - generation_radius)..=(center_z + generation_radius) {
            for x in (center_x - generation_radius)..=(center_x + generation_radius) {
                let coord = ChunkCoord(x, y, z);
                let chunk = generate_chunk(coord, world.meta(), block_registry.as_ref());
                world.insert_chunk(coord, chunk);
            }
        }
    }

    let mut render_meshes = Vec::new();
    for y in DEFAULT_RENDER_MIN_Y_CHUNK..=DEFAULT_RENDER_MAX_Y_CHUNK {
        for z in (center_z - radius)..=(center_z + radius) {
            for x in (center_x - radius)..=(center_x + radius) {
                let coord = ChunkCoord(x, y, z);
                let Some(snapshot) = world.snapshot_chunk(coord) else {
                    continue;
                };
                let mesh = build_chunk_mesh(&snapshot, world.query_neighbors(coord), block_registry.as_ref());
                if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                    continue;
                }
                render_meshes.push(world_mesh_to_render(mesh));
            }
        }
    }

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

    println!("preview seed: {seed}");
    println!("center chunk: ({center_x}, {center_z})");
    println!(
        "render footprint: xz radius={}, y={}..{}",
        radius, DEFAULT_RENDER_MIN_Y_CHUNK, DEFAULT_RENDER_MAX_Y_CHUNK
    );
    println!(
        "generated support footprint: xz radius={}, y={}..{}",
        generation_radius, generation_min_y, generation_max_y
    );
    println!("sea level: y={SEA_LEVEL_Y}");
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

fn combined_render_bounds(
    meshes: &[RenderCpuMesh],
) -> Option<new_world::renderer::RenderBounds> {
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
    "usage: cargo run --bin chunk_preview -- <seed> [--center-x <i32>] [--center-z <i32>] [--radius <i32>] [--quarter-turns <u8>] [--width <u32>] [--height <u32>] [--output <path>]"
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}
