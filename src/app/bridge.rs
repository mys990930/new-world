use winit::keyboard::KeyCode;

use super::GameApp;
use crate::ecs::{
    EcsInputSnapshot, QUARTER_VIEW_VERTICAL_WORLD_SIZE, quarter_view_basis, quarter_view_eye,
};
use crate::renderer::{
    ChunkCoord as RenderChunkCoord, CpuMesh as RenderCpuMesh, MeshVertex as RenderMeshVertex,
    RenderCameraState, RenderCubeInstance, RenderProjectionMode, RenderUploadRequest,
    RenderViewBasis,
};
use crate::world::{
    BlockFace, ChunkCoord as WorldChunkCoord, CpuMesh as WorldCpuMesh, MeshVertex as WorldMeshVertex,
    WorldBlockCoord,
};

pub struct AppRenderFrameData {
    pub camera: RenderCameraState,
    pub visible_chunks: Vec<RenderChunkCoord>,
    pub cube_instances: Vec<RenderCubeInstance>,
}

impl GameApp {
    pub fn bridge_platform_to_ecs(&mut self) {
        let window = self.platform.window_state();
        let input = self.platform.raw_input_state();
        let lifecycle = self.platform.lifecycle_state();

        self.ecs.insert_resource(EcsInputSnapshot {
            move_screen_x: axis(
                input.pressed_keys.contains(&KeyCode::KeyA),
                input.pressed_keys.contains(&KeyCode::KeyD),
            ),
            move_screen_y: axis(
                input.pressed_keys.contains(&KeyCode::KeyW),
                input.pressed_keys.contains(&KeyCode::KeyS),
            ),
            primary_down: input.left_pressed,
            primary_just_pressed: input.left_just_pressed,
            secondary_down: input.right_pressed,
            secondary_just_pressed: input.right_just_pressed,
            rotate_camera: axis(
                input.just_pressed_keys.contains(&KeyCode::KeyQ),
                input.just_pressed_keys.contains(&KeyCode::KeyE),
            ),
            recenter_camera: input.just_pressed_keys.contains(&KeyCode::KeyY),
            cursor_screen_pos: input.mouse_position,
            cursor_screen_delta: input.mouse_delta,
            focused: window.focused,
            active: lifecycle.active,
        });
    }

    pub fn bridge_ecs_to_render_frame(&self) -> AppRenderFrameData {
        let camera_state = self.ecs.camera_state();
        let target = self
            .ecs
            .local_player_transform()
            .map(|transform| transform.translation)
            .unwrap_or([0.0, 0.0, 0.0]);

        let camera = build_quarter_view_camera(target, camera_state.quarter_turns);
        let mut cube_instances = self
            .ecs
            .local_player_transform()
            .map(|transform| {
                vec![RenderCubeInstance {
                    center: transform.translation,
                    half_extents: [0.5, 0.5, 0.5],
                    color: [1.0, 1.0, 1.0, 1.0],
                }]
            })
            .unwrap_or_default();

        if let Some(player) = self.ecs.local_player_transform() {
            cube_instances.insert(
                0,
                build_ground_shadow_instance(player.translation),
            );
        }

        let selection = self.ecs.selection_state();
        if let (Some(block), Some(face)) = (selection.hovered_block, selection.hovered_face) {
            cube_instances.push(build_selection_face_instance(block, face));
        }

        AppRenderFrameData {
            camera,
            visible_chunks: self
                .ecs
                .visible_chunks()
                .into_iter()
                .map(world_chunk_to_render)
                .collect(),
            cube_instances,
        }
    }

    pub fn bridge_world_mesh_to_render_upload(
        &self,
        coord: WorldChunkCoord,
        mesh: WorldCpuMesh,
    ) -> RenderUploadRequest {
        RenderUploadRequest::UpsertChunkMesh {
            coord: world_chunk_to_render(coord),
            mesh: world_mesh_to_render(mesh),
        }
    }
}

fn axis(negative: bool, positive: bool) -> i8 {
    (positive as i8) - (negative as i8)
}

fn build_quarter_view_camera(target: [f32; 3], quarter_turns: u8) -> RenderCameraState {
    let basis = quarter_view_basis(quarter_turns);
    let eye = quarter_view_eye(target, quarter_turns);

    RenderCameraState {
        eye,
        target,
        up: basis.up,
        aspect_override: None,
        projection_mode: RenderProjectionMode::Orthographic {
            vertical_world_size: QUARTER_VIEW_VERTICAL_WORLD_SIZE,
        },
        basis_override: Some(RenderViewBasis {
            right: basis.right,
            up: basis.up,
            forward: basis.forward,
        }),
    }
}

fn build_ground_shadow_instance(center: [f32; 3]) -> RenderCubeInstance {
    let shadow_offset = [-0.18, 0.0, 0.12];
    RenderCubeInstance {
        center: [
            center[0] + shadow_offset[0],
            center[1] - 0.49,
            center[2] + shadow_offset[2],
        ],
        half_extents: [0.62, 0.01, 0.62],
        color: [0.08, 0.08, 0.10, 1.0],
    }
}

