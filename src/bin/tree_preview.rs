use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

use new_world::ecs::{QUARTER_VIEW_VERTICAL_WORLD_SIZE, quarter_view_basis, quarter_view_eye};
use new_world::renderer::{
    CpuMesh as RenderCpuMesh, MeshVertex as RenderMeshVertex, OffscreenRenderRequest,
    RenderCameraState, RenderEnvironment, RenderMaterialKind, RenderProjectionMode,
    RenderTextureArraySource, RenderTextureSource, RenderTextureTile, RenderViewBasis,
    render_offscreen, write_offscreen_png,
};
use new_world::world::{
    BlockMaterialKind, BlockRegistry, ChunkCoord, ChunkData, NeighborChunks, TextureTileSource,
    TreeBlockPalette, TreeBlueprint, TreeGenRequest, TreeKind, WorldBlockCoord, build_chunk_mesh,
    generate_tree_blueprint, world_to_chunk_local,
};

const DEFAULT_IMAGE_WIDTH: u32 = 1200;
const DEFAULT_IMAGE_HEIGHT: u32 = 900;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Err(cli_error(usage()));
    }

    let kind_key = parse_required::<String>(&mut args, "tree-kind")?;
    let kind = TreeKind::from_key(&kind_key)
        .ok_or_else(|| cli_error(format!("unknown tree kind: {kind_key}\n\n{}", usage())))?;
    let seed = parse_required::<u64>(&mut args, "seed")?;
    let mut output: Option<PathBuf> = None;
    let mut width = DEFAULT_IMAGE_WIDTH;
    let mut height = DEFAULT_IMAGE_HEIGHT;
    let mut quarter_turns = 0_u8;

    while let Some(flag) = args.first().cloned() {
        args.remove(0);
        match flag.as_str() {
            "--output" => {
                output = Some(PathBuf::from(parse_required::<String>(
                    &mut args, "output",
                )?))
            }
            "--width" => width = parse_required::<u32>(&mut args, "width")?,
            "--height" => height = parse_required::<u32>(&mut args, "height")?,
            "--quarter-turns" => quarter_turns = parse_required::<u8>(&mut args, "quarter-turns")?,
            _ => return Err(cli_error(format!("unknown flag: {flag}\n\n{}", usage()))),
        }
    }

    if width == 0 || height == 0 {
        return Err(cli_error("width and height must be greater than zero"));
    }

    let registry = BlockRegistry::load_default()
        .map_err(|error| cli_error(format!("failed to load block registry: {error:?}")))?;
    let palette = TreeBlockPalette::resolve_default(kind, &registry)?;
    let blueprint = generate_tree_blueprint(TreeGenRequest {
        kind,
        origin: WorldBlockCoord(16, 1, 16),
        seed,
        palette,
    });
    let chunks = build_preview_chunks(&blueprint, &registry);
    let render_meshes = collect_render_meshes(&chunks, &registry);
    let bounds = combined_render_bounds(&render_meshes)
        .ok_or_else(|| cli_error("generated tree produced no renderable mesh"))?;
    let camera = build_preview_camera(bounds, width, height, quarter_turns % 4);
    let output = output.unwrap_or_else(|| {
        PathBuf::from("target").join("tree-preview").join(format!(
            "{}_seed_{}.png",
            kind.key(),
            seed
        ))
    });

    let image = render_offscreen(OffscreenRenderRequest {
        width,
        height,
        camera,
        textures: block_registry_to_render_textures(&registry),
        environment: preview_environment(),
        chunk_meshes: render_meshes,
        clear_color_override: Some([0.72, 0.82, 0.92, 1.0]),
    })?;
    write_offscreen_png(&output, &image)?;

    println!("tree kind: {} ({})", kind.key(), kind.display_name());
    println!("seed: {seed}");
    println!("voxel count: {}", blueprint.voxels.len());
    println!(
        "bounds: min={:?} max={:?}",
        blueprint.bounds.min, blueprint.bounds.max
    );
    println!("output: {}", output.display());
    Ok(())
}

fn build_preview_chunks(
    blueprint: &TreeBlueprint,
    registry: &BlockRegistry,
) -> HashMap<ChunkCoord, ChunkData> {
    let mut chunks = HashMap::new();
    let ground = registry
        .block_id("grass")
        .unwrap_or(new_world::world::BlockId::GRASS);

    for x in -8..=24 {
        for z in -8..=24 {
            set_world_block(&mut chunks, WorldBlockCoord(x, 0, z), ground);
        }
    }

    for voxel in &blueprint.voxels {
        let world = WorldBlockCoord(
            blueprint.origin.0 + i32::from(voxel.offset[0]),
            blueprint.origin.1 + i32::from(voxel.offset[1]),
            blueprint.origin.2 + i32::from(voxel.offset[2]),
        );
        set_world_block(&mut chunks, world, voxel.block);
    }

    chunks
}

