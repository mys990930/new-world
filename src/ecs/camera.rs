use bevy_ecs::prelude::{Res, ResMut, Resource};

use super::command::PlayerCommand;
use super::PlayerCommandBuffer;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraState {
    pub quarter_turns: u8,
    pub recenter_boost_requested: bool,
}

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
