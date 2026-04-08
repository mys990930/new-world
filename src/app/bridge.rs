use winit::keyboard::KeyCode;

use super::GameApp;
use crate::ecs::EcsInputSnapshot;
use crate::renderer::{
    ChunkCoord, RenderCameraState, RenderCubeInstance, RenderProjectionMode, RenderViewBasis,
};

pub struct AppRenderFrameData {
    pub camera: RenderCameraState,
    pub visible_chunks: Vec<ChunkCoord>,
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
        let cube_instances = self
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

        AppRenderFrameData {
            camera,
            visible_chunks: Vec::new(),
            cube_instances,
        }
    }
}

fn axis(negative: bool, positive: bool) -> i8 {
    (positive as i8) - (negative as i8)
}

fn build_quarter_view_camera(target: [f32; 3], quarter_turns: u8) -> RenderCameraState {
    const CAMERA_DISTANCE: f32 = 18.0;
    const ORTHOGRAPHIC_VERTICAL_SIZE: f32 = 5.0;
    // The current prototype uses a 45-degree downward pitch and a 45-degree yaw-like
    // quarter-view basis so the top face reads as a diamond whose corners point
    // toward screen up/down/left/right.
    const CAMERA_UP_BASE: [f32; 3] = [-1.0, std::f32::consts::SQRT_2, 1.0];
    const CAMERA_RIGHT_BASE: [f32; 3] = [1.0, 0.0, 1.0];

    let right = normalize3(rotate_y_quarter_turns(CAMERA_RIGHT_BASE, quarter_turns));
    let up = normalize3(rotate_y_quarter_turns(CAMERA_UP_BASE, quarter_turns));
    // With an explicit basis override, `right` and `up` define the screen axes directly:
    // east -> screen bottom-right, north -> screen top-right.
    let forward = normalize3(cross3(right, up));
    let eye = add3(target, scale3(forward, -CAMERA_DISTANCE));

    RenderCameraState {
        eye,
        target,
        up,
        aspect_override: None,
        projection_mode: RenderProjectionMode::Orthographic {
            vertical_world_size: ORTHOGRAPHIC_VERTICAL_SIZE,
        },
        basis_override: Some(RenderViewBasis { right, up, forward }),
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

fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn scale3(vector: [f32; 3], scalar: f32) -> [f32; 3] {
    [vector[0] * scalar, vector[1] * scalar, vector[2] * scalar]
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length_sq = dot3(vector, vector);
    if length_sq <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        let inv_length = length_sq.sqrt().recip();
        scale3(vector, inv_length)
    }
}

fn cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

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
