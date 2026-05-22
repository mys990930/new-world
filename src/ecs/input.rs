use bevy_ecs::prelude::{Query, Res, ResMut, Resource, With};

use super::command::{PlayerCommand, PlayerCommandBuffer};
use super::inventory::{ManipulationMode, PlayerInventory};
use super::player::{FrameDeltaSeconds, LocalPlayerEntity, Player};
use super::tool_interaction::ToolUseCooldown;

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
    frame_delta: Res<FrameDeltaSeconds>,
    local_player: Option<Res<LocalPlayerEntity>>,
    inventories: Query<&PlayerInventory, With<Player>>,
    mut tool_cooldown: ResMut<ToolUseCooldown>,
    mut command_buffer: ResMut<PlayerCommandBuffer>,
) {
    tool_cooldown.tick(frame_delta.0);

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

    let player_inventory = local_player
        .as_deref()
        .and_then(|local_player| local_player.0)
        .and_then(|entity| inventories.get(entity).ok());
    if player_inventory.is_some_and(|inventory| inventory.inventory_open) {
        return;
    }

    let can_use_tool = player_inventory.is_some_and(|inventory| {
        matches!(inventory.manipulation_mode, ManipulationMode::Interaction)
            && inventory.selected_tool().is_some()
    });
    if input.primary_down && can_use_tool && tool_cooldown.ready() {
        command_buffer.0.push(PlayerCommand::PrimaryAction);
        tool_cooldown.trigger();
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

#[cfg(test)]
mod tests {
    use bevy_ecs::prelude::{Schedule, World};

    use super::*;
    use crate::ecs::{Player, PlayerCommandBuffer, ToolUseCooldown};

    fn world_with_input(input: EcsInputSnapshot, dt_seconds: f32) -> World {
        let mut world = World::new();
        world.insert_resource(input);
        world.insert_resource(FrameDeltaSeconds(dt_seconds));
        world.insert_resource(PlayerCommandBuffer::default());
        world.insert_resource(LocalPlayerEntity::default());
        world.insert_resource(ToolUseCooldown::default());
        let entity = world.spawn((Player, PlayerInventory::default())).id();
        world.resource_mut::<LocalPlayerEntity>().0 = Some(entity);
        world
    }

    #[test]
    fn held_primary_repeats_on_tool_cooldown() {
        let mut world = world_with_input(
            EcsInputSnapshot {
                primary_down: true,
                focused: true,
                active: true,
                ..EcsInputSnapshot::default()
            },
            0.25,
        );
        let mut schedule = Schedule::default();
        schedule.add_systems(interpret_input_system);

        schedule.run(&mut world);
        assert_eq!(
            world.resource::<PlayerCommandBuffer>().0,
            vec![PlayerCommand::PrimaryAction]
        );

        world.resource_mut::<PlayerCommandBuffer>().0.clear();
        schedule.run(&mut world);
        assert!(world.resource::<PlayerCommandBuffer>().0.is_empty());

        schedule.run(&mut world);
        assert_eq!(
            world.resource::<PlayerCommandBuffer>().0,
            vec![PlayerCommand::PrimaryAction]
        );
    }
}
