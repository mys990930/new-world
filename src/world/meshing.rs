use super::chunk::{BlockFace, BlockId, ChunkSnapshot};
use super::coord::{ChunkCoord, LocalBlockCoord, WorldBlockCoord, chunk_local_to_world};
use super::query::NeighborChunks;

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

pub fn build_chunk_mesh(center: &ChunkSnapshot, neighbors: NeighborChunks) -> CpuMesh {
    let mut mesh = CpuMesh::default();

    for y in 0..super::coord::CHUNK_EDGE as u8 {
        for z in 0..super::coord::CHUNK_EDGE as u8 {
            for x in 0..super::coord::CHUNK_EDGE as u8 {
                let local =
                    LocalBlockCoord::new(x, y, z).expect("chunk iteration must stay in bounds");
                let Some(block) = center.get_block(local) else {
                    continue;
                };
                if !block.is_solid() {
                    continue;
                }

                for face in faces() {
                    if neighbor_block(center, &neighbors, local, face)
                        .is_some_and(BlockId::is_solid)
                    {
                        continue;
                    }

                    append_face(&mut mesh, center.coord(), local, block, face);
                }
            }
        }
    }

    mesh
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

fn append_face(
    mesh: &mut CpuMesh,
    chunk: ChunkCoord,
    local: LocalBlockCoord,
    block: BlockId,
    face: BlockFace,
) {
    let world = chunk_local_to_world(chunk, local);
    let positions = face_positions(world, face);
    let base_index = mesh.vertices.len() as u32;
    let color = block.face_color(face);
    let normal = face_normal(face);

    extend_bounds(&mut mesh.bounds, &positions);

    for position in positions {
        mesh.vertices.push(MeshVertex {
            position,
            color,
            normal,
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

fn face_positions(world: WorldBlockCoord, face: BlockFace) -> [[f32; 3]; 4] {
    let min = [world.0 as f32, world.1 as f32, world.2 as f32];
    let max = [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0];

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
    use crate::world::ChunkData;

    #[test]
    fn generated_plane_chunk_produces_non_empty_mesh() {
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        for x in 0..super::super::coord::CHUNK_EDGE as u8 {
            for z in 0..super::super::coord::CHUNK_EDGE as u8 {
                let local = LocalBlockCoord::new(x, 0, z).unwrap();
                chunk.set_block(local, BlockId::Grass).unwrap();
            }
        }

        let mesh = build_chunk_mesh(&chunk.snapshot(), NeighborChunks::default());

        assert!(!mesh.is_empty());
        assert!(mesh.triangle_count() > 0);
        assert_eq!(
            mesh.bounds,
            Some(RenderBounds {
                min: [0.0, 0.0, 0.0],
                max: [16.0, 1.0, 16.0],
            })
        );
    }

    #[test]
    fn fully_occluded_face_is_removed_by_neighbor_snapshot() {
        let mut center = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        let mut east = ChunkData::new_empty(ChunkCoord(1, 0, 0));
        let center_local = LocalBlockCoord::new(15, 0, 0).unwrap();
        let east_local = LocalBlockCoord::new(0, 0, 0).unwrap();
        center.set_block(center_local, BlockId::Grass).unwrap();
        east.set_block(east_local, BlockId::Grass).unwrap();

        let without_neighbor = build_chunk_mesh(&center.snapshot(), NeighborChunks::default());
        let with_neighbor = build_chunk_mesh(
            &center.snapshot(),
            NeighborChunks {
                pos_x: Some(east.snapshot()),
                ..NeighborChunks::default()
            },
        );

        assert!(without_neighbor.triangle_count() > with_neighbor.triangle_count());
    }

    #[test]
    fn empty_snapshot_produces_empty_mesh() {
        let mesh = build_chunk_mesh(
            &ChunkData::new_empty(ChunkCoord(0, 0, 0)).snapshot(),
            NeighborChunks::default(),
        );
        assert!(mesh.is_empty());
        assert!(mesh.bounds.is_none());
    }
}
