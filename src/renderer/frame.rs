use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use super::{
    camera::CameraUniform, CameraUpdateError, ChunkCoord, MeshVertex, RenderBounds,
    RenderCameraState, RenderMaterialKind, RenderSurfaceError, RenderUiSprite, Renderer,
    ui::UiVertex,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderCubeInstance {
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
    pub color: [f32; 4],
    pub top_texture_layer: u32,
    pub bottom_texture_layer: u32,
    pub side_texture_layer: u32,
    pub material_kind: RenderMaterialKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderFrameInput<'a> {
    pub camera: &'a RenderCameraState,
    pub draw_scene: bool,
    pub visible_chunks: &'a [ChunkCoord],
    pub cube_instances: &'a [RenderCubeInstance],
    pub ui_sprites: &'a [RenderUiSprite],
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
            .unwrap_or_else(|| self.environment.resolved_clear_color(self.config.clear_color));

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
            .filter(|coord| {
                self.world
                    .chunk_meshes
                    .get(coord)
                    .is_some_and(|mesh| {
                        mesh.opaque_buffers.is_some() || mesh.translucent_buffers.is_some()
                    })
            })
            .count();
        let submitted_chunk_count = u32::try_from(submitted_chunk_count).unwrap_or(u32::MAX);

        stats.submitted_chunk_count = submitted_chunk_count;
        stats.draw_call_count = 0;

        let dynamic_cube_mesh = frame.draw_scene.then(|| build_cube_mesh(frame.cube_instances)).flatten();
        let debug_edge_mesh = self
            .config
            .debug
            .debug_overlay
            .then(|| {
                frame
                    .draw_scene
                    .then(|| build_cube_edge_mesh(frame.cube_instances, frame.camera))
                    .flatten()
            })
            .flatten();
        let ui_sprite_mesh = build_ui_sprite_mesh(
            frame.ui_sprites,
            self.surface.width(),
            self.surface.height(),
        );
        let sun_shadow_uniform = if frame.draw_scene {
            build_sun_shadow_uniform(
                frame.camera,
                frame.visible_chunks,
                &self.world,
                frame.cube_instances,
                self.environment.current(),
                &self.config.quality,
            )
        } else {
            super::surface::SunShadowUniform::disabled()
        };

        let Some(backend) = self.backend.as_mut() else {
            self.last_stats = stats;
            return Ok(stats);
        };

        let camera_uniform = CameraUniform::from_view_projection_and_eye(
            self.camera.view_projection,
            self.camera.eye_position,
            self.camera.focus_position,
        );
        let environment_uniform = super::surface::EnvironmentUniform::from_settings(
            self.environment.current(),
            &self.config.quality,
        );
        backend
            .queue
            .write_buffer(&backend.camera_buffer, 0, cast_slice(&[camera_uniform]));
        backend.queue.write_buffer(
            &backend.environment_buffer,
            0,
            cast_slice(&[environment_uniform]),
        );
        backend.queue.write_buffer(
            &backend.shadow_uniform_buffer,
            0,
            cast_slice(&[sun_shadow_uniform]),
        );

        let dynamic_cube_buffers = dynamic_cube_mesh
            .as_ref()
            .map(|(vertices, indices)| {
                let vertex_buffer =
                    backend
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("renderer_cube_vertex_buffer"),
                            contents: cast_slice(vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                let index_buffer =
                    backend
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("renderer_cube_index_buffer"),
                            contents: cast_slice(indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });
                (vertex_buffer, index_buffer, indices.len() as u32)
            });
        let debug_edge_buffers = debug_edge_mesh
            .as_ref()
            .map(|(vertices, indices)| {
                let vertex_buffer =
                    backend
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("renderer_cube_edge_vertex_buffer"),
                            contents: cast_slice(vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                let index_buffer =
                    backend
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("renderer_cube_edge_index_buffer"),
                            contents: cast_slice(indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });
                (vertex_buffer, index_buffer, indices.len() as u32)
            });
        let ui_sprite_buffers = ui_sprite_mesh.as_ref().map(|(vertices, indices)| {
            let vertex_buffer = backend
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("renderer_ui_sprite_vertex_buffer"),
                    contents: cast_slice(vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let index_buffer = backend
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("renderer_ui_sprite_index_buffer"),
                    contents: cast_slice(indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            (vertex_buffer, index_buffer, indices.len() as u32)
        });

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

        if frame.draw_scene && sun_shadow_uniform.shadow_params[2] > 0.5 {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("renderer_shadow_depth_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &backend.shadow_map_view,
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

            shadow_pass.set_pipeline(&backend.shadow_depth_pipeline);
            shadow_pass.set_bind_group(0, &backend.shadow_pass_bind_group, &[]);

            for coord in frame.visible_chunks {
                let Some(chunk_mesh) = self.world.chunk_meshes.get(coord) else {
                    continue;
                };
                let Some(buffers) = chunk_mesh.opaque_buffers.as_ref() else {
                    continue;
                };

                shadow_pass.set_vertex_buffer(0, buffers.vertex_buffer.slice(..));
                shadow_pass.set_index_buffer(
                    buffers.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                shadow_pass.draw_indexed(0..chunk_mesh.opaque_index_count, 0, 0..1);
                stats.draw_call_count = stats.draw_call_count.saturating_add(1);
            }

            if let Some((vertex_buffer, index_buffer, index_count)) = dynamic_cube_buffers.as_ref() {
                shadow_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                shadow_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                shadow_pass.draw_indexed(0..*index_count, 0, 0..1);
                stats.draw_call_count = stats.draw_call_count.saturating_add(1);
            }
        }

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

            if frame.draw_scene {
                render_pass.set_pipeline(&backend.sun_overlay_pipeline);
                render_pass.set_bind_group(0, &backend.shadow_pass_bind_group, &[]);
                render_pass.draw(0..3, 0..1);
                stats.draw_call_count = stats.draw_call_count.saturating_add(1);

                render_pass.set_bind_group(0, &backend.camera_bind_group, &[]);
                render_pass.set_bind_group(1, &backend.environment_bind_group, &[]);
                render_pass.set_bind_group(2, &backend.block_textures.bind_group, &[]);
                render_pass.set_bind_group(3, &backend.shadow_sampling_bind_group, &[]);
                render_pass.set_pipeline(&backend.terrain_pipeline);

                for coord in frame.visible_chunks {
                    let Some(chunk_mesh) = self.world.chunk_meshes.get(coord) else {
                        continue;
                    };
                    let Some(buffers) = chunk_mesh.opaque_buffers.as_ref() else {
                        continue;
                    };

                    render_pass.set_vertex_buffer(0, buffers.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        buffers.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.draw_indexed(0..chunk_mesh.opaque_index_count, 0, 0..1);
                    stats.draw_call_count = stats.draw_call_count.saturating_add(1);
                }

                if let Some((vertex_buffer, index_buffer, index_count)) =
                    dynamic_cube_buffers.as_ref()
                {
                    render_pass.set_pipeline(&backend.dynamic_cube_pipeline);
                    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    render_pass
                        .set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..*index_count, 0, 0..1);
                    stats.draw_call_count = stats.draw_call_count.saturating_add(1);
                }

                render_pass.set_pipeline(&backend.water_pipeline);
                for coord in frame.visible_chunks {
                    let Some(chunk_mesh) = self.world.chunk_meshes.get(coord) else {
                        continue;
                    };
                    let Some(buffers) = chunk_mesh.translucent_buffers.as_ref() else {
                        continue;
                    };

                    render_pass.set_vertex_buffer(0, buffers.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        buffers.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.draw_indexed(0..chunk_mesh.translucent_index_count, 0, 0..1);
                    stats.draw_call_count = stats.draw_call_count.saturating_add(1);
                }
            }

        }

        if frame.draw_scene && self.config.debug.debug_overlay {
            if let Some((edge_vertex_buffer, edge_index_buffer, edge_index_count)) =
                debug_edge_buffers.as_ref()
            {
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

                edge_pass.set_pipeline(&backend.debug_edge_pipeline);
                edge_pass.set_bind_group(0, &backend.camera_bind_group, &[]);
                edge_pass.set_bind_group(1, &backend.environment_bind_group, &[]);
                edge_pass.set_bind_group(2, &backend.block_textures.bind_group, &[]);
                edge_pass.set_bind_group(3, &backend.shadow_sampling_bind_group, &[]);
                edge_pass.set_vertex_buffer(0, edge_vertex_buffer.slice(..));
                edge_pass
                    .set_index_buffer(edge_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                edge_pass.draw_indexed(0..*edge_index_count, 0, 0..1);
                stats.draw_call_count = stats.draw_call_count.saturating_add(1);
            }
        }

        if let Some((vertex_buffer, index_buffer, index_count)) = ui_sprite_buffers.as_ref() {
            let mut ui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("renderer_ui_overlay_pass"),
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
            ui_pass.set_pipeline(&backend.ui_sprite_pipeline);
            ui_pass.set_bind_group(0, &backend.ui_texture.bind_group, &[]);
            ui_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            ui_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            ui_pass.draw_indexed(0..*index_count, 0, 0..1);
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
            ([4_u32, 5, 6, 7], [0.0, 0.0, 1.0], cube.side_texture_layer),
            ([1_u32, 0, 3, 2], [0.0, 0.0, -1.0], cube.side_texture_layer),
            ([0_u32, 4, 7, 3], [-1.0, 0.0, 0.0], cube.side_texture_layer),
            ([5_u32, 1, 2, 6], [1.0, 0.0, 0.0], cube.side_texture_layer),
            ([3_u32, 7, 6, 2], [0.0, 1.0, 0.0], cube.top_texture_layer),
            ([0_u32, 1, 5, 4], [0.0, -1.0, 0.0], cube.bottom_texture_layer),
        ];
        let face_uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

        for (corner_indices, face_normal, texture_layer) in face_specs {
            let base_index = vertices.len() as u32;
            for (corner_index, uv) in corner_indices.into_iter().zip(face_uvs) {
                vertices.push(MeshVertex {
                    position: corners[corner_index as usize],
                    color: cube.color,
                    normal: face_normal,
                    uv,
                    texture_layer,
                    material_kind: cube.material_kind.as_u32(),
                    contour_edges: 0,
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
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            texture_layer: 0,
            material_kind: RenderMaterialKind::Highlight.as_u32(),
            contour_edges: 0,
        }));
        let view_to_eye = view_direction_towards_eye(camera);
        let edge_pairs = visible_edge_pairs(view_to_eye);

        for (a, b) in edge_pairs {
            indices.extend_from_slice(&[base_index + a, base_index + b]);
        }
    }

    Some((vertices, indices))
}

fn build_ui_sprite_mesh(
    ui_sprites: &[RenderUiSprite],
    surface_width: u32,
    surface_height: u32,
) -> Option<(Vec<UiVertex>, Vec<u32>)> {
    if ui_sprites.is_empty() || surface_width == 0 || surface_height == 0 {
        return None;
    }

    let mut vertices = Vec::with_capacity(ui_sprites.len() * 4);
    let mut indices = Vec::with_capacity(ui_sprites.len() * 6);
    let width = surface_width as f32;
    let height = surface_height as f32;

    for sprite in ui_sprites {
        let min_x = sprite.min_screen_px[0].clamp(0.0, width);
        let min_y = sprite.min_screen_px[1].clamp(0.0, height);
        let max_x = sprite.max_screen_px[0].clamp(0.0, width);
        let max_y = sprite.max_screen_px[1].clamp(0.0, height);

        if max_x <= min_x || max_y <= min_y {
            continue;
        }

        let x0 = (min_x / width) * 2.0 - 1.0;
        let x1 = (max_x / width) * 2.0 - 1.0;
        let y0 = 1.0 - (min_y / height) * 2.0;
        let y1 = 1.0 - (max_y / height) * 2.0;
        let base_index = vertices.len() as u32;

        vertices.extend_from_slice(&[
            UiVertex {
                position: [x0, y0],
                uv: [sprite.uv_min[0], sprite.uv_min[1]],
                tint: sprite.tint,
            },
            UiVertex {
                position: [x1, y0],
                uv: [sprite.uv_max[0], sprite.uv_min[1]],
                tint: sprite.tint,
            },
            UiVertex {
                position: [x1, y1],
                uv: [sprite.uv_max[0], sprite.uv_max[1]],
                tint: sprite.tint,
            },
            UiVertex {
                position: [x0, y1],
                uv: [sprite.uv_min[0], sprite.uv_max[1]],
                tint: sprite.tint,
            },
        ]);
        indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);
    }

    (!vertices.is_empty()).then_some((vertices, indices))
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

fn build_sun_shadow_uniform(
    camera: &RenderCameraState,
    visible_chunks: &[ChunkCoord],
    world: &super::RenderWorld,
    cube_instances: &[RenderCubeInstance],
    environment: &super::RenderEnvironment,
    quality: &super::RenderQualityConfig,
) -> super::surface::SunShadowUniform {
    let Some(shadow_map_size) = quality.shadow_map_size() else {
        let mut disabled = super::surface::SunShadowUniform::disabled();
        if let Some((sun_x, sun_y)) = project_sun_to_screen(camera, environment.sun_direction) {
            disabled.sun_screen_position_radius[0] = sun_x;
            disabled.sun_screen_position_radius[1] = sun_y;
        }
        disabled.sun_color_intensity = [
            environment.sun_color[0],
            environment.sun_color[1],
            environment.sun_color[2],
            environment.sun_intensity,
        ];
        disabled.sun_direction_shadow_strength = [
            environment.sun_direction[0],
            environment.sun_direction[1],
            environment.sun_direction[2],
            0.0,
        ];
        return disabled;
    };

    let sun_direction = normalize3(environment.sun_direction);
    let scene_bounds =
        scene_bounds_from_visible_geometry(visible_chunks, world, cube_instances)
            .unwrap_or_else(|| fallback_bounds_from_camera(camera));
    let center = [
        (scene_bounds.min[0] + scene_bounds.max[0]) * 0.5,
        (scene_bounds.min[1] + scene_bounds.max[1]) * 0.5,
        (scene_bounds.min[2] + scene_bounds.max[2]) * 0.5,
    ];
    let extents = [
        (scene_bounds.max[0] - scene_bounds.min[0]) * 0.5,
        (scene_bounds.max[1] - scene_bounds.min[1]) * 0.5,
        (scene_bounds.max[2] - scene_bounds.min[2]) * 0.5,
    ];
    let radius = extents[0].max(extents[1]).max(extents[2]).max(4.0);
    let light_eye = add3(center, scale3(sun_direction, radius * 3.0 + 24.0));
    let light_up = choose_light_up(sun_direction);
    let light_view = look_at_rh(light_eye, center, light_up);
    let mut light_space_bounds = transformed_bounds(scene_bounds, light_view);
    let xy_padding = radius * 0.35 + 2.0;
    light_space_bounds.min[0] -= xy_padding;
    light_space_bounds.max[0] += xy_padding;
    light_space_bounds.min[1] -= xy_padding;
    light_space_bounds.max[1] += xy_padding;

    let near_plane = (-light_space_bounds.max[2]).max(0.1);
    let far_plane = (-light_space_bounds.min[2]).max(near_plane + 0.1) + radius * 2.0;
    let light_projection = orthographic_bounds_rh(
        light_space_bounds.min[0],
        light_space_bounds.max[0],
        light_space_bounds.min[1],
        light_space_bounds.max[1],
        near_plane,
        far_plane,
    );
    let light_view_projection = multiply_matrix4(light_view, light_projection);
    let (sun_screen_x, sun_screen_y) =
        project_sun_to_screen(camera, sun_direction).unwrap_or((0.72, 0.66));
    let shadow_strength = match quality.tier {
        super::RenderQualityTier::Low => 0.0,
        super::RenderQualityTier::Medium => 0.62,
        super::RenderQualityTier::High => 0.78,
    };
    let sun_radius = match quality.tier {
        super::RenderQualityTier::Low => 0.06,
        super::RenderQualityTier::Medium => 0.075,
        super::RenderQualityTier::High => 0.085,
    };
    let halo_radius = sun_radius * 2.6;

    super::surface::SunShadowUniform {
        light_view_projection,
        sun_direction_shadow_strength: [
            sun_direction[0],
            sun_direction[1],
            sun_direction[2],
            shadow_strength,
        ],
        sun_color_intensity: [
            environment.sun_color[0],
            environment.sun_color[1],
            environment.sun_color[2],
            environment.sun_intensity,
        ],
        sun_screen_position_radius: [sun_screen_x, sun_screen_y, sun_radius, halo_radius],
        shadow_params: [0.0015, 1.0 / shadow_map_size as f32, 1.0, shadow_map_size as f32],
    }
}

fn scene_bounds_from_visible_geometry(
    visible_chunks: &[ChunkCoord],
    world: &super::RenderWorld,
    cube_instances: &[RenderCubeInstance],
) -> Option<RenderBounds> {
    let mut bounds = None;

    for coord in visible_chunks {
        let Some(chunk_mesh) = world.chunk_mesh(*coord) else {
            continue;
        };
        let Some(chunk_bounds) = chunk_mesh.bounds else {
            continue;
        };
        expand_bounds(&mut bounds, chunk_bounds);
    }

    for cube in cube_instances {
        let min = [
            cube.center[0] - cube.half_extents[0],
            cube.center[1] - cube.half_extents[1],
            cube.center[2] - cube.half_extents[2],
        ];
        let max = [
            cube.center[0] + cube.half_extents[0],
            cube.center[1] + cube.half_extents[1],
            cube.center[2] + cube.half_extents[2],
        ];
        expand_bounds(
            &mut bounds,
            RenderBounds {
                min,
                max,
            },
        );
    }

    bounds
}

fn fallback_bounds_from_camera(camera: &RenderCameraState) -> RenderBounds {
    RenderBounds {
        min: [
            camera.target[0] - 8.0,
            camera.target[1] - 2.0,
            camera.target[2] - 8.0,
        ],
        max: [
            camera.target[0] + 8.0,
            camera.target[1] + 10.0,
            camera.target[2] + 8.0,
        ],
    }
}

fn project_sun_to_screen(camera: &RenderCameraState, sun_direction: [f32; 3]) -> Option<(f32, f32)> {
    let view_projection = camera_view_projection(camera)?;
    let sun_anchor = add3(camera.target, scale3(normalize3(sun_direction), 96.0));
    let clip = multiply_row_vector(
        [sun_anchor[0], sun_anchor[1], sun_anchor[2], 1.0],
        view_projection,
    );
    let w = if clip[3].abs() <= f32::EPSILON { 1.0 } else { clip[3] };
    Some((clip[0] / w, clip[1] / w))
}

fn camera_view_projection(camera: &RenderCameraState) -> Option<[[f32; 4]; 4]> {
    let aspect = camera.aspect_override.unwrap_or(1.0).max(0.0001);
    let view = match camera.basis_override {
        Some(basis) => view_from_basis(camera.eye, basis)?,
        None => look_at_rh(camera.eye, camera.target, camera.up),
    };
    let projection = match camera.projection_mode {
        super::RenderProjectionMode::Perspective => perspective_rh(
            std::f32::consts::FRAC_PI_3,
            aspect,
            0.1,
            1_000.0,
        )?,
        super::RenderProjectionMode::Orthographic {
            vertical_world_size,
        } => orthographic_symmetric_rh(aspect, vertical_world_size, 0.1, 1_000.0)?,
    };
    Some(multiply_matrix4(view, projection))
}

fn expand_bounds(bounds: &mut Option<RenderBounds>, next: RenderBounds) {
    match bounds {
        Some(bounds) => {
            bounds.min[0] = bounds.min[0].min(next.min[0]);
            bounds.min[1] = bounds.min[1].min(next.min[1]);
            bounds.min[2] = bounds.min[2].min(next.min[2]);
            bounds.max[0] = bounds.max[0].max(next.max[0]);
            bounds.max[1] = bounds.max[1].max(next.max[1]);
            bounds.max[2] = bounds.max[2].max(next.max[2]);
        }
        None => *bounds = Some(next),
    }
}

fn transformed_bounds(bounds: RenderBounds, matrix: [[f32; 4]; 4]) -> RenderBounds {
    let mut transformed = None;
    for corner in bounds_corners(bounds) {
        let point = multiply_row_vector([corner[0], corner[1], corner[2], 1.0], matrix);
        expand_bounds(
            &mut transformed,
            RenderBounds {
                min: [point[0], point[1], point[2]],
                max: [point[0], point[1], point[2]],
            },
        );
    }
    transformed.expect("bounds corners should produce a transformed bound")
}

fn bounds_corners(bounds: RenderBounds) -> [[f32; 3]; 8] {
    let [min_x, min_y, min_z] = bounds.min;
    let [max_x, max_y, max_z] = bounds.max;
    [
        [min_x, min_y, min_z],
        [max_x, min_y, min_z],
        [min_x, max_y, min_z],
        [max_x, max_y, min_z],
        [min_x, min_y, max_z],
        [max_x, min_y, max_z],
        [min_x, max_y, max_z],
        [max_x, max_y, max_z],
    ]
}

fn choose_light_up(direction: [f32; 3]) -> [f32; 3] {
    if direction[1].abs() > 0.94 {
        [0.0, 0.0, 1.0]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn subtract3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn multiply_row_vector(vector: [f32; 4], matrix: [[f32; 4]; 4]) -> [f32; 4] {
    let mut result = [0.0; 4];
    for column in 0..4 {
        result[column] = (0..4).map(|index| vector[index] * matrix[index][column]).sum();
    }
    result
}

fn multiply_matrix4(left: [[f32; 4]; 4], right: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            result[row][col] = (0..4).map(|idx| left[row][idx] * right[idx][col]).sum();
        }
    }
    result
}

fn look_at_rh(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let forward = normalize3(subtract3(target, eye));
    let right = normalize3(cross3(forward, up));
    let recalculated_up = cross3(right, forward);

    [
        [right[0], recalculated_up[0], -forward[0], 0.0],
        [right[1], recalculated_up[1], -forward[1], 0.0],
        [right[2], recalculated_up[2], -forward[2], 0.0],
        [
            -dot3(right, eye),
            -dot3(recalculated_up, eye),
            dot3(forward, eye),
            1.0,
        ],
    ]
}

fn view_from_basis(
    eye: [f32; 3],
    basis: super::RenderViewBasis,
) -> Option<[[f32; 4]; 4]> {
    let right = normalize3(basis.right);
    let up = normalize3(basis.up);
    let forward = normalize3(basis.forward);

    Some([
        [right[0], up[0], -forward[0], 0.0],
        [right[1], up[1], -forward[1], 0.0],
        [right[2], up[2], -forward[2], 0.0],
        [
            -dot3(right, eye),
            -dot3(up, eye),
            dot3(forward, eye),
            1.0,
        ],
    ])
}

fn perspective_rh(
    vertical_fov_radians: f32,
    aspect_ratio: f32,
    near_plane: f32,
    far_plane: f32,
) -> Option<[[f32; 4]; 4]> {
    if vertical_fov_radians <= 0.0
        || aspect_ratio <= 0.0
        || near_plane <= 0.0
        || far_plane <= near_plane
    {
        return None;
    }

    let focal_length = 1.0 / (vertical_fov_radians * 0.5).tan();
    Some([
        [focal_length / aspect_ratio, 0.0, 0.0, 0.0],
        [0.0, focal_length, 0.0, 0.0],
        [0.0, 0.0, far_plane / (near_plane - far_plane), -1.0],
        [0.0, 0.0, (near_plane * far_plane) / (near_plane - far_plane), 0.0],
    ])
}

fn orthographic_symmetric_rh(
    aspect_ratio: f32,
    vertical_world_size: f32,
    near_plane: f32,
    far_plane: f32,
) -> Option<[[f32; 4]; 4]> {
    if aspect_ratio <= 0.0
        || vertical_world_size <= 0.0
        || near_plane <= 0.0
        || far_plane <= near_plane
    {
        return None;
    }

    let half_height = vertical_world_size * 0.5;
    let half_width = half_height * aspect_ratio;
    Some(orthographic_bounds_rh(
        -half_width,
        half_width,
        -half_height,
        half_height,
        near_plane,
        far_plane,
    ))
}

fn orthographic_bounds_rh(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near_plane: f32,
    far_plane: f32,
) -> [[f32; 4]; 4] {
    [
        [2.0 / (right - left), 0.0, 0.0, 0.0],
        [0.0, 2.0 / (top - bottom), 0.0, 0.0],
        [0.0, 0.0, 1.0 / (near_plane - far_plane), 0.0],
        [
            -(right + left) / (right - left),
            -(top + bottom) / (top - bottom),
            near_plane / (near_plane - far_plane),
            1.0,
        ],
    ]
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use pollster::block_on;
    use wgpu::util::DeviceExt;

    use super::*;
    use crate::renderer::{
        surface::{
            create_environment_bind_group_layout, create_shadow_sampling_bind_group_layout,
            default_environment_uniform, EnvironmentUniform, SunShadowUniform,
        },
        CameraGpuState, CameraProjectionConfig, RenderProjectionMode, RenderViewBasis,
    };

    #[test]
    fn offscreen_cube_render_contains_visible_top_face_pixels() {
        let stats =
            block_on(render_cube_offscreen(true)).expect("offscreen render should succeed");

        assert!(stats.highlight_pixels > 0, "top face should contribute a highlight");
        assert!(
            stats.midtone_pixels + stats.shadow_pixels > 0,
            "visible side faces should contribute non-highlight pixels"
        );
        assert!(
            stats.highlight_pixels + stats.midtone_pixels + stats.shadow_pixels
                > stats.highlight_pixels,
            "the cube should render more than a single flat face"
        );
    }

    #[derive(Debug, Default)]
    struct LitPixelStats {
        highlight_pixels: u32,
        midtone_pixels: u32,
        shadow_pixels: u32,
    }

    async fn render_cube_offscreen(use_depth: bool) -> Result<LitPixelStats, String> {
        if use_depth {
            render_cube_offscreen_with_depth_compare(wgpu::CompareFunction::LessEqual, 1.0).await
        } else {
            render_cube_offscreen_with_depth_compare(wgpu::CompareFunction::Always, 1.0).await
        }
    }

    async fn render_cube_offscreen_with_depth_compare(
        depth_compare: wgpu::CompareFunction,
        depth_clear: f32,
    ) -> Result<LitPixelStats, String> {
        const WIDTH: u32 = 256;
        const HEIGHT: u32 = 256;
        const BYTES_PER_PIXEL: u32 = 4;
        let use_depth = depth_compare != wgpu::CompareFunction::Always;

        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: None,
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|error| format!("adapter request failed: {error}"))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("offscreen_test_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .map_err(|error| format!("device request failed: {error}"))?;

        let color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_color_texture"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let depth_texture = use_depth.then(|| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("offscreen_depth_texture"),
                size: wgpu::Extent3d {
                    width: WIDTH,
                    height: HEIGHT,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        });
        let depth_view =
            depth_texture.as_ref().map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));

        let shader = device.create_shader_module(wgpu::include_wgsl!("player_cube.wgsl"));
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("offscreen_camera_buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("offscreen_camera_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("offscreen_camera_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let environment_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("offscreen_environment_buffer"),
            size: std::mem::size_of::<EnvironmentUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &environment_buffer,
            0,
            cast_slice(&[default_environment_uniform()]),
        );
        let environment_bind_group_layout = create_environment_bind_group_layout(&device);
        let environment_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("offscreen_environment_bind_group"),
            layout: &environment_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: environment_buffer.as_entire_binding(),
            }],
        });
        let shadow_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("offscreen_shadow_uniform_buffer"),
            size: std::mem::size_of::<SunShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &shadow_uniform_buffer,
            0,
            cast_slice(&[SunShadowUniform::disabled()]),
        );
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_shadow_texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            compare: Some(wgpu::CompareFunction::LessEqual),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let shadow_bind_group_layout = create_shadow_sampling_bind_group_layout(&device);
        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("offscreen_shadow_bind_group"),
            layout: &shadow_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });
        let block_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("offscreen_block_texture_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let block_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_block_texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &block_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let block_texture_view = block_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let block_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let block_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("offscreen_block_texture_bind_group"),
            layout: &block_texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&block_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&block_texture_sampler),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("offscreen_pipeline_layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&environment_bind_group_layout),
                Some(&block_texture_bind_group_layout),
                Some(&shadow_bind_group_layout),
            ],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("offscreen_cube_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[MeshVertex::vertex_buffer_layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: use_depth.then_some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(depth_compare),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let camera_state = RenderCameraState {
            eye: [9.0, 13.227922, -9.0],
            target: [0.0, 0.5, 0.0],
            up: [-0.5, std::f32::consts::FRAC_1_SQRT_2, 0.5],
            aspect_override: Some(1.0),
            projection_mode: RenderProjectionMode::Orthographic {
                vertical_world_size: 5.0,
            },
            basis_override: Some(RenderViewBasis {
                right: [std::f32::consts::FRAC_1_SQRT_2, 0.0, std::f32::consts::FRAC_1_SQRT_2],
                up: [-0.5, std::f32::consts::FRAC_1_SQRT_2, 0.5],
                forward: [
                    -0.5,
                    -std::f32::consts::FRAC_1_SQRT_2,
                    0.5,
                ],
            }),
        };
        let mut camera_gpu_state = CameraGpuState::default();
        camera_gpu_state
            .update(
                &camera_state,
                &CameraProjectionConfig::default(),
                WIDTH,
                HEIGHT,
                0,
            )
            .map_err(|error| format!("camera update failed: {error:?}"))?;
        let camera_uniform = CameraUniform::from_view_projection_and_eye(
            camera_gpu_state.view_projection,
            camera_gpu_state.eye_position,
            camera_gpu_state.focus_position,
        );
        queue.write_buffer(&camera_buffer, 0, cast_slice(&[camera_uniform]));

        let (vertices, indices) = build_cube_mesh(&[RenderCubeInstance {
            center: [0.0, 0.5, 0.0],
            half_extents: [0.5, 0.5, 0.5],
            color: [1.0, 1.0, 1.0, 1.0],
            top_texture_layer: 0,
            bottom_texture_layer: 0,
            side_texture_layer: 0,
            material_kind: RenderMaterialKind::Actor,
        }])
        .ok_or_else(|| "expected cube mesh".to_string())?;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("offscreen_vertex_buffer"),
            contents: cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("offscreen_index_buffer"),
            contents: cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let padded_bytes_per_row =
            (WIDTH * BYTES_PER_PIXEL).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("offscreen_output_buffer"),
            size: u64::from(padded_bytes_per_row * HEIGHT),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("offscreen_encoder"),
        });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("offscreen_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: depth_view.as_ref().map(|depth_view| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(depth_clear),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&pipeline);
            render_pass.set_bind_group(0, &camera_bind_group, &[]);
            render_pass.set_bind_group(1, &environment_bind_group, &[]);
            render_pass.set_bind_group(2, &block_texture_bind_group, &[]);
            render_pass.set_bind_group(3, &shadow_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &color_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &output_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(HEIGHT),
                },
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );

        let submission_index = queue.submit(Some(encoder.finish()));
        let slice = output_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission_index),
                timeout: None,
            })
            .map_err(|error| format!("device poll failed: {error:?}"))?;
        receiver
            .recv()
            .map_err(|error| format!("map callback failed: {error}"))?
            .map_err(|error| format!("buffer map failed: {error:?}"))?;

        let data = slice.get_mapped_range();
        let mut stats = LitPixelStats::default();
        for row in 0..HEIGHT as usize {
            let start = row * padded_bytes_per_row as usize;
            let row_bytes = &data[start..start + (WIDTH * BYTES_PER_PIXEL) as usize];
            for pixel in row_bytes.chunks_exact(4) {
                let [r, g, b, a]: [u8; 4] = pixel.try_into().unwrap();
                if a == 0 || (r == 0 && g == 0 && b == 0) {
                    continue;
                }

                let luminance = r.max(g).max(b);
                if luminance >= 245 {
                    stats.highlight_pixels = stats.highlight_pixels.saturating_add(1);
                } else if luminance >= 80 {
                    stats.midtone_pixels = stats.midtone_pixels.saturating_add(1);
                } else {
                    stats.shadow_pixels = stats.shadow_pixels.saturating_add(1);
                }
            }
        }
        drop(data);
        output_buffer.unmap();

        Ok(stats)
    }
}