fn build_selection_face_instance(block: WorldBlockCoord, face: BlockFace) -> RenderCubeInstance {
    const HIGHLIGHT_HALF_THICKNESS: f32 = 0.02;
    const HIGHLIGHT_HALF_SPAN: f32 = 0.52;
    const HIGHLIGHT_FACE_OFFSET: f32 = 0.02;
    let center = [
        block.0 as f32 + 0.5,
        block.1 as f32 + 0.5,
        block.2 as f32 + 0.5,
    ];

    match face {
        BlockFace::NegX => RenderCubeInstance {
            center: [center[0] - 0.5 - HIGHLIGHT_FACE_OFFSET, center[1], center[2]],
            half_extents: [HIGHLIGHT_HALF_THICKNESS, HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_SPAN],
            color: [1.0, 0.92, 0.20, 1.0],
        },
        BlockFace::PosX => RenderCubeInstance {
            center: [center[0] + 0.5 + HIGHLIGHT_FACE_OFFSET, center[1], center[2]],
            half_extents: [HIGHLIGHT_HALF_THICKNESS, HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_SPAN],
            color: [1.0, 0.92, 0.20, 1.0],
        },
        BlockFace::NegY => RenderCubeInstance {
            center: [center[0], center[1] - 0.5 - HIGHLIGHT_FACE_OFFSET, center[2]],
            half_extents: [HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_THICKNESS, HIGHLIGHT_HALF_SPAN],
            color: [1.0, 0.92, 0.20, 1.0],
        },
        BlockFace::PosY => RenderCubeInstance {
            center: [center[0], center[1] + 0.5 + HIGHLIGHT_FACE_OFFSET, center[2]],
            half_extents: [HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_THICKNESS, HIGHLIGHT_HALF_SPAN],
            color: [1.0, 0.92, 0.20, 1.0],
        },
        BlockFace::NegZ => RenderCubeInstance {
            center: [center[0], center[1], center[2] - 0.5 - HIGHLIGHT_FACE_OFFSET],
            half_extents: [HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_THICKNESS],
            color: [1.0, 0.92, 0.20, 1.0],
        },
        BlockFace::PosZ => RenderCubeInstance {
            center: [center[0], center[1], center[2] + 0.5 + HIGHLIGHT_FACE_OFFSET],
            half_extents: [HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_SPAN, HIGHLIGHT_HALF_THICKNESS],
            color: [1.0, 0.92, 0.20, 1.0],
        },
    }
}

fn world_chunk_to_render(coord: WorldChunkCoord) -> RenderChunkCoord {
    RenderChunkCoord(coord.0, coord.1, coord.2)
}

fn world_mesh_to_render(mesh: WorldCpuMesh) -> RenderCpuMesh {
    RenderCpuMesh {
        vertices: mesh.vertices.into_iter().map(world_vertex_to_render).collect(),
        indices: mesh.indices,
        bounds: mesh.bounds.map(|bounds| crate::renderer::RenderBounds {
            min: bounds.min,
            max: bounds.max,
        }),
    }
}

fn world_vertex_to_render(vertex: WorldMeshVertex) -> RenderMeshVertex {
    RenderMeshVertex {
        position: vertex.position,
        color: vertex.color,
        normal: vertex.normal,
    }
}

#[cfg(test)]
fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_view_basis_maps_world_axes_to_expected_screen_directions() {
        let camera = build_quarter_view_camera([0.0, 0.5, 0.0], 0);
        let basis = camera.basis_override.expect("quarter-view basis should exist");

        let east = project_to_screen_axes([1.0, 0.0, 0.0], basis);
        let north = project_to_screen_axes([0.0, 0.0, 1.0], basis);
        let up = project_to_screen_axes([0.0, 1.0, 0.0], basis);

        assert!(east.0 > 0.0);
        assert!(east.1 < 0.0);
        assert!(north.0 > 0.0);
        assert!(north.1 > 0.0);
        assert!(up.0.abs() < 1e-5);
        assert!(up.1 > 0.0);
    }

    #[test]
    fn top_face_projects_as_cardinal_diamond() {
        let camera = build_quarter_view_camera([0.0, 0.5, 0.0], 0);
        let basis = camera.basis_override.expect("quarter-view basis should exist");

        let projected = [
            project_to_screen_axes([-0.5, 1.0, -0.5], basis),
            project_to_screen_axes([0.5, 1.0, -0.5], basis),
            project_to_screen_axes([0.5, 1.0, 0.5], basis),
            project_to_screen_axes([-0.5, 1.0, 0.5], basis),
        ];

        let left = projected
            .iter()
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .copied()
            .unwrap();
        let right = projected
            .iter()
            .max_by(|left, right| left.0.total_cmp(&right.0))
            .copied()
            .unwrap();
        let top = projected
            .iter()
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .copied()
            .unwrap();
        let bottom = projected
            .iter()
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .copied()
            .unwrap();

        assert!(left.0 < 0.0);
        assert!(right.0 > 0.0);
        assert!(top.1 > 0.0);
        assert!(bottom.1 > 0.0);
        assert!(left.1 > bottom.1);
        assert!(right.1 > bottom.1);
        assert!(top.0.abs() < 1e-5);
        assert!(bottom.0.abs() < 1e-5);
    }

    fn project_to_screen_axes(point: [f32; 3], basis: RenderViewBasis) -> (f32, f32) {
        (dot3(point, basis.right), dot3(point, basis.up))
    }
}
