use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use bytemuck::{Pod, Zeroable};

use super::Renderer;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMaterialKind {
    GenericOpaque = 0,
    Grass = 1,
    Soil = 2,
    Stone = 3,
    Sand = 4,
    Foliage = 5,
    Water = 6,
    Emissive = 7,
    Actor = 8,
    Shadow = 9,
    Highlight = 10,
}

impl RenderMaterialKind {
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

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
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub texture_layer: u32,
    pub material_kind: u32,
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
        const ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x4,
            2 => Float32x3,
            3 => Float32x2,
            4 => Uint32,
            5 => Uint32
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeshVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GpuChunkMesh {
    pub vertex_count: u32,
    pub index_count: u32,
    pub triangle_count: u32,
    pub bounds: Option<RenderBounds>,
    pub upload_generation: u64,
    pub(crate) cpu_mesh: CpuMesh,
    pub(crate) buffers: Option<ChunkMeshBuffers>,
}

impl GpuChunkMesh {
    fn from_cpu_mesh(
        mesh: CpuMesh,
        upload_generation: u64,
        device: Option<&wgpu::Device>,
    ) -> Result<Self, RenderUploadError> {
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
        let buffers = device.map(|device| create_chunk_mesh_buffers(device, &mesh)).transpose()?;

        Ok(Self {
            vertex_count,
            index_count,
            triangle_count: index_count / 3,
            bounds: mesh.bounds,
            upload_generation,
            cpu_mesh: mesh,
            buffers,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ChunkMeshBuffers {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
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
                let gpu_mesh =
                    GpuChunkMesh::from_cpu_mesh(mesh, next_generation, self.backend_device())?;
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

    pub(crate) fn rebuild_chunk_mesh_buffers(&mut self) -> Result<(), RenderUploadError> {
        let Some(device) = self.backend_device().cloned() else {
            return Ok(());
        };

        for mesh in self.world.chunk_meshes.values_mut() {
            mesh.buffers = Some(create_chunk_mesh_buffers(&device, &mesh.cpu_mesh)?);
        }

        Ok(())
    }

    fn backend_device(&self) -> Option<&wgpu::Device> {
        self.backend.as_ref().map(|backend| &backend.device)
    }
}

fn create_chunk_mesh_buffers(
    device: &wgpu::Device,
    mesh: &CpuMesh,
) -> Result<ChunkMeshBuffers, RenderUploadError> {
    for index in &mesh.indices {
        if *index as usize >= mesh.vertices.len() {
            return Err(RenderUploadError::IndexOutOfRange {
                index: *index,
                vertex_count: mesh.vertices.len(),
            });
        }
    }

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("renderer_chunk_vertex_buffer"),
        contents: cast_slice(&mesh.vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("renderer_chunk_index_buffer"),
        contents: cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    Ok(ChunkMeshBuffers {
        vertex_buffer,
        index_buffer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_vertex_layout_matches_repr_c_memory() {
        let layout = MeshVertex::vertex_buffer_layout();

        assert_eq!(std::mem::size_of::<MeshVertex>(), 56);
        assert_eq!(layout.array_stride, 56);
        assert_eq!(layout.attributes.len(), 6);
        assert_eq!(layout.attributes[0].offset, 0);
        assert_eq!(layout.attributes[1].offset, 12);
        assert_eq!(layout.attributes[2].offset, 28);
        assert_eq!(layout.attributes[3].offset, 40);
        assert_eq!(layout.attributes[4].offset, 48);
        assert_eq!(layout.attributes[5].offset, 52);
    }
}
