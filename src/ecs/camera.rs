use bevy_ecs::prelude::{Query, Res, ResMut, Resource, With};

use super::command::PlayerCommand;
use super::player::{FrameDeltaSeconds, LocalPlayerEntity, Player, Transform};
use super::{MoveWorldIntent, PlayerCommandBuffer};

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CameraState {
    pub quarter_turns: u8,
    pub smoothed_target: [f32; 3],
    pub desired_target: [f32; 3],
    pub recenter_requested: bool,
    pub recentering: bool,
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

pub const QUARTER_VIEW_CAMERA_DISTANCE: f32 = 400.0;
pub const QUARTER_VIEW_VERTICAL_WORLD_SIZE: f32 = 5.0;
const CAMERA_UP_BASE: [f32; 3] = [-1.0, std::f32::consts::SQRT_2, 1.0];
const CAMERA_RIGHT_BASE: [f32; 3] = [1.0, 0.0, 1.0];
const CAMERA_DEADZONE_HALF_WIDTH: f32 = 0.75;
const CAMERA_DEADZONE_HALF_HEIGHT: f32 = 0.45;
const CAMERA_MOVE_BIAS_DISTANCE: f32 = 0.4;
const CAMERA_FOLLOW_LERP_PER_SECOND: f32 = 8.0;
const CAMERA_RECENTER_LERP_PER_SECOND: f32 = 12.0;
const CAMERA_RECENTER_COMPLETE_DISTANCE: f32 = 0.02;

impl Default for CameraState {
    fn default() -> Self {
        Self {
            quarter_turns: 0,
            smoothed_target: [0.0, 0.0, 0.0],
            desired_target: [0.0, 0.0, 0.0],
            recenter_requested: false,
            recentering: false,
            initialized: false,
        }
    }
}

pub(crate) fn clear_camera_impulses_system(mut camera: ResMut<CameraState>) {
    camera.recenter_requested = false;
}

pub(crate) fn apply_camera_commands_system(
    command_buffer: Res<PlayerCommandBuffer>,
    mut camera: ResMut<CameraState>,
) {
    for command in &command_buffer.0 {
        match command {
            PlayerCommand::RotateCamera { quarter_turns } => {
                camera.quarter_turns =
                    ((camera.quarter_turns as i8 + quarter_turns).rem_euclid(4)) as u8;
            }
            PlayerCommand::RecenterCamera => {
                camera.recenter_requested = true;
            }
            PlayerCommand::PrimaryAction | PlayerCommand::PlaceBlock => {}
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
        let anchor_target = add3(
            player_target,
            movement_bias_offset(*move_world_intent, basis),
        );
        resolve_deadzone_target(camera.smoothed_target, anchor_target, basis)
    };

    camera.desired_target = desired_target;

    let dt = frame_delta.0.max(0.0);
    if dt > f32::EPSILON {
        let rate = if camera.recentering {
            CAMERA_RECENTER_LERP_PER_SECOND
        } else {
            CAMERA_FOLLOW_LERP_PER_SECOND
        };
        let factor = smoothing_factor(rate, dt);
        camera.smoothed_target = lerp3(camera.smoothed_target, desired_target, factor);
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
    QuarterViewCameraPose {
        target: camera.smoothed_target,
        eye: quarter_view_eye(camera.smoothed_target, camera.quarter_turns),
        basis,
    }
}

pub fn quarter_view_eye(target: [f32; 3], quarter_turns: u8) -> [f32; 3] {
    let basis = quarter_view_basis(quarter_turns);
    add3(
        target,
        scale3(basis.forward, -QUARTER_VIEW_CAMERA_DISTANCE),
    )
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
    add3(
        scale3(
            basis.right,
            screen_right * inv_length * CAMERA_MOVE_BIAS_DISTANCE,
        ),
        scale3(
            basis.up,
            screen_up * inv_length * CAMERA_MOVE_BIAS_DISTANCE,
        ),
    )
}

fn smoothing_factor(rate_per_second: f32, dt: f32) -> f32 {
    1.0 - (-rate_per_second * dt).exp()
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
        );

        assert!(dot3(bias, basis.right) > 0.0);
        assert!(dot3(bias, basis.up) < 0.0);
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
            recenter_requested: true,
            recentering: false,
            initialized: true,
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
}
