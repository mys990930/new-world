use super::chunk::{BlockFace, BlockId, ChunkSnapshot};
use super::coord::{
    CHUNK_EDGE_I32, ChunkCoord, LocalBlockCoord, WorldBlockCoord, chunk_local_to_world,
};
use super::query::NeighborChunks;
use super::registry::{BlockDef, BlockMaterialKind, BlockRegistry};

const TOP_EDGE_NEG_X: u32 = 1 << 0;
const TOP_EDGE_POS_X: u32 = 1 << 1;
const TOP_EDGE_NEG_Z: u32 = 1 << 2;
const TOP_EDGE_POS_Z: u32 = 1 << 3;
const HEIGHT_EPSILON: f32 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub texture_layer: u32,
    pub material_kind: BlockMaterialKind,
    pub contour_edges: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuMesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub bounds: Option<RenderBounds>,
}

impl CpuMesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.indices.is_empty()
    }
}

pub fn build_chunk_mesh(
    center: &ChunkSnapshot,
    neighbors: NeighborChunks,
    registry: &BlockRegistry,
) -> CpuMesh {
    if let Some(mesh) = uniform_chunk_mesh(center, &neighbors, registry) {
        return mesh;
    }

    let mut mesh = CpuMesh::default();

    for y in 0..super::coord::CHUNK_EDGE as u8 {
        for z in 0..super::coord::CHUNK_EDGE as u8 {
            for x in 0..super::coord::CHUNK_EDGE as u8 {
                let local =
                    LocalBlockCoord::new(x, y, z).expect("chunk iteration must stay in bounds");
                let Some(block) = center.get_block(local) else {
                    continue;
                };
                let block_def = registry.block_or_missing(block);
                if block_def.is_rendered_foliage_cross() {
                    append_foliage_cross(&mut mesh, center.coord(), local, block_def);
                    continue;
                }
                if !block_def.is_rendered_cube() {
                    continue;
                }
                let block_height =
                    resolved_block_surface_height(center, &neighbors, local, registry);
                if block_height <= HEIGHT_EPSILON {
                    continue;
                }

                for face in faces() {
                    if matches!(face, BlockFace::PosY) {
                        continue;
                    }

                    let Some(face_span) = visible_face_span(
                        center,
                        &neighbors,
                        local,
                        block_def,
                        block_height,
                        face,
                        registry,
                    ) else {
                        continue;
                    };

                    append_face(
                        &mut mesh,
                        center.coord(),
                        local,
                        block_def,
                        face,
                        face_span,
                        top_face_contour_edges(
                            center,
                            &neighbors,
                            local,
                            block_height,
                            face,
                            registry,
                        ),
                    );
                }
            }
        }
    }

    append_greedy_top_faces(&mut mesh, center, &neighbors, registry);

    mesh
}

fn uniform_chunk_mesh(
    center: &ChunkSnapshot,
    neighbors: &NeighborChunks,
    registry: &BlockRegistry,
) -> Option<CpuMesh> {
    let block = center.uniform_block()?;
    let block_def = registry.block_or_missing(block);
    if !block_def.is_rendered_cube() {
        return if block_def.is_rendered_foliage_cross() {
            None
        } else {
            Some(CpuMesh::default())
        };
    }
    if !block_def.is_opaque() || block_def.surface_height() < 1.0 - HEIGHT_EPSILON {
        return None;
    }

    Some(build_uniform_opaque_boundary_mesh(
        center, neighbors, block_def, registry,
    ))
}

fn build_uniform_opaque_boundary_mesh(
    center: &ChunkSnapshot,
    neighbors: &NeighborChunks,
    block_def: &BlockDef,
    registry: &BlockRegistry,
) -> CpuMesh {
    let mut mesh = CpuMesh::default();
    let block_height = 1.0;

    for face in faces() {
        for a in 0..super::coord::CHUNK_EDGE as u8 {
            for b in 0..super::coord::CHUNK_EDGE as u8 {
                let local = boundary_local_for_face(face, a, b);
                let Some(face_span) = visible_face_span(
                    center,
                    neighbors,
                    local,
                    block_def,
                    block_height,
                    face,
                    registry,
                ) else {
                    continue;
                };

                append_face(
                    &mut mesh,
                    center.coord(),
                    local,
                    block_def,
                    face,
                    face_span,
                    top_face_contour_edges(center, neighbors, local, block_height, face, registry),
                );
            }
        }
    }

    mesh
}

