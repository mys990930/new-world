use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use super::{
    camera::CameraUniform, CameraUpdateError, ChunkCoord, MeshVertex, RenderCameraState,
    RenderMaterialKind, RenderSurfaceError, Renderer,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderCubeInstance {
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
    pub color: [f32; 4],
    pub material_kind: RenderMaterialKind,
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
                    .and_then(|mesh| mesh.buffers.as_ref())
                    .is_some()
            })
            .count();
        let submitted_chunk_count = u32::try_from(submitted_chunk_count).unwrap_or(u32::MAX);

        stats.submitted_chunk_count = submitted_chunk_count;
        stats.draw_call_count = 0;

        let Some(backend) = self.backend.as_mut() else {
            self.last_stats = stats;
            return Ok(stats);
        };

        let camera_uniform = CameraUniform::from_view_projection_and_eye(
            self.camera.view_projection,
            self.camera.eye_position,
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

            render_pass.set_bind_group(0, &backend.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &backend.environment_bind_group, &[]);
            render_pass.set_bind_group(2, &backend.block_textures.bind_group, &[]);
            render_pass.set_pipeline(&backend.terrain_pipeline);

            for coord in frame.visible_chunks {
                let Some(chunk_mesh) = self.world.chunk_meshes.get(coord) else {
                    continue;
                };
                let Some(buffers) = chunk_mesh.buffers.as_ref() else {
                    continue;
                };

                render_pass.set_vertex_buffer(0, buffers.vertex_buffer.slice(..));
                render_pass.set_index_buffer(
                    buffers.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                render_pass.draw_indexed(0..chunk_mesh.index_count, 0, 0..1);
                stats.draw_call_count = stats.draw_call_count.saturating_add(1);
            }

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

                render_pass.set_pipeline(&backend.dynamic_cube_pipeline);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
                stats.draw_call_count = stats.draw_call_count.saturating_add(1);
            }

        }

        if self.config.debug.debug_overlay {
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

                edge_pass.set_pipeline(&backend.debug_edge_pipeline);
                edge_pass.set_bind_group(0, &backend.camera_bind_group, &[]);
                edge_pass.set_bind_group(1, &backend.environment_bind_group, &[]);
                edge_pass.set_bind_group(2, &backend.block_textures.bind_group, &[]);
                edge_pass.set_vertex_buffer(0, edge_vertex_buffer.slice(..));
                edge_pass
                    .set_index_buffer(edge_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                edge_pass.draw_indexed(0..edge_indices.len() as u32, 0, 0..1);
                stats.draw_call_count = stats.draw_call_count.saturating_add(1);
            }
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
            ([4_u32, 5, 6, 7], [0.0, 0.0, 1.0]),
            ([1_u32, 0, 3, 2], [0.0, 0.0, -1.0]),
            ([0_u32, 4, 7, 3], [-1.0, 0.0, 0.0]),
            ([5_u32, 1, 2, 6], [1.0, 0.0, 0.0]),
            ([3_u32, 7, 6, 2], [0.0, 1.0, 0.0]),
            ([0_u32, 1, 5, 4], [0.0, -1.0, 0.0]),
        ];
        let face_uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

        for (corner_indices, face_normal) in face_specs {
            let base_index = vertices.len() as u32;
            for (corner_index, uv) in corner_indices.into_iter().zip(face_uvs) {
                vertices.push(MeshVertex {
                    position: corners[corner_index as usize],
                    color: cube.color,
                    normal: face_normal,
                    uv,
                    texture_layer: 0,
                    material_kind: cube.material_kind.as_u32(),
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

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use pollster::block_on;
    use wgpu::util::DeviceExt;

    use super::*;
    use crate::renderer::{
        surface::{
            create_environment_bind_group_layout, default_environment_uniform, EnvironmentUniform,
        },
        CameraGpuState, CameraProjectionConfig, RenderProjectionMode, RenderViewBasis,
    };

    #[test]
    fn offscreen_cube_render_contains_visible_top_face_pixels() {
        let stats =
            block_on(render_cube_offscreen(true)).expect("offscreen render should succeed");

        assert!(stats.highlight_pixels > 0, "top face should contribute a highlight");
        assert!(stats.midtone_pixels > 0, "visible side faces should contribute midtones");
        assert!(
            stats.highlight_pixels + stats.midtone_pixels > stats.shadow_pixels,
            "lit cube pixels should dominate over shadow-only pixels"
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
        );
        queue.write_buffer(&camera_buffer, 0, cast_slice(&[camera_uniform]));

        let (vertices, indices) = build_cube_mesh(&[RenderCubeInstance {
            center: [0.0, 0.5, 0.0],
            half_extents: [0.5, 0.5, 0.5],
            color: [1.0, 1.0, 1.0, 1.0],
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
                if luminance >= 220 {
                    stats.highlight_pixels = stats.highlight_pixels.saturating_add(1);
                } else if luminance >= 110 {
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
