use bevy_ecs::prelude::{Query, Res, ResMut, Resource, With};

use super::command::PlayerCommand;
use super::inventory::PlayerInventory;
use super::input::EcsInputSnapshot;
use super::player::{FrameDeltaSeconds, LocalPlayerEntity, Player, Transform};
use super::{MoveWorldIntent, PlayerCommandBuffer};

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CameraState {
    pub quarter_turns: u8,
    pub smoothed_target: [f32; 3],
    pub desired_target: [f32; 3],
    pub render_yaw_radians: f32,
    pub desired_render_yaw_radians: f32,
    pub vertical_world_size: f32,
    pub desired_vertical_world_size: f32,
    pub recenter_requested: bool,
    pub recentering: bool,
    pub render_rotation_initialized: bool,
    pub initialized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuarterViewBasis {
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuarterViewCameraPose {
    pub target: [f32; 3],
    pub eye: [f32; 3],
    pub basis: QuarterViewBasis,
}

// For the gameplay quarter-view camera, this is the main framing/zoom handle.
// Larger values show more world around the focus plane and make the camera feel farther away.
pub const QUARTER_VIEW_VERTICAL_WORLD_SIZE: f32 = 20.0;
pub const QUARTER_VIEW_MIN_VERTICAL_WORLD_SIZE: f32 = 8.0;
pub const QUARTER_VIEW_MAX_VERTICAL_WORLD_SIZE: f32 = 48.0;
// The gameplay slice now uses a weak perspective projection tuned for better slope readability.
// This fixed FOV must stay aligned with the renderer config wired up by app bootstrap.
pub const QUARTER_VIEW_PERSPECTIVE_VERTICAL_FOV_RADIANS: f32 = 0.42;
// This legacy orthographic eye distance is still used by preview/debug helpers that keep an
// orthographic quarter-view camera. The main gameplay render/selection path no longer uses it.
pub const QUARTER_VIEW_CAMERA_DISTANCE: f32 = 520.0;
const CAMERA_UP_BASE: [f32; 3] = [-1.0, std::f32::consts::SQRT_2, 1.0];
const CAMERA_RIGHT_BASE: [f32; 3] = [1.0, 0.0, 1.0];
const CAMERA_DEADZONE_HALF_WIDTH: f32 = 0.75;
const CAMERA_DEADZONE_HALF_HEIGHT: f32 = 0.45;
const CAMERA_FORWARD_VIEW_RATIO: f32 = 0.65;
const CAMERA_FORWARD_BIAS_FROM_CENTER_RATIO: f32 = (CAMERA_FORWARD_VIEW_RATIO - 0.5) * 2.0;
const QUARTER_VIEW_CARDINAL_HALF_SPAN_MULTIPLIER: f32 = 1.732_050_8;
const CAMERA_FOLLOW_LERP_PER_SECOND: f32 = 2.0;
const CAMERA_RECENTER_LERP_PER_SECOND: f32 = 3.0;
// About 95% of the visual turn settles within roughly 200 ms.
const CAMERA_ROTATION_LERP_PER_SECOND: f32 = 15.0;
const CAMERA_ZOOM_LERP_PER_SECOND: f32 = 8.0 / 3.0;
const CAMERA_RECENTER_COMPLETE_DISTANCE: f32 = 0.02;
const CAMERA_ROTATION_COMPLETE_RADIANS: f32 = 0.001;
const CAMERA_ZOOM_WORLD_UNITS_PER_SCROLL_LINE: f32 = 2.0;
const CAMERA_SCROLL_PIXEL_DELTA_THRESHOLD: f32 = 8.0;
const CAMERA_MAX_SCROLL_LINES_PER_FRAME: f32 = 4.0;

impl Default for CameraState {
    fn default() -> Self {
        Self {
            quarter_turns: 0,
            smoothed_target: [0.0, 0.0, 0.0],
            desired_target: [0.0, 0.0, 0.0],
            render_yaw_radians: 0.0,
            desired_render_yaw_radians: 0.0,
            vertical_world_size: QUARTER_VIEW_VERTICAL_WORLD_SIZE,
            desired_vertical_world_size: QUARTER_VIEW_VERTICAL_WORLD_SIZE,
            recenter_requested: false,
            recentering: false,
            render_rotation_initialized: false,
            initialized: false,
        }
    }
}

pub(crate) fn clear_camera_impulses_system(mut camera: ResMut<CameraState>) {
    camera.recenter_requested = false;
}

pub(crate) fn apply_camera_zoom_input_system(
    input: Res<EcsInputSnapshot>,
    local_player: Option<Res<LocalPlayerEntity>>,
    inventories: Query<&PlayerInventory, With<Player>>,
    mut camera: ResMut<CameraState>,
) {
    if !input.active || !input.focused {
        return;
    }

    let inventory_open = local_player
        .as_deref()
        .and_then(|local_player| local_player.0)
        .and_then(|entity| inventories.get(entity).ok())
        .is_some_and(|inventory| inventory.inventory_open);
    if inventory_open {
        return;
    }

    let scroll_lines = normalized_scroll_lines(input.zoom_scroll_delta);
    if scroll_lines.abs() <= f32::EPSILON {
        return;
    }

    let next_vertical_world_size = clamp_vertical_world_size(
        camera.desired_vertical_world_size
            - scroll_lines * CAMERA_ZOOM_WORLD_UNITS_PER_SCROLL_LINE,
    );
    camera.desired_vertical_world_size = next_vertical_world_size;

    if !camera.initialized {
        camera.vertical_world_size = next_vertical_world_size;
    }
}

pub(crate) fn apply_camera_commands_system(
    command_buffer: Res<PlayerCommandBuffer>,
    mut camera: ResMut<CameraState>,
) {
    for command in &command_buffer.0 {
        match command {
            PlayerCommand::RotateCamera { quarter_turns } => {
                initialize_render_rotation_from_logical(&mut camera);
                camera.quarter_turns =
                    ((camera.quarter_turns as i8 + quarter_turns).rem_euclid(4)) as u8;
                camera.desired_render_yaw_radians +=
                    *quarter_turns as f32 * std::f32::consts::FRAC_PI_2;
            }
            PlayerCommand::RecenterCamera => {
                camera.recenter_requested = true;
            }
            PlayerCommand::PrimaryAction
            | PlayerCommand::PlaceBlock
            | PlayerCommand::ToggleManipulationMode
            | PlayerCommand::ToggleInventory
            | PlayerCommand::CycleQuickslot { .. }
            | PlayerCommand::SelectQuickslot { .. } => {}
        }
    }
}

pub(crate) fn update_camera_follow_system(
    local_player: Res<LocalPlayerEntity>,
    frame_delta: Res<FrameDeltaSeconds>,
    move_world_intent: Res<MoveWorldIntent>,
    mut camera: ResMut<CameraState>,
    players: Query<&Transform, With<Player>>,
) {
    initialize_render_rotation_from_logical(&mut camera);

    let dt = frame_delta.0.max(0.0);
    if dt > f32::EPSILON {
        let rotation_factor = smoothing_factor(CAMERA_ROTATION_LERP_PER_SECOND, dt);
        camera.render_yaw_radians = lerp_wrapped_angle(
            camera.render_yaw_radians,
            camera.desired_render_yaw_radians,
            rotation_factor,
        );

        if wrapped_angle_delta(camera.render_yaw_radians, camera.desired_render_yaw_radians).abs()
            <= CAMERA_ROTATION_COMPLETE_RADIANS
        {
            camera.render_yaw_radians = camera.desired_render_yaw_radians;
        }
    }

    let Some(entity) = local_player.0 else {
        return;
    };

    let Ok(player_transform) = players.get(entity) else {
        return;
    };

    let player_target = player_transform.translation;
    if !camera.initialized {
        camera.smoothed_target = player_target;
        camera.desired_target = player_target;
        camera.vertical_world_size = clamp_vertical_world_size(camera.desired_vertical_world_size);
        camera.desired_vertical_world_size = camera.vertical_world_size;
        camera.recentering = false;
        camera.initialized = true;
        return;
    }

    if camera.recenter_requested {
        camera.recentering = true;
    }

    let basis = quarter_view_basis(camera.quarter_turns);
    let desired_target = if camera.recentering {
        player_target
    } else {
        let mut anchor_target = add3(
            [
                player_target[0],
                camera.smoothed_target[1],
                player_target[2],
            ],
            movement_bias_offset(*move_world_intent, basis, camera.vertical_world_size),
        );
        anchor_target[1] = camera.smoothed_target[1];
        let mut desired_target =
            resolve_deadzone_target(camera.smoothed_target, anchor_target, basis);
        desired_target[1] = player_target[1];
        desired_target
    };

    camera.desired_target = desired_target;

    if dt > f32::EPSILON {
        let rate = if camera.recentering {
            CAMERA_RECENTER_LERP_PER_SECOND
        } else {
            CAMERA_FOLLOW_LERP_PER_SECOND
        };
        let factor = smoothing_factor(rate, dt);
        camera.smoothed_target = lerp3(camera.smoothed_target, desired_target, factor);
        let zoom_factor = smoothing_factor(CAMERA_ZOOM_LERP_PER_SECOND, dt);
        camera.vertical_world_size = lerp_scalar(
            camera.vertical_world_size,
            camera.desired_vertical_world_size,
            zoom_factor,
        );
    }

    if camera.recentering
        && distance_squared(camera.smoothed_target, player_target)
            <= CAMERA_RECENTER_COMPLETE_DISTANCE * CAMERA_RECENTER_COMPLETE_DISTANCE
    {
        camera.smoothed_target = player_target;
        camera.desired_target = player_target;
        camera.recentering = false;
    }
}

pub fn quarter_view_basis(quarter_turns: u8) -> QuarterViewBasis {
    let right = normalize3(rotate_y_quarter_turns(CAMERA_RIGHT_BASE, quarter_turns));
    let up = normalize3(rotate_y_quarter_turns(CAMERA_UP_BASE, quarter_turns));
    let forward = normalize3(cross3(right, up));

    QuarterViewBasis { right, up, forward }
}

pub fn quarter_view_camera_pose(camera: CameraState) -> QuarterViewCameraPose {
    let basis = quarter_view_basis(camera.quarter_turns);
    let vertical_world_size = quarter_view_vertical_world_size(camera);
    QuarterViewCameraPose {
        target: camera.smoothed_target,
        eye: quarter_view_perspective_eye(
            camera.smoothed_target,
            camera.quarter_turns,
            vertical_world_size,
        ),
        basis,
    }
}

pub fn quarter_view_render_camera_pose(camera: CameraState) -> QuarterViewCameraPose {
    let render_yaw = current_render_yaw_radians(camera);
    let basis = quarter_view_basis_from_yaw(render_yaw);
    let vertical_world_size = quarter_view_vertical_world_size(camera);
    QuarterViewCameraPose {
        target: camera.smoothed_target,
        eye: quarter_view_perspective_eye_from_yaw(
            camera.smoothed_target,
            render_yaw,
            vertical_world_size,
        ),
        basis,
    }
}

pub fn quarter_view_vertical_world_size(camera: CameraState) -> f32 {
    clamp_vertical_world_size(camera.vertical_world_size)
}

pub fn quarter_view_perspective_eye(
    target: [f32; 3],
    quarter_turns: u8,
    vertical_world_size: f32,
) -> [f32; 3] {
    quarter_view_perspective_eye_from_yaw(
        target,
        quarter_turn_yaw_radians(quarter_turns),
        vertical_world_size,
    )
}

pub fn quarter_view_perspective_distance(vertical_world_size: f32) -> f32 {
    let half_height = clamp_vertical_world_size(vertical_world_size) * 0.5;
    let half_fov = QUARTER_VIEW_PERSPECTIVE_VERTICAL_FOV_RADIANS * 0.5;
    half_height / half_fov.tan().max(0.001)
}

pub fn quarter_view_eye(target: [f32; 3], quarter_turns: u8) -> [f32; 3] {
    let basis = quarter_view_basis(quarter_turns);
    add3(
        target,
        scale3(basis.forward, -QUARTER_VIEW_CAMERA_DISTANCE),
    )
}

fn quarter_view_perspective_eye_from_yaw(
    target: [f32; 3],
    yaw_radians: f32,
    vertical_world_size: f32,
) -> [f32; 3] {
    let basis = quarter_view_basis_from_yaw(yaw_radians);
    add3(
        target,
        scale3(
            basis.forward,
            -quarter_view_perspective_distance(vertical_world_size),
        ),
    )
}

fn quarter_view_basis_from_yaw(yaw_radians: f32) -> QuarterViewBasis {
    let right = normalize3(rotate_y(CAMERA_RIGHT_BASE, yaw_radians));
    let up = normalize3(rotate_y(CAMERA_UP_BASE, yaw_radians));
    let forward = normalize3(cross3(right, up));

    QuarterViewBasis { right, up, forward }
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

fn rotate_y(vector: [f32; 3], yaw_radians: f32) -> [f32; 3] {
    let sin = yaw_radians.sin();
    let cos = yaw_radians.cos();
    [
        vector[0] * cos + vector[2] * sin,
        vector[1],
        -vector[0] * sin + vector[2] * cos,
    ]
}

fn quarter_turn_yaw_radians(quarter_turns: u8) -> f32 {
    quarter_turns as f32 * std::f32::consts::FRAC_PI_2
}

fn current_render_yaw_radians(camera: CameraState) -> f32 {
    if camera.render_rotation_initialized {
        camera.render_yaw_radians
    } else {
        quarter_turn_yaw_radians(camera.quarter_turns)
    }
}

fn initialize_render_rotation_from_logical(camera: &mut CameraState) {
    if camera.render_rotation_initialized {
        return;
    }

    let logical_yaw = quarter_turn_yaw_radians(camera.quarter_turns);
    camera.render_yaw_radians = logical_yaw;
    camera.desired_render_yaw_radians = logical_yaw;
    camera.render_rotation_initialized = true;
}

fn add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn scale3(vector: [f32; 3], scalar: f32) -> [f32; 3] {
    [vector[0] * scalar, vector[1] * scalar, vector[2] * scalar]
}

fn lerp_scalar(current: f32, target: f32, factor: f32) -> f32 {
    current + (target - current) * factor
}

fn lerp3(current: [f32; 3], target: [f32; 3], factor: f32) -> [f32; 3] {
    [
        current[0] + (target[0] - current[0]) * factor,
        current[1] + (target[1] - current[1]) * factor,
        current[2] + (target[2] - current[2]) * factor,
    ]
}

fn resolve_deadzone_target(
    current_target: [f32; 3],
    anchor_target: [f32; 3],
    basis: QuarterViewBasis,
) -> [f32; 3] {
    let delta = subtract3(anchor_target, current_target);
    let screen_right = dot3(delta, basis.right);
    let screen_up = dot3(delta, basis.up);
    let target_shift = add3(
        scale3(
            basis.right,
            signed_deadzone_excess(screen_right, CAMERA_DEADZONE_HALF_WIDTH),
        ),
        scale3(
            basis.up,
            signed_deadzone_excess(screen_up, CAMERA_DEADZONE_HALF_HEIGHT),
        ),
    );

    add3(current_target, target_shift)
}

fn signed_deadzone_excess(value: f32, half_extent: f32) -> f32 {
    if value > half_extent {
        value - half_extent
    } else if value < -half_extent {
        value + half_extent
    } else {
        0.0
    }
}

fn movement_bias_offset(
    move_world_intent: MoveWorldIntent,
    basis: QuarterViewBasis,
    vertical_world_size: f32,
) -> [f32; 3] {
    let move_world = [
        move_world_intent.east as f32,
        0.0,
        move_world_intent.north as f32,
    ];
    let screen_right = dot3(move_world, basis.right);
    let screen_up = dot3(move_world, basis.up);
    let screen_length_sq = screen_right * screen_right + screen_up * screen_up;
    if screen_length_sq <= f32::EPSILON {
        return [0.0, 0.0, 0.0];
    }

    let inv_length = screen_length_sq.sqrt().recip();
    let bias_distance = vertical_world_size
        * 0.5
        * QUARTER_VIEW_CARDINAL_HALF_SPAN_MULTIPLIER
        * CAMERA_FORWARD_BIAS_FROM_CENTER_RATIO;
    add3(
        scale3(
            basis.right,
            screen_right * inv_length * bias_distance,
        ),
        scale3(
            basis.up,
            screen_up * inv_length * bias_distance,
        ),
    )
}

fn smoothing_factor(rate_per_second: f32, dt: f32) -> f32 {
    1.0 - (-rate_per_second * dt).exp()
}

fn lerp_wrapped_angle(current: f32, target: f32, factor: f32) -> f32 {
    current + wrapped_angle_delta(current, target) * factor
}

fn wrapped_angle_delta(current: f32, target: f32) -> f32 {
    (target - current + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI
}

fn clamp_vertical_world_size(value: f32) -> f32 {
    value.clamp(
        QUARTER_VIEW_MIN_VERTICAL_WORLD_SIZE,
        QUARTER_VIEW_MAX_VERTICAL_WORLD_SIZE,
    )
}

fn normalized_scroll_lines(raw_delta: f32) -> f32 {
    if !raw_delta.is_finite() {
        return 0.0;
    }

    let normalized = if raw_delta.abs() > CAMERA_SCROLL_PIXEL_DELTA_THRESHOLD {
        raw_delta / 120.0
    } else {
        raw_delta
    };

    normalized.clamp(
        -CAMERA_MAX_SCROLL_LINES_PER_FRAME,
        CAMERA_MAX_SCROLL_LINES_PER_FRAME,
    )
}

fn normalize3(vector: [f32; 3]) -> [f32; 3] {
    let length_sq = vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2];
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

fn subtract3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn distance_squared(left: [f32; 3], right: [f32; 3]) -> f32 {
    let delta = subtract3(left, right);
    dot3(delta, delta)
}

#[cfg(test)]
mod tests {
    use bevy_ecs::prelude::{Schedule, World};

    use super::*;
    use crate::ecs::MoveWorldIntent;

    #[test]
    fn deadzone_keeps_target_still_for_small_offsets() {
        let basis = quarter_view_basis(0);
        let current_target = [0.0, 0.0, 0.0];
        let anchor_target = add3(
            current_target,
            add3(
                scale3(basis.right, CAMERA_DEADZONE_HALF_WIDTH * 0.5),
                scale3(basis.up, CAMERA_DEADZONE_HALF_HEIGHT * 0.5),
            ),
        );

        let desired = resolve_deadzone_target(current_target, anchor_target, basis);

        assert_eq!(desired, current_target);
    }

    #[test]
    fn deadzone_moves_only_by_excess_amount() {
        let basis = quarter_view_basis(0);
        let current_target = [0.0, 0.0, 0.0];
        let anchor_target = add3(
            current_target,
            scale3(basis.right, CAMERA_DEADZONE_HALF_WIDTH + 0.25),
        );

        let desired = resolve_deadzone_target(current_target, anchor_target, basis);
        let delta = subtract3(desired, current_target);

        assert!((dot3(delta, basis.right) - 0.25).abs() < 1e-5);
        assert!(dot3(delta, basis.up).abs() < 1e-5);
    }

    #[test]
    fn movement_bias_uses_camera_screen_plane() {
        let basis = quarter_view_basis(0);
        let bias = movement_bias_offset(
            MoveWorldIntent {
                east: 1,
                north: 0,
            },
            basis,
            QUARTER_VIEW_VERTICAL_WORLD_SIZE,
        );

        assert!(dot3(bias, basis.right) > 0.0);
        assert!(dot3(bias, basis.up) < 0.0);
    }

    #[test]
    fn movement_bias_targets_forward_heavy_composition() {
        let basis = quarter_view_basis(0);
        let bias = movement_bias_offset(
            MoveWorldIntent {
                east: 1,
                north: 0,
            },
            basis,
            QUARTER_VIEW_VERTICAL_WORLD_SIZE,
        );
        let screen_bias = [dot3(bias, basis.right), dot3(bias, basis.up)];
        let forward_screen = [dot3([1.0, 0.0, 0.0], basis.right), dot3([1.0, 0.0, 0.0], basis.up)];
        let forward_length = (forward_screen[0] * forward_screen[0]
            + forward_screen[1] * forward_screen[1])
            .sqrt();
        let normalized_forward = [
            forward_screen[0] / forward_length,
            forward_screen[1] / forward_length,
        ];
        let projected_bias =
            screen_bias[0] * normalized_forward[0] + screen_bias[1] * normalized_forward[1];
        let expected_bias = QUARTER_VIEW_VERTICAL_WORLD_SIZE
            * 0.5
            * QUARTER_VIEW_CARDINAL_HALF_SPAN_MULTIPLIER
            * CAMERA_FORWARD_BIAS_FROM_CENTER_RATIO;

        assert!((projected_bias - expected_bias).abs() < 1e-5);
    }

    #[test]
    fn recenter_smoothly_moves_camera_back_to_player_anchor() {
        let mut world = World::new();
        world.insert_resource(LocalPlayerEntity::default());
        world.insert_resource(FrameDeltaSeconds(0.25));
        world.insert_resource(MoveWorldIntent::default());
        world.insert_resource(CameraState {
            quarter_turns: 0,
            smoothed_target: [0.0, 1.5, 0.0],
            desired_target: [0.0, 1.5, 0.0],
            vertical_world_size: QUARTER_VIEW_VERTICAL_WORLD_SIZE,
            desired_vertical_world_size: QUARTER_VIEW_VERTICAL_WORLD_SIZE,
            recenter_requested: true,
            recentering: false,
            initialized: true,
            ..CameraState::default()
        });
        let entity = world
            .spawn((
                Player,
                Transform {
                    translation: [3.0, 1.5, 3.0],
                },
            ))
            .id();
        world.resource_mut::<LocalPlayerEntity>().0 = Some(entity);

        let mut schedule = Schedule::default();
        schedule.add_systems(update_camera_follow_system);
        schedule.run(&mut world);

        let camera = *world.resource::<CameraState>();
        assert!(camera.recentering);
        assert!(camera.smoothed_target[0] > 0.0);
        assert!(camera.smoothed_target[2] > 0.0);
        assert!(camera.smoothed_target[0] < 3.0);
        assert!(camera.smoothed_target[2] < 3.0);
        assert_eq!(camera.desired_target, [3.0, 1.5, 3.0]);
    }

    #[test]
    fn follow_camera_tracks_player_height_changes() {
        let mut world = World::new();
        world.insert_resource(LocalPlayerEntity::default());
        world.insert_resource(FrameDeltaSeconds(0.25));
        world.insert_resource(MoveWorldIntent::default());
        world.insert_resource(CameraState {
            quarter_turns: 0,
            smoothed_target: [3.0, 1.5, 3.0],
            desired_target: [3.0, 1.5, 3.0],
            vertical_world_size: QUARTER_VIEW_VERTICAL_WORLD_SIZE,
            desired_vertical_world_size: QUARTER_VIEW_VERTICAL_WORLD_SIZE,
            recenter_requested: false,
            recentering: false,
            initialized: true,
            ..CameraState::default()
        });
        let entity = world
            .spawn((
                Player,
                Transform {
                    translation: [3.0, 4.5, 3.0],
                },
            ))
            .id();
        world.resource_mut::<LocalPlayerEntity>().0 = Some(entity);

        let mut schedule = Schedule::default();
        schedule.add_systems(update_camera_follow_system);
        schedule.run(&mut world);

        let camera = *world.resource::<CameraState>();
        assert_eq!(camera.desired_target[1], 4.5);
        assert!(camera.smoothed_target[1] > 1.5);
        assert!(camera.smoothed_target[1] < 4.5);
    }

    #[test]
    fn scroll_zoom_updates_desired_vertical_world_size() {
        let mut world = World::new();
        world.insert_resource(EcsInputSnapshot {
            zoom_scroll_delta: 1.0,
            focused: true,
            active: true,
            ..EcsInputSnapshot::default()
        });
        world.insert_resource(CameraState::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_camera_zoom_input_system);
        schedule.run(&mut world);

        let camera = *world.resource::<CameraState>();
        assert!(camera.desired_vertical_world_size < QUARTER_VIEW_VERTICAL_WORLD_SIZE);
        assert_eq!(
            camera.vertical_world_size,
            camera.desired_vertical_world_size
        );
    }

    #[test]
    fn scroll_zoom_clamps_to_supported_range() {
        let mut world = World::new();
        world.insert_resource(EcsInputSnapshot {
            zoom_scroll_delta: 1200.0,
            focused: true,
            active: true,
            ..EcsInputSnapshot::default()
        });
        world.insert_resource(CameraState {
            desired_vertical_world_size: QUARTER_VIEW_MIN_VERTICAL_WORLD_SIZE + 0.5,
            vertical_world_size: QUARTER_VIEW_MIN_VERTICAL_WORLD_SIZE + 0.5,
            ..CameraState::default()
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_camera_zoom_input_system);
        schedule.run(&mut world);

        let camera = *world.resource::<CameraState>();
        assert_eq!(
            camera.desired_vertical_world_size,
            QUARTER_VIEW_MIN_VERTICAL_WORLD_SIZE
        );
    }

    #[test]
    fn render_rotation_lerps_after_logical_quarter_turn_snap() {
        let mut world = World::new();
        world.insert_resource(PlayerCommandBuffer(vec![PlayerCommand::RotateCamera {
            quarter_turns: 1,
        }]));
        world.insert_resource(LocalPlayerEntity::default());
        world.insert_resource(FrameDeltaSeconds(0.05));
        world.insert_resource(MoveWorldIntent::default());
        world.insert_resource(CameraState {
            quarter_turns: 0,
            smoothed_target: [2.0, 1.5, 2.0],
            desired_target: [2.0, 1.5, 2.0],
            vertical_world_size: QUARTER_VIEW_VERTICAL_WORLD_SIZE,
            desired_vertical_world_size: QUARTER_VIEW_VERTICAL_WORLD_SIZE,
            initialized: true,
            ..CameraState::default()
        });
        let entity = world
            .spawn((
                Player,
                Transform {
                    translation: [2.0, 1.5, 2.0],
                },
            ))
            .id();
        world.resource_mut::<LocalPlayerEntity>().0 = Some(entity);

        let mut update_schedule = Schedule::default();
        update_schedule.add_systems(apply_camera_commands_system);
        update_schedule.run(&mut world);

        let camera_after_command = *world.resource::<CameraState>();
        assert_eq!(camera_after_command.quarter_turns, 1);
        assert_eq!(camera_after_command.render_yaw_radians, 0.0);
        assert!((camera_after_command.desired_render_yaw_radians
            - std::f32::consts::FRAC_PI_2)
            .abs()
            < 1e-5);

        let mut post_update_schedule = Schedule::default();
        post_update_schedule.add_systems(update_camera_follow_system);
        post_update_schedule.run(&mut world);

        let camera = *world.resource::<CameraState>();
        assert!(camera.render_yaw_radians > 0.0);
        assert!(camera.render_yaw_radians < std::f32::consts::FRAC_PI_2);

        let render_pose = quarter_view_render_camera_pose(camera);
        let gameplay_pose = quarter_view_camera_pose(camera);
        assert!(render_pose.basis.right[0] > gameplay_pose.basis.right[0]);
        assert!(render_pose.basis.right[2] > gameplay_pose.basis.right[2]);
    }

    #[test]
    fn render_rotation_is_almost_settled_after_two_hundred_ms() {
        let mut world = World::new();
        world.insert_resource(LocalPlayerEntity::default());
        world.insert_resource(FrameDeltaSeconds(0.2));
        world.insert_resource(MoveWorldIntent::default());
        world.insert_resource(CameraState {
            render_yaw_radians: 0.0,
            desired_render_yaw_radians: std::f32::consts::FRAC_PI_2,
            render_rotation_initialized: true,
            initialized: true,
            ..CameraState::default()
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(update_camera_follow_system);
        schedule.run(&mut world);

        let camera = *world.resource::<CameraState>();
        assert!(
            camera.render_yaw_radians
                > std::f32::consts::FRAC_PI_2 * 0.94
        );
        assert!(camera.render_yaw_radians < std::f32::consts::FRAC_PI_2);
    }
}
