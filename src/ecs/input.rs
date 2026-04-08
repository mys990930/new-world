use bevy_ecs::prelude::{Res, ResMut, Resource};

use super::command::{PlayerCommand, PlayerCommandBuffer};

#[derive(Resource, Debug, Clone, Default)]
pub struct EcsInputSnapshot {
    pub move_screen_x: i8,
    pub move_screen_y: i8,
    pub primary_down: bool,
    pub primary_just_pressed: bool,
    pub secondary_down: bool,
    pub secondary_just_pressed: bool,
    pub rotate_camera: i8,
    pub recenter_camera: bool,
    pub cursor_screen_pos: (f64, f64),
    pub cursor_screen_delta: (f64, f64),
    pub focused: bool,
    pub active: bool,
}

pub(crate) fn interpret_input_system(
    input: Res<EcsInputSnapshot>,
    mut command_buffer: ResMut<PlayerCommandBuffer>,
) {
    if !input.active || !input.focused {
        return;
    }

    if input.move_screen_x != 0 || input.move_screen_y != 0 {
        command_buffer.0.push(PlayerCommand::MoveScreen {
            x: input.move_screen_x,
            y: input.move_screen_y,
        });
    }

    if input.primary_just_pressed {
        command_buffer.0.push(PlayerCommand::PrimaryAction);
    }

    if input.secondary_just_pressed {
        command_buffer.0.push(PlayerCommand::PlaceBlock);
    }

    if input.rotate_camera != 0 {
        command_buffer.0.push(PlayerCommand::RotateCamera {
            quarter_turns: input.rotate_camera,
        });
    }

    if input.recenter_camera {
        command_buffer.0.push(PlayerCommand::RecenterCamera);
    }
}
