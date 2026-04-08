use bevy_ecs::prelude::{ResMut, Resource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerCommand {
    MoveScreen { x: i8, y: i8 },
    PrimaryAction,
    PlaceBlock,
    RotateCamera { quarter_turns: i8 },
    RecenterCamera,
}

#[derive(Resource, Debug, Default)]
pub struct PlayerCommandBuffer(pub Vec<PlayerCommand>);

pub(crate) fn clear_player_command_buffer_system(mut command_buffer: ResMut<PlayerCommandBuffer>) {
    command_buffer.0.clear();
}
