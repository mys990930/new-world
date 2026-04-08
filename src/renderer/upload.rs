use bytemuck::{Pod, Zeroable};

use super::Renderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoord(pub i32, pub i32, pub i32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
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
}

impl MeshVertex {
    pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeshVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuChunkMesh {
    pub vertex_count: u32,
    pub index_count: u32,
    pub triangle_count: u32,
    pub bounds: Option<RenderBounds>,
    pub upload_generation: u64,
}

impl GpuChunkMesh {
    fn from_cpu_mesh(mesh: CpuMesh, upload_generation: u64) -> Result<Self, RenderUploadError> {
        if mesh.vertices.is_empty() {
            return Err(RenderUploadError::EmptyVertexBuffer);
        }

        if mesh.indices.is_empty() {
            return Err(RenderUploadError::EmptyIndexBuffer);
        }

        for index in &mesh.indices {
            if *index as usize >= mesh.vertices.len() {
                return Err(RenderUploadError::IndexOutOfRange {
                    index: *index,
                    vertex_count: mesh.vertices.len(),
                });
            }
        }

        let vertex_count =
            u32::try_from(mesh.vertices.len()).map_err(|_| RenderUploadError::VertexCountOverflow)?;
        let index_count =
            u32::try_from(mesh.indices.len()).map_err(|_| RenderUploadError::IndexCountOverflow)?;

        Ok(Self {
            vertex_count,
            index_count,
            triangle_count: index_count / 3,
            bounds: mesh.bounds,
            upload_generation,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderUploadRequest {
    UpsertChunkMesh { coord: ChunkCoord, mesh: CpuMesh },
    RemoveChunkMesh { coord: ChunkCoord },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderUploadError {
    EmptyVertexBuffer,
    EmptyIndexBuffer,
    IndexOutOfRange { index: u32, vertex_count: usize },
    VertexCountOverflow,
    IndexCountOverflow,
}

impl Renderer {
    pub fn apply_upload(&mut self, request: RenderUploadRequest) -> Result<(), RenderUploadError> {
        match request {
            RenderUploadRequest::UpsertChunkMesh { coord, mesh } => {
                let next_generation = self.world.generation.saturating_add(1);
                let gpu_mesh = GpuChunkMesh::from_cpu_mesh(mesh, next_generation)?;
                self.world.chunk_meshes.insert(coord, gpu_mesh);
                self.world.generation = next_generation;
                self.world.uploaded_this_frame = self.world.uploaded_this_frame.saturating_add(1);
            }
            RenderUploadRequest::RemoveChunkMesh { coord } => {
                self.remove_chunk_mesh(coord);
            }
        }

        Ok(())
    }

    pub fn remove_chunk_mesh(&mut self, coord: ChunkCoord) {
        if self.world.chunk_meshes.remove(&coord).is_some() {
            self.world.generation = self.world.generation.saturating_add(1);
            self.world.removed_this_frame = self.world.removed_this_frame.saturating_add(1);
        }
    }
}