fn set_world_block(
    chunks: &mut HashMap<ChunkCoord, ChunkData>,
    world: WorldBlockCoord,
    block: new_world::world::BlockId,
) {
    let (coord, local) = world_to_chunk_local(world);
    let chunk = chunks
        .entry(coord)
        .or_insert_with(|| ChunkData::new_empty(coord));
    chunk
        .set_block(local, block)
        .expect("world_to_chunk_local must produce an in-bounds local coord");
}

fn collect_render_meshes(
    chunks: &HashMap<ChunkCoord, ChunkData>,
    registry: &BlockRegistry,
) -> Vec<RenderCpuMesh> {
    let snapshots = chunks
        .iter()
        .map(|(coord, chunk)| (*coord, chunk.snapshot()))
        .collect::<HashMap<_, _>>();

    snapshots
        .iter()
        .filter_map(|(coord, snapshot)| {
            let neighbors = NeighborChunks {
                neg_x: snapshots.get(&coord.offset(-1, 0, 0)).cloned(),
                pos_x: snapshots.get(&coord.offset(1, 0, 0)).cloned(),
                neg_y: snapshots.get(&coord.offset(0, -1, 0)).cloned(),
                pos_y: snapshots.get(&coord.offset(0, 1, 0)).cloned(),
                neg_z: snapshots.get(&coord.offset(0, 0, -1)).cloned(),
                pos_z: snapshots.get(&coord.offset(0, 0, 1)).cloned(),
            };
            let mesh = build_chunk_mesh(snapshot, neighbors, registry);
            (!mesh.is_empty()).then(|| world_mesh_to_render(mesh))
        })
        .collect()
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

fn world_vertex_to_render(vertex: new_world::world::MeshVertex) -> RenderMeshVertex {
    RenderMeshVertex {
        position: vertex.position,
        color: vertex.color,
        normal: vertex.normal,
        uv: vertex.uv,
        texture_layer: vertex.texture_layer,
        material_kind: render_material_kind_from_world(vertex.material_kind).as_u32(),
        contour_edges: vertex.contour_edges,
    }
}

fn render_material_kind_from_world(kind: BlockMaterialKind) -> RenderMaterialKind {
    match kind {
        BlockMaterialKind::GenericOpaque => RenderMaterialKind::GenericOpaque,
        BlockMaterialKind::Grass => RenderMaterialKind::Grass,
        BlockMaterialKind::Soil => RenderMaterialKind::Soil,
        BlockMaterialKind::Stone => RenderMaterialKind::Stone,
        BlockMaterialKind::Sand => RenderMaterialKind::Sand,
        BlockMaterialKind::Foliage => RenderMaterialKind::Foliage,
        BlockMaterialKind::Water => RenderMaterialKind::Water,
        BlockMaterialKind::Emissive => RenderMaterialKind::Emissive,
    }
}

fn combined_render_bounds(meshes: &[RenderCpuMesh]) -> Option<new_world::renderer::RenderBounds> {
    let mut combined: Option<new_world::renderer::RenderBounds> = None;

    for mesh in meshes {
        let Some(bounds) = mesh.bounds else {
            continue;
        };
        match &mut combined {
            Some(current) => {
                current.min[0] = current.min[0].min(bounds.min[0]);
                current.min[1] = current.min[1].min(bounds.min[1]);
                current.min[2] = current.min[2].min(bounds.min[2]);
                current.max[0] = current.max[0].max(bounds.max[0]);
                current.max[1] = current.max[1].max(bounds.max[1]);
                current.max[2] = current.max[2].max(bounds.max[2]);
            }
            None => combined = Some(bounds),
        }
    }

    combined
}

fn build_preview_camera(
    bounds: new_world::renderer::RenderBounds,
    width: u32,
    height: u32,
    quarter_turns: u8,
) -> RenderCameraState {
    let aspect = width as f32 / height as f32;
    let basis = quarter_view_basis(quarter_turns);
    let target = [
        (bounds.min[0] + bounds.max[0]) * 0.5,
        bounds.min[1] + (bounds.max[1] - bounds.min[1]) * 0.45,
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
        (up_extent.max(right_extent / aspect) * 1.22).max(QUARTER_VIEW_VERTICAL_WORLD_SIZE * 0.35);

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

fn preview_environment() -> RenderEnvironment {
    let mut environment = RenderEnvironment::midday_quarter_view();
    environment.fog_density = 0.0;
    environment.ambient_intensity = 1.05;
    environment.sun_intensity = 1.05;
    environment.top_face_boost = 0.20;
    environment.side_shadow_strength = 0.22;
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

fn parse_required<T>(args: &mut Vec<String>, name: &'static str) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| cli_error(format!("missing required argument: {name}")))?;
    args.remove(0);
    value
        .parse::<T>()
        .map_err(|error| cli_error(format!("invalid {name}: {error}")))
}

fn cli_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

fn usage() -> String {
    let kinds = TreeKind::all()
        .iter()
        .map(|kind| kind.key())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "usage: cargo run --bin tree_preview -- <tree-kind> <seed> [--output <path>] [--width <u32>] [--height <u32>] [--quarter-turns <u8>]\n\navailable tree kinds: {kinds}"
    )
}
