use bevy_ecs::prelude::{ResMut, Resource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerCommand {
    PrimaryAction,
    PlaceBlock,
    RotateCamera { quarter_turns: i8 },
    RecenterCamera,
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MoveWorldIntent {
    pub east: i8,
    pub north: i8,
}

#[derive(Resource, Debug, Default)]
pub struct PlayerCommandBuffer(pub Vec<PlayerCommand>);

pub(crate) fn clear_player_command_buffer_system(mut command_buffer: ResMut<PlayerCommandBuffer>) {
    command_buffer.0.clear();
}