fn boundary_local_for_face(face: BlockFace, a: u8, b: u8) -> LocalBlockCoord {
    let edge = super::coord::CHUNK_EDGE as u8 - 1;
    match face {
        BlockFace::NegX => LocalBlockCoord::new(0, a, b).unwrap(),
        BlockFace::PosX => LocalBlockCoord::new(edge, a, b).unwrap(),
        BlockFace::NegY => LocalBlockCoord::new(a, 0, b).unwrap(),
        BlockFace::PosY => LocalBlockCoord::new(a, edge, b).unwrap(),
        BlockFace::NegZ => LocalBlockCoord::new(a, b, 0).unwrap(),
        BlockFace::PosZ => LocalBlockCoord::new(a, b, edge).unwrap(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TopFaceCell {
    block: BlockId,
    face_span: FaceSpan,
    color: [f32; 4],
    texture_layer: u32,
    material_kind: BlockMaterialKind,
    contour_edges: u32,
}

impl TopFaceCell {
    fn is_mergeable(self) -> bool {
        self.contour_edges == 0
    }

    fn same_merge_key(self, other: Self) -> bool {
        self.is_mergeable()
            && other.is_mergeable()
            && self.block == other.block
            && self.face_span == other.face_span
            && self.color == other.color
            && self.texture_layer == other.texture_layer
            && self.material_kind == other.material_kind
    }
}

fn append_greedy_top_faces(
    mesh: &mut CpuMesh,
    center: &ChunkSnapshot,
    neighbors: &NeighborChunks,
    registry: &BlockRegistry,
) {
    let edge = super::coord::CHUNK_EDGE;
    let mut cells = vec![None; edge * edge];
    let mut visited = vec![false; edge * edge];

    for y in 0..edge as u8 {
        cells.fill(None);
        visited.fill(false);

        for z in 0..edge as u8 {
            for x in 0..edge as u8 {
                let local =
                    LocalBlockCoord::new(x, y, z).expect("chunk iteration must stay in bounds");
                cells[top_face_index(x, z)] = top_face_cell(center, neighbors, local, registry);
            }
        }

        for z in 0..edge as u8 {
            for x in 0..edge as u8 {
                let index = top_face_index(x, z);
                if visited[index] {
                    continue;
                }
                let Some(cell) = cells[index] else {
                    continue;
                };

                let local =
                    LocalBlockCoord::new(x, y, z).expect("chunk iteration must stay in bounds");
                if !cell.is_mergeable() {
                    visited[index] = true;
                    let block_def = registry.block_or_missing(cell.block);
                    append_face(
                        mesh,
                        center.coord(),
                        local,
                        block_def,
                        BlockFace::PosY,
                        cell.face_span,
                        cell.contour_edges,
                    );
                    continue;
                }

                let width = top_face_merge_width(&cells, &visited, x, z, cell);
                let depth = top_face_merge_depth(&cells, &visited, x, z, width, cell);

                for dz in 0..depth {
                    for dx in 0..width {
                        visited[top_face_index(x + dx, z + dz)] = true;
                    }
                }

                append_top_face_rect(
                    mesh,
                    center.coord(),
                    local,
                    u32::from(width),
                    u32::from(depth),
                    cell,
                );
            }
        }
    }
}

fn top_face_cell(
    center: &ChunkSnapshot,
    neighbors: &NeighborChunks,
    local: LocalBlockCoord,
    registry: &BlockRegistry,
) -> Option<TopFaceCell> {
    let block = center.get_block(local)?;
    let block_def = registry.block_or_missing(block);
    if !block_def.is_rendered_cube() {
        return None;
    }

    let block_height = resolved_block_surface_height(center, neighbors, local, registry);
    if block_height <= HEIGHT_EPSILON {
        return None;
    }

    let face_span = visible_face_span(
        center,
        neighbors,
        local,
        block_def,
        block_height,
        BlockFace::PosY,
        registry,
    )?;
    let contour_edges = top_face_contour_edges(
        center,
        neighbors,
        local,
        block_height,
        BlockFace::PosY,
        registry,
    );

    Some(TopFaceCell {
        block,
        face_span,
        color: block_def.tint_as_linear_rgba(),
        texture_layer: u32::from(block_def.texture_for_face(BlockFace::PosY).0),
        material_kind: block_def.material,
        contour_edges,
    })
}

fn top_face_merge_width(
    cells: &[Option<TopFaceCell>],
    visited: &[bool],
    x: u8,
    z: u8,
    cell: TopFaceCell,
) -> u8 {
    let edge = super::coord::CHUNK_EDGE as u8;
    let mut width = 0;
    while x + width < edge {
        let index = top_face_index(x + width, z);
        if visited[index] || !same_top_face_merge_cell(cells[index], cell) {
            break;
        }
        width += 1;
    }
    width
}

fn top_face_merge_depth(
    cells: &[Option<TopFaceCell>],
    visited: &[bool],
    x: u8,
    z: u8,
    width: u8,
    cell: TopFaceCell,
) -> u8 {
    let edge = super::coord::CHUNK_EDGE as u8;
    let mut depth = 0;
    'rows: while z + depth < edge {
        for dx in 0..width {
            let index = top_face_index(x + dx, z + depth);
            if visited[index] || !same_top_face_merge_cell(cells[index], cell) {
                break 'rows;
            }
        }
        depth += 1;
    }
    depth
}

fn same_top_face_merge_cell(candidate: Option<TopFaceCell>, cell: TopFaceCell) -> bool {
    candidate.is_some_and(|candidate| candidate.same_merge_key(cell))
}

fn top_face_index(x: u8, z: u8) -> usize {
    usize::from(z) * super::coord::CHUNK_EDGE + usize::from(x)
}

fn neighbor_block(
    center: &ChunkSnapshot,
    neighbors: &NeighborChunks,
    local: LocalBlockCoord,
    face: BlockFace,
) -> Option<BlockId> {
    match face {
        BlockFace::NegX if local.x > 0 => {
            center.get_block(LocalBlockCoord::new(local.x - 1, local.y, local.z).unwrap())
        }
        BlockFace::PosX if usize::from(local.x) + 1 < super::coord::CHUNK_EDGE => {
            center.get_block(LocalBlockCoord::new(local.x + 1, local.y, local.z).unwrap())
        }
        BlockFace::NegY if local.y > 0 => {
            center.get_block(LocalBlockCoord::new(local.x, local.y - 1, local.z).unwrap())
        }
        BlockFace::PosY if usize::from(local.y) + 1 < super::coord::CHUNK_EDGE => {
            center.get_block(LocalBlockCoord::new(local.x, local.y + 1, local.z).unwrap())
        }
        BlockFace::NegZ if local.z > 0 => {
            center.get_block(LocalBlockCoord::new(local.x, local.y, local.z - 1).unwrap())
        }
        BlockFace::PosZ if usize::from(local.z) + 1 < super::coord::CHUNK_EDGE => {
            center.get_block(LocalBlockCoord::new(local.x, local.y, local.z + 1).unwrap())
        }
        _ => neighbors.block_across_face(face, local),
    }
}

fn append_top_face_rect(
    mesh: &mut CpuMesh,
    chunk: ChunkCoord,
    local: LocalBlockCoord,
    width: u32,
    depth: u32,
    cell: TopFaceCell,
) {
    let world = chunk_local_to_world(chunk, local);
    let min = [world.0 as f32, world.1 as f32, world.2 as f32];
    let top_y = min[1] + cell.face_span.max_y;
    let max_x = min[0] + width as f32;
    let max_z = min[2] + depth as f32;
    let positions = [
        [min[0], top_y, min[2]],
        [max_x, top_y, min[2]],
        [max_x, top_y, max_z],
        [min[0], top_y, max_z],
    ];
    let uv = [
        [0.0, depth as f32],
        [width as f32, depth as f32],
        [width as f32, 0.0],
        [0.0, 0.0],
    ];
    let base_index = mesh.vertices.len() as u32;

    extend_bounds(&mut mesh.bounds, &positions);

    for (position, uv) in positions.into_iter().zip(uv) {
        mesh.vertices.push(MeshVertex {
            position,
            color: cell.color,
            normal: [0.0, 1.0, 0.0],
            uv,
            texture_layer: cell.texture_layer,
            material_kind: cell.material_kind,
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

fn append_face(
    mesh: &mut CpuMesh,
    chunk: ChunkCoord,
    local: LocalBlockCoord,
    block_def: &BlockDef,
    face: BlockFace,
    face_span: FaceSpan,
    contour_edges: u32,
) {
    let world = chunk_local_to_world(chunk, local);
    let positions = face_positions(world, face, face_span);
    let base_index = mesh.vertices.len() as u32;
    let color = block_def.tint_as_linear_rgba();
    let normal = face_normal(face);
    let uv = face_uvs(face, face_span);
    let texture_layer = u32::from(block_def.texture_for_face(face).0);
    let material_kind = block_def.material;

    extend_bounds(&mut mesh.bounds, &positions);

    for (position, uv) in positions.into_iter().zip(uv) {
        mesh.vertices.push(MeshVertex {
            position,
            color,
            normal,
            uv,
            texture_layer,
            material_kind,
            contour_edges,
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

fn append_foliage_cross(
    mesh: &mut CpuMesh,
    chunk: ChunkCoord,
    local: LocalBlockCoord,
    block_def: &BlockDef,
) {
    let world = chunk_local_to_world(chunk, local);
    let min = [world.0 as f32, world.1 as f32, world.2 as f32];
    let max = [
        min[0] + 1.0,
        min[1] + block_def.surface_height(),
        min[2] + 1.0,
    ];
    let color = block_def.tint_as_linear_rgba();
    let texture_layer = u32::from(block_def.texture_for_face(BlockFace::PosZ).0);
    let material_kind = block_def.material;

    let quads = [
        (
            [
                [min[0], min[1], min[2]],
                [max[0], min[1], max[2]],
                [max[0], max[1], max[2]],
                [min[0], max[1], min[2]],
            ],
            normalize2([-1.0, 0.0, 1.0]),
        ),
        (
            [
                [max[0], min[1], min[2]],
                [min[0], min[1], max[2]],
                [min[0], max[1], max[2]],
                [max[0], max[1], min[2]],
            ],
            normalize2([1.0, 0.0, 1.0]),
        ),
    ];

    for (positions, normal) in quads {
        append_textured_quad(mesh, positions, color, normal, texture_layer, material_kind);
    }
}

fn append_textured_quad(
    mesh: &mut CpuMesh,
    positions: [[f32; 3]; 4],
    color: [f32; 4],
    normal: [f32; 3],
    texture_layer: u32,
    material_kind: BlockMaterialKind,
) {
    let base_index = mesh.vertices.len() as u32;
    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

    extend_bounds(&mut mesh.bounds, &positions);

    for (position, uv) in positions.into_iter().zip(uvs) {
        mesh.vertices.push(MeshVertex {
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
        base_index + 2,
        base_index + 1,
        base_index,
        base_index + 3,
        base_index + 2,
        base_index,
    ]);
}

fn normalize2(value: [f32; 3]) -> [f32; 3] {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if length <= f32::EPSILON {
        return [0.0, 1.0, 0.0];
    }
    [value[0] / length, value[1] / length, value[2] / length]
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FaceSpan {
    min_y: f32,
    max_y: f32,
}

fn face_uvs(face: BlockFace, face_span: FaceSpan) -> [[f32; 2]; 4] {
    match face {
        BlockFace::PosY | BlockFace::NegY => [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        BlockFace::NegX | BlockFace::PosX | BlockFace::NegZ | BlockFace::PosZ => {
            let bottom_v = 1.0 - face_span.min_y;
            let top_v = 1.0 - face_span.max_y;
            [[0.0, bottom_v], [1.0, bottom_v], [1.0, top_v], [0.0, top_v]]
        }
    }
}

fn face_positions(world: WorldBlockCoord, face: BlockFace, face_span: FaceSpan) -> [[f32; 3]; 4] {
    let min = [world.0 as f32, world.1 as f32, world.2 as f32];
    let max = [min[0] + 1.0, min[1] + face_span.max_y, min[2] + 1.0];
    let clipped_min_y = min[1] + face_span.min_y;

    match face {
        BlockFace::NegX => [
            [min[0], clipped_min_y, min[2]],
            [min[0], clipped_min_y, max[2]],
            [min[0], max[1], max[2]],
            [min[0], max[1], min[2]],
        ],
        BlockFace::PosX => [
            [max[0], clipped_min_y, max[2]],
            [max[0], clipped_min_y, min[2]],
            [max[0], max[1], min[2]],
            [max[0], max[1], max[2]],
        ],
        BlockFace::NegY => [
            [min[0], clipped_min_y, max[2]],
            [max[0], clipped_min_y, max[2]],
            [max[0], clipped_min_y, min[2]],
            [min[0], clipped_min_y, min[2]],
        ],
        BlockFace::PosY => [
            [min[0], max[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ],
        BlockFace::NegZ => [
            [max[0], clipped_min_y, min[2]],
            [min[0], clipped_min_y, min[2]],
            [min[0], max[1], min[2]],
            [max[0], max[1], min[2]],
        ],
        BlockFace::PosZ => [
            [min[0], clipped_min_y, max[2]],
            [max[0], clipped_min_y, max[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ],
    }
}

fn visible_face_span(
    center: &ChunkSnapshot,
    neighbors: &NeighborChunks,
    local: LocalBlockCoord,
    block_def: &BlockDef,
    block_height: f32,
    face: BlockFace,
    registry: &BlockRegistry,
) -> Option<FaceSpan> {
    let neighbor = neighbor_block(center, neighbors, local, face);
    let neighbor_def = neighbor.map(|block| registry.block_or_missing(block));

    match face {
        BlockFace::PosY | BlockFace::NegY => {
            if neighbor_def.is_some_and(|neighbor_def| {
                neighbor_def.is_opaque() || shared_water_volume(block_def, neighbor_def)
            }) {
                return None;
            }

            let y = if matches!(face, BlockFace::PosY) {
                block_height
            } else {
                0.0
            };
            Some(FaceSpan { min_y: y, max_y: y })
        }
        BlockFace::NegX | BlockFace::PosX | BlockFace::NegZ | BlockFace::PosZ => {
            if neighbor_def.is_some_and(BlockDef::is_opaque) {
                return None;
            }

            if let Some(neighbor_def) = neighbor_def {
                if shared_water_volume(block_def, neighbor_def) {
                    let neighbor_height = surface_height_at_offset(
                        center,
                        neighbors,
                        local,
                        face_offset(face),
                        registry,
                    )
                    .unwrap_or(1.0);
                    if neighbor_height >= block_height - HEIGHT_EPSILON {
                        return None;
                    }

                    return Some(FaceSpan {
                        min_y: neighbor_height.clamp(0.0, block_height),
                        max_y: block_height,
                    });
                }
            }

            Some(FaceSpan {
                min_y: 0.0,
                max_y: block_height,
            })
        }
    }
}

fn top_face_contour_edges(
    center: &ChunkSnapshot,
    neighbors: &NeighborChunks,
    local: LocalBlockCoord,
    block_height: f32,
    face: BlockFace,
    registry: &BlockRegistry,
) -> u32 {
    if !matches!(face, BlockFace::PosY) {
        return 0;
    }

    let mut mask = 0;
    for (edge_mask, offset) in [
        (TOP_EDGE_NEG_X, (-1, 0, 0)),
        (TOP_EDGE_POS_X, (1, 0, 0)),
        (TOP_EDGE_NEG_Z, (0, 0, -1)),
        (TOP_EDGE_POS_Z, (0, 0, 1)),
    ] {
        let neighbor_height =
            surface_height_at_offset(center, neighbors, local, offset, registry).unwrap_or(0.0);
        if (neighbor_height - block_height).abs() > HEIGHT_EPSILON {
            mask |= edge_mask;
        }
    }

    mask
}

fn surface_height_at_offset(
    center: &ChunkSnapshot,
    neighbors: &NeighborChunks,
    local: LocalBlockCoord,
    offset: (i32, i32, i32),
    registry: &BlockRegistry,
) -> Option<f32> {
    let block = block_at_offset(center, neighbors, local, offset)?;
    let block_def = registry.block_or_missing(block);
    if !block_def.is_rendered_cube() {
        return Some(0.0);
    }

    let mut height = block_def.surface_height();
    if matches!(block_def.material, BlockMaterialKind::Water) {
        let above = block_at_offset(center, neighbors, local, (offset.0, offset.1 + 1, offset.2));
        if above.is_some_and(|block| {
            let above_def = registry.block_or_missing(block);
            above_def.is_rendered_cube() && matches!(above_def.material, BlockMaterialKind::Water)
        }) {
            height = 1.0;
        }
    }

    Some(height)
}

fn resolved_block_surface_height(
    center: &ChunkSnapshot,
    neighbors: &NeighborChunks,
    local: LocalBlockCoord,
    registry: &BlockRegistry,
) -> f32 {
    surface_height_at_offset(center, neighbors, local, (0, 0, 0), registry).unwrap_or(0.0)
}

fn block_at_offset(
    center: &ChunkSnapshot,
    neighbors: &NeighborChunks,
    local: LocalBlockCoord,
    offset: (i32, i32, i32),
) -> Option<BlockId> {
    let x = i32::from(local.x) + offset.0;
    let y = i32::from(local.y) + offset.1;
    let z = i32::from(local.z) + offset.2;

    let in_bounds = |value: i32| (0..CHUNK_EDGE_I32).contains(&value);
    if in_bounds(x) && in_bounds(y) && in_bounds(z) {
        return center.get_block(LocalBlockCoord::new(x as u8, y as u8, z as u8).unwrap());
    }

    let out_x = !in_bounds(x);
    let out_y = !in_bounds(y);
    let out_z = !in_bounds(z);
    if [out_x, out_y, out_z].into_iter().filter(|out| *out).count() != 1 {
        return None;
    }

    let snapshot = if out_x {
        if x < 0 {
            neighbors.neg_x.as_ref()
        } else {
            neighbors.pos_x.as_ref()
        }
    } else if out_y {
        if y < 0 {
            neighbors.neg_y.as_ref()
        } else {
            neighbors.pos_y.as_ref()
        }
    } else if z < 0 {
        neighbors.neg_z.as_ref()
    } else {
        neighbors.pos_z.as_ref()
    }?;

    let neighbor_x = wrap_local_coord(x)?;
    let neighbor_y = wrap_local_coord(y)?;
    let neighbor_z = wrap_local_coord(z)?;
    snapshot.get_block(LocalBlockCoord::new(neighbor_x, neighbor_y, neighbor_z).unwrap())
}

fn wrap_local_coord(value: i32) -> Option<u8> {
    if (0..CHUNK_EDGE_I32).contains(&value) {
        Some(value as u8)
    } else if value < 0 {
        Some((CHUNK_EDGE_I32 - 1) as u8)
    } else if value == CHUNK_EDGE_I32 {
        Some(0)
    } else {
        None
    }
}

fn face_offset(face: BlockFace) -> (i32, i32, i32) {
    match face {
        BlockFace::NegX => (-1, 0, 0),
        BlockFace::PosX => (1, 0, 0),
        BlockFace::NegY => (0, -1, 0),
        BlockFace::PosY => (0, 1, 0),
        BlockFace::NegZ => (0, 0, -1),
        BlockFace::PosZ => (0, 0, 1),
    }
}

fn shared_water_volume(current: &BlockDef, neighbor: &BlockDef) -> bool {
    current.is_rendered_cube()
        && neighbor.is_rendered_cube()
        && matches!(current.material, BlockMaterialKind::Water)
        && matches!(neighbor.material, BlockMaterialKind::Water)
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

fn extend_bounds(bounds: &mut Option<RenderBounds>, positions: &[[f32; 3]; 4]) {
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
                *bounds = Some(RenderBounds {
                    min: *position,
                    max: *position,
                });
            }
        }
    }
}

fn faces() -> [BlockFace; 6] {
    [
        BlockFace::NegX,
        BlockFace::PosX,
        BlockFace::NegY,
        BlockFace::PosY,
        BlockFace::NegZ,
        BlockFace::PosZ,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockRegistry, CHUNK_EDGE, ChunkData};

    fn test_registry() -> BlockRegistry {
        BlockRegistry::load_default().expect("default registry should load")
    }

    #[test]
    fn generated_plane_chunk_produces_non_empty_mesh() {
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        for x in 0..super::super::coord::CHUNK_EDGE as u8 {
            for z in 0..super::super::coord::CHUNK_EDGE as u8 {
                let local = LocalBlockCoord::new(x, 0, z).unwrap();
                chunk.set_block(local, BlockId::GRASS).unwrap();
            }
        }

        let mesh = build_chunk_mesh(
            &chunk.snapshot(),
            NeighborChunks::default(),
            &test_registry(),
        );

        assert!(!mesh.is_empty());
        assert!(mesh.triangle_count() > 0);
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.material_kind == BlockMaterialKind::Grass)
        );
        assert_eq!(
            mesh.bounds,
            Some(RenderBounds {
                min: [0.0, 0.0, 0.0],
                max: [CHUNK_EDGE as f32, 1.0, CHUNK_EDGE as f32],
            })
        );
    }

    #[test]
    fn flat_top_faces_merge_without_removing_height_edge_contours() {
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        for x in 0..super::super::coord::CHUNK_EDGE as u8 {
            for z in 0..super::super::coord::CHUNK_EDGE as u8 {
                let local = LocalBlockCoord::new(x, 0, z).unwrap();
                chunk.set_block(local, BlockId::GRASS).unwrap();
            }
        }

        let mesh = build_chunk_mesh(
            &chunk.snapshot(),
            NeighborChunks::default(),
            &test_registry(),
        );
        let top_vertices = mesh
            .vertices
            .iter()
            .filter(|vertex| vertex.normal == [0.0, 1.0, 0.0])
            .collect::<Vec<_>>();
        let top_face_count = top_vertices.len() / 4;

        assert!(top_face_count < CHUNK_EDGE * CHUNK_EDGE);
        assert!(
            top_vertices
                .iter()
                .any(|vertex| vertex.uv[0] > 1.0 || vertex.uv[1] > 1.0)
        );
        assert!(top_vertices.iter().any(|vertex| vertex.contour_edges != 0));
    }

    #[test]
    fn fully_occluded_face_is_removed_by_neighbor_snapshot() {
        let mut center = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        let mut east = ChunkData::new_empty(ChunkCoord(1, 0, 0));
        let center_local = LocalBlockCoord::new(CHUNK_EDGE as u8 - 1, 0, 0).unwrap();
        let east_local = LocalBlockCoord::new(0, 0, 0).unwrap();
        center.set_block(center_local, BlockId::GRASS).unwrap();
        east.set_block(east_local, BlockId::GRASS).unwrap();
        let registry = test_registry();

        let without_neighbor =
            build_chunk_mesh(&center.snapshot(), NeighborChunks::default(), &registry);
        let with_neighbor = build_chunk_mesh(
            &center.snapshot(),
            NeighborChunks {
                pos_x: Some(east.snapshot()),
                ..NeighborChunks::default()
            },
            &registry,
        );

        assert!(without_neighbor.triangle_count() > with_neighbor.triangle_count());
    }

    #[test]
    fn empty_snapshot_produces_empty_mesh() {
        let mesh = build_chunk_mesh(
            &ChunkData::new_empty(ChunkCoord(0, 0, 0)).snapshot(),
            NeighborChunks::default(),
            &test_registry(),
        );
        assert!(mesh.is_empty());
        assert!(mesh.bounds.is_none());
    }

    #[test]
    fn uniform_opaque_chunk_without_neighbors_emits_boundary_shell() {
        let chunk = ChunkData::new_filled(ChunkCoord(0, 0, 0), BlockId::STONE);

        let mesh = build_chunk_mesh(
            &chunk.snapshot(),
            NeighborChunks::default(),
            &test_registry(),
        );

        assert_eq!(mesh.triangle_count(), 6 * CHUNK_EDGE * CHUNK_EDGE * 2);
        assert_eq!(
            mesh.bounds,
            Some(RenderBounds {
                min: [0.0, 0.0, 0.0],
                max: [CHUNK_EDGE as f32, CHUNK_EDGE as f32, CHUNK_EDGE as f32],
            })
        );
    }

    #[test]
    fn uniform_opaque_chunk_with_opaque_neighbors_produces_empty_mesh() {
        let center = ChunkData::new_filled(ChunkCoord(0, 0, 0), BlockId::STONE);
        let neighbors = NeighborChunks {
            neg_x: Some(ChunkData::new_filled(ChunkCoord(-1, 0, 0), BlockId::STONE).snapshot()),
            pos_x: Some(ChunkData::new_filled(ChunkCoord(1, 0, 0), BlockId::STONE).snapshot()),
            neg_y: Some(ChunkData::new_filled(ChunkCoord(0, -1, 0), BlockId::STONE).snapshot()),
            pos_y: Some(ChunkData::new_filled(ChunkCoord(0, 1, 0), BlockId::STONE).snapshot()),
            neg_z: Some(ChunkData::new_filled(ChunkCoord(0, 0, -1), BlockId::STONE).snapshot()),
            pos_z: Some(ChunkData::new_filled(ChunkCoord(0, 0, 1), BlockId::STONE).snapshot()),
        };

        let mesh = build_chunk_mesh(&center.snapshot(), neighbors, &test_registry());

        assert!(mesh.is_empty());
    }

    #[test]
    fn lowered_water_top_marks_real_top_face_edges() {
        let registry = test_registry();
        let water = registry
            .block_id("water")
            .expect("water block should exist");
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        chunk
            .set_block(LocalBlockCoord::new(0, 0, 0).unwrap(), water)
            .unwrap();
        chunk
            .set_block(LocalBlockCoord::new(1, 0, 0).unwrap(), BlockId::GRASS)
            .unwrap();

        let mesh = build_chunk_mesh(&chunk.snapshot(), NeighborChunks::default(), &registry);
        let top_face_vertices = mesh
            .vertices
            .iter()
            .filter(|vertex| {
                vertex.material_kind == BlockMaterialKind::Water
                    && vertex.normal == [0.0, 1.0, 0.0]
                    && (vertex.position[1] - 0.9).abs() <= 0.0001
            })
            .collect::<Vec<_>>();

        assert_eq!(top_face_vertices.len(), 4);
        assert!(
            top_face_vertices
                .iter()
                .all(|vertex| vertex.contour_edges != 0)
        );
        assert!(
            top_face_vertices
                .iter()
                .all(|vertex| vertex.contour_edges & TOP_EDGE_POS_X != 0)
        );
    }

    #[test]
    fn foliage_cross_vine_emits_two_double_sided_alpha_quads() {
        let registry = test_registry();
        let vine = registry
            .block_id("swamp_cypress_vine")
            .expect("tree vine should exist");
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        chunk
            .set_block(LocalBlockCoord::new(0, 0, 0).unwrap(), vine)
            .unwrap();

        let mesh = build_chunk_mesh(&chunk.snapshot(), NeighborChunks::default(), &registry);

        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(mesh.triangle_count(), 8);
        assert_eq!(
            mesh.bounds,
            Some(RenderBounds {
                min: [0.0, 0.0, 0.0],
                max: [1.0, 1.0, 1.0],
            })
        );
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.material_kind == BlockMaterialKind::Foliage)
        );
    }

    #[test]
    fn stacked_water_keeps_full_submerged_height_and_only_exposes_upper_side_strip() {
        let registry = test_registry();
        let water = registry
            .block_id("water")
            .expect("water block should exist");
        let mut center = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        let mut east = ChunkData::new_empty(ChunkCoord(1, 0, 0));
        center
            .set_block(
                LocalBlockCoord::new(CHUNK_EDGE as u8 - 1, 0, 0).unwrap(),
                water,
            )
            .unwrap();
        center
            .set_block(
                LocalBlockCoord::new(CHUNK_EDGE as u8 - 1, 1, 0).unwrap(),
                water,
            )
            .unwrap();
        east.set_block(LocalBlockCoord::new(0, 0, 0).unwrap(), water)
            .unwrap();

        let mesh = build_chunk_mesh(
            &center.snapshot(),
            NeighborChunks {
                pos_x: Some(east.snapshot()),
                ..NeighborChunks::default()
            },
            &registry,
        );

        assert!(mesh.vertices.iter().any(|vertex| {
            vertex.material_kind == BlockMaterialKind::Water
                && (vertex.position[1] - 1.9).abs() <= HEIGHT_EPSILON
        }));
        assert!(mesh.vertices.iter().any(|vertex| {
            vertex.material_kind == BlockMaterialKind::Water
                && vertex.normal == [1.0, 0.0, 0.0]
                && vertex.position[0] >= CHUNK_EDGE as f32
                && vertex.position[1] > 0.89
                && vertex.position[1] <= 1.0 + HEIGHT_EPSILON
        }));
    }
}
