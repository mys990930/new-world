use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

use new_world::ecs::QUARTER_VIEW_VERTICAL_WORLD_SIZE;
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

mod common;

use common::preview_compass::draw_compass_offscreen;

const DEFAULT_IMAGE_WIDTH: u32 = 1600;
const DEFAULT_IMAGE_HEIGHT: u32 = 1000;
const PREVIEW_TREE_COUNT: usize = 5;
const PREVIEW_TREE_SPACING_BLOCKS: i32 = 36;
const PREVIEW_CAMERA_DISTANCE: f32 = 520.0;
const PREVIEW_CAMERA_DOWNWARD_COMPONENT: f32 = 0.42;

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
    let blueprints = generate_preview_blueprints(kind, seed, palette);
    let chunks = build_preview_chunks(&blueprints, &registry);
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

    let mut image = render_offscreen(OffscreenRenderRequest {
        width,
        height,
        camera,
        textures: block_registry_to_render_textures(&registry),
        environment: preview_environment(),
        chunk_meshes: render_meshes,
        clear_color_override: Some([0.34, 0.39, 0.43, 1.0]),
    })?;
    draw_compass_offscreen(&mut image);
    write_offscreen_png(&output, &image)?;

    println!("tree kind: {} ({})", kind.key(), kind.display_name());
    println!("preview seed: {seed}");
    for (index, blueprint) in blueprints.iter().enumerate() {
        println!(
            "tree {}: origin=({},{},{}) seed={} voxels={} bounds={:?}..{:?}",
            index + 1,
            blueprint.origin.0,
            blueprint.origin.1,
            blueprint.origin.2,
            preview_tree_seed(seed, kind, index),
            blueprint.voxels.len(),
            blueprint.bounds.min,
            blueprint.bounds.max
        );
    }
    println!("output: {}", output.display());
    Ok(())
}

fn generate_preview_blueprints(
    kind: TreeKind,
    preview_seed: u64,
    palette: TreeBlockPalette,
) -> Vec<TreeBlueprint> {
    let start_x = -((PREVIEW_TREE_COUNT as i32 - 1) * PREVIEW_TREE_SPACING_BLOCKS) / 2;
    (0..PREVIEW_TREE_COUNT)
        .map(|index| {
            let origin =
                WorldBlockCoord(start_x + index as i32 * PREVIEW_TREE_SPACING_BLOCKS, 1, 0);
            generate_tree_blueprint(TreeGenRequest {
                kind,
                origin,
                seed: preview_tree_seed(preview_seed, kind, index),
                palette,
            })
        })
        .collect()
}

fn build_preview_chunks(
    blueprints: &[TreeBlueprint],
    registry: &BlockRegistry,
) -> HashMap<ChunkCoord, ChunkData> {
    let mut chunks = HashMap::new();
    let ground = registry
        .block_id("grass")
        .unwrap_or(new_world::world::BlockId::GRASS);
    let (min_x, max_x, min_z, max_z) = preview_ground_bounds(blueprints);

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            set_world_block(&mut chunks, WorldBlockCoord(x, 0, z), ground);
        }
    }

    for blueprint in blueprints {
        for voxel in &blueprint.voxels {
            let world = WorldBlockCoord(
                blueprint.origin.0 + i32::from(voxel.offset[0]),
                blueprint.origin.1 + i32::from(voxel.offset[1]),
                blueprint.origin.2 + i32::from(voxel.offset[2]),
            );
            set_world_block(&mut chunks, world, voxel.block);
        }
    }

    chunks
}

fn preview_ground_bounds(blueprints: &[TreeBlueprint]) -> (i32, i32, i32, i32) {
    let mut min_x = 0;
    let mut max_x = 0;
    let mut min_z = 0;
    let mut max_z = 0;

    for (index, blueprint) in blueprints.iter().enumerate() {
        let tree_min_x = blueprint.origin.0 + i32::from(blueprint.bounds.min[0]);
        let tree_max_x = blueprint.origin.0 + i32::from(blueprint.bounds.max[0]);
        let tree_min_z = blueprint.origin.2 + i32::from(blueprint.bounds.min[2]);
        let tree_max_z = blueprint.origin.2 + i32::from(blueprint.bounds.max[2]);
        if index == 0 {
            min_x = tree_min_x;
            max_x = tree_max_x;
            min_z = tree_min_z;
            max_z = tree_max_z;
        } else {
            min_x = min_x.min(tree_min_x);
            max_x = max_x.max(tree_max_x);
            min_z = min_z.min(tree_min_z);
            max_z = max_z.max(tree_max_z);
        }
    }

    (min_x - 8, max_x + 8, min_z - 8, max_z + 8)
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
    let basis = low_angle_preview_basis(quarter_turns);
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
        (up_extent.max(right_extent / aspect) * 1.08).max(QUARTER_VIEW_VERTICAL_WORLD_SIZE * 0.32);

    RenderCameraState {
        eye: add3(target, scale3(basis.forward, -PREVIEW_CAMERA_DISTANCE)),
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
    let mut environment = RenderEnvironment::sunset_quarter_view();
    environment.sun_color = [0.96, 0.88, 0.72];
    environment.sun_intensity = 0.76;
    environment.ambient_color = [0.38, 0.43, 0.48];
    environment.ambient_intensity = 0.48;
    environment.fog_color = [0.46, 0.52, 0.57];
    environment.fog_density = 0.003;
    environment.top_face_boost = 0.08;
    environment.side_shadow_strength = 0.58;
    environment.silhouette_boost = 0.18;
    environment.saturation_boost = 0.0;
    environment
}

fn low_angle_preview_basis(quarter_turns: u8) -> RenderViewBasis {
    let horizontal = (1.0 - PREVIEW_CAMERA_DOWNWARD_COMPONENT * PREVIEW_CAMERA_DOWNWARD_COMPONENT)
        .max(0.0)
        .sqrt();
    let inv_sqrt_2 = std::f32::consts::FRAC_1_SQRT_2;
    let right = rotate_y_quarter_turns([inv_sqrt_2, 0.0, inv_sqrt_2], quarter_turns);
    let forward = normalize3(rotate_y_quarter_turns(
        [
            -horizontal * inv_sqrt_2,
            -PREVIEW_CAMERA_DOWNWARD_COMPONENT,
            horizontal * inv_sqrt_2,
        ],
        quarter_turns,
    ));
    let up = normalize3(cross3(forward, right));

    RenderViewBasis { right, up, forward }
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

fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn scale3(vector: [f32; 3], scalar: f32) -> [f32; 3] {
    [vector[0] * scalar, vector[1] * scalar, vector[2] * scalar]
}

fn cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length_sq = dot3(vector, vector);
    if length_sq <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        scale3(vector, length_sq.sqrt().recip())
    }
}

fn rotate_y_quarter_turns(vector: [f32; 3], quarter_turns: u8) -> [f32; 3] {
    match quarter_turns % 4 {
        0 => vector,
        1 => [vector[2], vector[1], -vector[0]],
        2 => [-vector[0], vector[1], -vector[2]],
        3 => [-vector[2], vector[1], vector[0]],
        _ => unreachable!(),
    }
}

fn preview_tree_seed(preview_seed: u64, kind: TreeKind, index: usize) -> u64 {
    let value = preview_seed
        ^ (kind.key().bytes().fold(0_u64, |hash, byte| {
            hash.wrapping_mul(131).wrapping_add(u64::from(byte))
        }))
        ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    splitmix64(value)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
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
