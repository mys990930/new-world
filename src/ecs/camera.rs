use bevy_ecs::prelude::{Res, ResMut, Resource};

use super::command::PlayerCommand;
use super::PlayerCommandBuffer;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraState {
    pub quarter_turns: u8,
    pub recenter_boost_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuarterViewBasis {
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
}

pub const QUARTER_VIEW_CAMERA_DISTANCE: f32 = 400.0;
pub const QUARTER_VIEW_VERTICAL_WORLD_SIZE: f32 = 5.0;
const CAMERA_UP_BASE: [f32; 3] = [-1.0, std::f32::consts::SQRT_2, 1.0];
const CAMERA_RIGHT_BASE: [f32; 3] = [1.0, 0.0, 1.0];

impl Default for CameraState {
    fn default() -> Self {
        Self {
            quarter_turns: 0,
            recenter_boost_requested: false,
        }
    }
}

pub(crate) fn clear_camera_impulses_system(mut camera: ResMut<CameraState>) {
    camera.recenter_boost_requested = false;
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
            PlayerCommand::RecenterCamera
            | PlayerCommand::PrimaryAction
            | PlayerCommand::PlaceBlock => {
                camera.recenter_boost_requested = true;
            }
        }
    }
}

pub fn quarter_view_basis(quarter_turns: u8) -> QuarterViewBasis {
    let right = normalize3(rotate_y_quarter_turns(CAMERA_RIGHT_BASE, quarter_turns));
    let up = normalize3(rotate_y_quarter_turns(CAMERA_UP_BASE, quarter_turns));
    let forward = normalize3(cross3(right, up));

    QuarterViewBasis { right, up, forward }
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
