use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use super::{
    camera::CameraUniform, CameraUpdateError, ChunkCoord, MeshVertex, RenderCameraState,
    RenderSurfaceError, Renderer,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderCubeInstance {
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderFrameInput<'a> {
    pub camera: &'a RenderCameraState,
    pub visible_chunks: &'a [ChunkCoord],
    pub cube_instances: &'a [RenderCubeInstance],
    pub clear_color_override: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderStats {
    pub frame_index: u64,
    pub draw_call_count: u32,
    pub submitted_chunk_count: u32,
    pub visible_chunk_count: u32,
    pub uploaded_mesh_count: u32,
    pub removed_mesh_count: u32,
    pub presented: bool,
    pub clear_color: [f32; 4],
}

impl Default for RenderStats {
    fn default() -> Self {
        Self {
            frame_index: 0,
            draw_call_count: 0,
            submitted_chunk_count: 0,
            visible_chunk_count: 0,
            uploaded_mesh_count: 0,
            removed_mesh_count: 0,
            presented: false,
            clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
    Surface(RenderSurfaceError),
    Camera(CameraUpdateError),
}

impl Renderer {
    pub fn render(&mut self, frame: RenderFrameInput<'_>) -> Result<RenderStats, RenderError> {
        let frame_index = self.frame_index;
        self.frame_index = self.frame_index.saturating_add(1);

        let (uploaded_mesh_count, removed_mesh_count) = self.world.finish_frame();
        let clear_color = frame
            .clear_color_override
            .unwrap_or_else(|| self.config.clear_color.to_array());

        let visible_chunk_count = u32::try_from(frame.visible_chunks.len()).unwrap_or(u32::MAX);
        let mut stats = RenderStats {
            frame_index,
            draw_call_count: 0,
            submitted_chunk_count: 0,
            visible_chunk_count,
            uploaded_mesh_count,
            removed_mesh_count,
            presented: false,
            clear_color,
        };

        if !self.surface.is_configured() {
            self.last_stats = stats;
            return Ok(stats);
        }

        self.camera
            .update(
                frame.camera,
                &self.config.camera_projection,
                self.surface.width(),
                self.surface.height(),
                frame_index,
            )
            .map_err(RenderError::Camera)?;

        let submitted_chunk_count = frame
            .visible_chunks
            .iter()
            .filter(|coord| self.world.chunk_meshes.contains_key(coord))
            .count();
        let submitted_chunk_count = u32::try_from(submitted_chunk_count).unwrap_or(u32::MAX);

        stats.submitted_chunk_count = submitted_chunk_count;
        stats.draw_call_count = submitted_chunk_count;

        let Some(backend) = self.backend.as_mut() else {
            self.last_stats = stats;
            return Ok(stats);
        };

        let camera_uniform = CameraUniform::from_view_projection(self.camera.view_projection);
        backend
            .queue
            .write_buffer(&backend.camera_buffer, 0, cast_slice(&[camera_uniform]));

        let surface_texture = match backend.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Lost => {
                super::surface::reconfigure_surface_backend(
                    backend,
                    self.surface.width(),
                    self.surface.height(),
                );
                return Err(RenderError::Surface(RenderSurfaceError::Lost));
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                super::surface::reconfigure_surface_backend(
                    backend,
                    self.surface.width(),
                    self.surface.height(),
                );
                return Err(RenderError::Surface(RenderSurfaceError::Outdated));
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Err(RenderError::Surface(RenderSurfaceError::Timeout));
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(RenderError::Surface(RenderSurfaceError::Validation));
            }
        };

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = backend
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("renderer_frame_encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("renderer_main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear_color[0] as f64,
                            g: clear_color[1] as f64,
                            b: clear_color[2] as f64,
                            a: clear_color[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &backend.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&backend.cube_pipeline);
            render_pass.set_bind_group(0, &backend.camera_bind_group, &[]);

            if let Some((vertices, indices)) = build_cube_mesh(frame.cube_instances) {
                let vertex_buffer =
                    backend
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("renderer_cube_vertex_buffer"),
                            contents: cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                let index_buffer =
                    backend
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("renderer_cube_index_buffer"),
                            contents: cast_slice(&indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });

                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
                stats.draw_call_count = stats.draw_call_count.saturating_add(1);
            }

        }

        if let Some((edge_vertices, edge_indices)) =
            build_cube_edge_mesh(frame.cube_instances, frame.camera)
        {
            let edge_vertex_buffer =
                backend
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("renderer_cube_edge_vertex_buffer"),
                        contents: cast_slice(&edge_vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
            let edge_index_buffer =
                backend
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("renderer_cube_edge_index_buffer"),
                        contents: cast_slice(&edge_indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });

            let mut edge_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("renderer_edge_overlay_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            edge_pass.set_pipeline(&backend.cube_edge_pipeline);
            edge_pass.set_bind_group(0, &backend.camera_bind_group, &[]);
            edge_pass.set_vertex_buffer(0, edge_vertex_buffer.slice(..));
            edge_pass.set_index_buffer(edge_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            edge_pass.draw_indexed(0..edge_indices.len() as u32, 0, 0..1);
            stats.draw_call_count = stats.draw_call_count.saturating_add(1);
        }

        backend.queue.submit(Some(encoder.finish()));
        surface_texture.present();
        stats.presented = true;

        self.surface.mark_presented();
        self.last_stats = stats;
        Ok(stats)
    }
}

fn build_cube_mesh(cube_instances: &[RenderCubeInstance]) -> Option<(Vec<MeshVertex>, Vec<u32>)> {
    if cube_instances.is_empty() {
        return None;
    }

    let mut vertices = Vec::with_capacity(cube_instances.len() * 24);
    let mut indices = Vec::with_capacity(cube_instances.len() * 36);

    for cube in cube_instances {
        let [cx, cy, cz] = cube.center;
        let [hx, hy, hz] = cube.half_extents;
        let corners = [
            [cx - hx, cy - hy, cz - hz],
            [cx + hx, cy - hy, cz - hz],
            [cx + hx, cy + hy, cz - hz],
            [cx - hx, cy + hy, cz - hz],
            [cx - hx, cy - hy, cz + hz],
            [cx + hx, cy - hy, cz + hz],
            [cx + hx, cy + hy, cz + hz],
            [cx - hx, cy + hy, cz + hz],
        ];

        let face_specs = [
            ([4_u32, 5, 6, 7], shade_face(cube.color, 0.70, [0.08, 0.14, 0.22, 1.0])),
            ([1_u32, 0, 3, 2], shade_face(cube.color, 0.42, [0.02, 0.04, 0.10, 1.0])),
            ([0_u32, 4, 7, 3], shade_face(cube.color, 0.55, [0.04, 0.08, 0.16, 1.0])),
            ([5_u32, 1, 2, 6], shade_face(cube.color, 0.90, [0.12, 0.18, 0.28, 1.0])),
            ([3_u32, 7, 6, 2], shade_face(cube.color, 1.25, [0.92, 0.97, 1.0, 1.0])),
            ([0_u32, 1, 5, 4], shade_face(cube.color, 0.30, [0.01, 0.02, 0.05, 1.0])),
        ];

        for (corner_indices, face_color) in face_specs {
            let base_index = vertices.len() as u32;
            for corner_index in corner_indices {
                vertices.push(MeshVertex {
                    position: corners[corner_index as usize],
                    color: face_color,
                });
            }

            indices.extend_from_slice(&[
                base_index,
                base_index + 1,
                base_index + 2,
                base_index,
                base_index + 2,
                base_index + 3,
            ]);
        }
    }

    Some((vertices, indices))
}

fn build_cube_edge_mesh(
    cube_instances: &[RenderCubeInstance],
    camera: &RenderCameraState,
) -> Option<(Vec<MeshVertex>, Vec<u32>)> {
    if cube_instances.is_empty() {
        return None;
    }

    let mut vertices = Vec::with_capacity(cube_instances.len() * 8);
    let mut indices = Vec::with_capacity(cube_instances.len() * 24);

    for cube in cube_instances {
        let base_index = vertices.len() as u32;
        let [cx, cy, cz] = cube.center;
        let [hx, hy, hz] = cube.half_extents;
        let edge_color = [0.01, 0.01, 0.02, 1.0];
        let corners = [
            [cx - hx, cy - hy, cz - hz],
            [cx + hx, cy - hy, cz - hz],
            [cx + hx, cy + hy, cz - hz],
            [cx - hx, cy + hy, cz - hz],
            [cx - hx, cy - hy, cz + hz],
            [cx + hx, cy - hy, cz + hz],
            [cx + hx, cy + hy, cz + hz],
            [cx - hx, cy + hy, cz + hz],
        ];

        vertices.extend(corners.into_iter().map(|position| MeshVertex {
            position,
            color: edge_color,
        }));
        let view_to_eye = view_direction_towards_eye(camera);
        let edge_pairs = visible_edge_pairs(view_to_eye);

        for (a, b) in edge_pairs {
            indices.extend_from_slice(&[base_index + a, base_index + b]);
        }
    }

    Some((vertices, indices))
}

fn view_direction_towards_eye(camera: &RenderCameraState) -> [f32; 3] {
    match camera.basis_override {
        Some(basis) => scale3(basis.forward, -1.0),
        None => normalize3([
            camera.eye[0] - camera.target[0],
            camera.eye[1] - camera.target[1],
            camera.eye[2] - camera.target[2],
        ]),
    }
}

fn visible_edge_pairs(view_to_eye: [f32; 3]) -> Vec<(u32, u32)> {
    let mut edges = Vec::new();

    if view_to_eye[0] >= 0.0 {
        edges.extend_from_slice(&[(1, 5), (5, 6), (6, 2), (1, 2)]);
    } else {
        edges.extend_from_slice(&[(0, 4), (4, 7), (7, 3), (0, 3)]);
    }

    if view_to_eye[1] >= 0.0 {
        edges.extend_from_slice(&[(3, 2), (2, 6), (6, 7), (7, 3)]);
    } else {
        edges.extend_from_slice(&[(0, 1), (1, 5), (5, 4), (4, 0)]);
    }

    if view_to_eye[2] >= 0.0 {
        edges.extend_from_slice(&[(4, 5), (5, 6), (6, 7), (7, 4)]);
    } else {
        edges.extend_from_slice(&[(0, 1), (1, 2), (2, 3), (3, 0)]);
    }

    edges.sort_unstable();
    edges.dedup();
    edges
}

fn scale3(vector: [f32; 3], scalar: f32) -> [f32; 3] {
    [vector[0] * scalar, vector[1] * scalar, vector[2] * scalar]
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length_sq = vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2];
    if length_sq <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        scale3(vector, length_sq.sqrt().recip())
    }
}

fn shade_face(color: [f32; 4], brightness: f32, accent: [f32; 4]) -> [f32; 4] {
    [
        ((color[0] * brightness) * 0.75 + accent[0] * 0.25).clamp(0.0, 1.0),
        ((color[1] * brightness) * 0.75 + accent[1] * 0.25).clamp(0.0, 1.0),
        ((color[2] * brightness) * 0.75 + accent[2] * 0.25).clamp(0.0, 1.0),
        color[3],
    ]
}
