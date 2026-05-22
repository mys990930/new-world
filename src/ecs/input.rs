use bevy_ecs::prelude::{Query, Res, ResMut, Resource, With};

use super::command::{PlayerCommand, PlayerCommandBuffer};
use super::inventory::PlayerInventory;
use super::player::{LocalPlayerEntity, Player};

#[derive(Resource, Debug, Clone, Default)]
pub struct EcsInputSnapshot {
    pub move_screen_x: i8,
    pub move_screen_y: i8,
    pub sprint_down: bool,
    pub jump_just_pressed: bool,
    pub zoom_scroll_delta: f32,
    pub quickslot_scroll_steps: i8,
    pub primary_down: bool,
    pub primary_just_pressed: bool,
    pub secondary_down: bool,
    pub secondary_just_pressed: bool,
    pub rotate_camera: i8,
    pub recenter_camera: bool,
    pub toggle_manipulation_mode: bool,
    pub toggle_inventory: bool,
    pub select_quickslot: Option<u8>,
    pub cursor_screen_pos: (f64, f64),
    pub cursor_screen_delta: (f64, f64),
    pub focused: bool,
    pub active: bool,
}

pub(crate) fn interpret_input_system(
    input: Res<EcsInputSnapshot>,
    local_player: Option<Res<LocalPlayerEntity>>,
    inventories: Query<&PlayerInventory, With<Player>>,
    mut command_buffer: ResMut<PlayerCommandBuffer>,
) {
    if !input.active || !input.focused {
        return;
    }

    if input.toggle_manipulation_mode {
        command_buffer.0.push(PlayerCommand::ToggleManipulationMode);
    }

    if input.toggle_inventory {
        command_buffer.0.push(PlayerCommand::ToggleInventory);
    }

    if input.quickslot_scroll_steps != 0 {
        command_buffer.0.push(PlayerCommand::CycleQuickslot {
            delta: input.quickslot_scroll_steps,
        });
    }

    if let Some(slot_index) = input.select_quickslot {
        command_buffer
            .0
            .push(PlayerCommand::SelectQuickslot { slot_index });
    }

    let inventory_open = local_player
        .as_deref()
        .and_then(|local_player| local_player.0)
        .and_then(|entity| inventories.get(entity).ok())
        .is_some_and(|inventory| inventory.inventory_open);
    if inventory_open {
        return;
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
