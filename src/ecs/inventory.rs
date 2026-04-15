use bevy_ecs::prelude::{Component, Query, Res, Resource, With, World};

use super::command::{PlayerCommand, PlayerCommandBuffer};
use super::player::{LocalPlayerEntity, Player};
use crate::world::BlockId;

pub const QUICKSLOT_COUNT: usize = 10;
pub const GENERAL_SLOT_COUNT: usize = 40;
pub const PLAYER_REACH_BLOCKS: f32 = 6.0;
pub const BUILD_REACH_BLOCKS: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ManipulationMode {
    #[default]
    Interaction,
    Build,
}

impl ManipulationMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Interaction => "INTERACTION",
            Self::Build => "BUILD",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Shovel,
    Pickaxe,
}

impl ToolKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Shovel => "SHOVEL",
            Self::Pickaxe => "PICKAXE",
        }
    }

    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Shovel => "SHVL",
            Self::Pickaxe => "PICK",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolPreviewShape {
    SingleBlock,
    FacePlane3x3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolSpec {
    pub range_blocks: f32,
    pub preview_shape: ToolPreviewShape,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct ToolCatalog {
    pub shovel: ToolSpec,
    pub pickaxe: ToolSpec,
}

impl Default for ToolCatalog {
    fn default() -> Self {
        Self {
            shovel: ToolSpec {
                range_blocks: PLAYER_REACH_BLOCKS,
                preview_shape: ToolPreviewShape::FacePlane3x3,
            },
            pickaxe: ToolSpec {
                range_blocks: PLAYER_REACH_BLOCKS,
                preview_shape: ToolPreviewShape::SingleBlock,
            },
        }
    }
}

impl ToolCatalog {
    pub const fn spec(self, tool: ToolKind) -> ToolSpec {
        match tool {
            ToolKind::Shovel => self.shovel,
            ToolKind::Pickaxe => self.pickaxe,
        }
    }

    pub const fn default_interaction_spec(self) -> ToolSpec {
        ToolSpec {
            range_blocks: PLAYER_REACH_BLOCKS,
            preview_shape: ToolPreviewShape::SingleBlock,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryItem {
    Tool(ToolKind),
    Block(BlockId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventorySlot {
    pub item: InventoryItem,
    pub count: u8,
}

impl InventorySlot {
    pub const fn tool(tool: ToolKind) -> Self {
        Self {
            item: InventoryItem::Tool(tool),
            count: 1,
        }
    }

    pub const fn block(block: BlockId, count: u8) -> Self {
        Self {
            item: InventoryItem::Block(block),
            count,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerInventory {
    pub general_slots: [Option<InventorySlot>; GENERAL_SLOT_COUNT],
    pub tool_quickslots: [Option<InventorySlot>; QUICKSLOT_COUNT],
    pub block_quickslots: [Option<InventorySlot>; QUICKSLOT_COUNT],
    pub selected_tool_slot: u8,
    pub selected_block_slot: u8,
    pub manipulation_mode: ManipulationMode,
    pub inventory_open: bool,
}

impl Default for PlayerInventory {
    fn default() -> Self {
        let mut inventory = Self {
            general_slots: [None; GENERAL_SLOT_COUNT],
            tool_quickslots: [None; QUICKSLOT_COUNT],
            block_quickslots: [None; QUICKSLOT_COUNT],
            selected_tool_slot: 0,
            selected_block_slot: 0,
            manipulation_mode: ManipulationMode::Interaction,
            inventory_open: false,
        };

        inventory.tool_quickslots[0] = Some(InventorySlot::tool(ToolKind::Shovel));
        inventory.tool_quickslots[1] = Some(InventorySlot::tool(ToolKind::Pickaxe));

        inventory.block_quickslots[0] = Some(InventorySlot::block(BlockId::GRASS, 64));
        inventory.block_quickslots[1] = Some(InventorySlot::block(BlockId::DIRT, 96));
        inventory.block_quickslots[2] = Some(InventorySlot::block(BlockId::STONE, 128));

        inventory.general_slots[0] = Some(InventorySlot::block(BlockId::DIRT, 255));
        inventory.general_slots[1] = Some(InventorySlot::block(BlockId::STONE, 192));
        inventory.general_slots[2] = Some(InventorySlot::block(BlockId::GRASS, 128));

        inventory
    }
}

impl PlayerInventory {
    pub fn active_selected_slot(self) -> usize {
        match self.manipulation_mode {
            ManipulationMode::Interaction => self.selected_tool_slot as usize,
            ManipulationMode::Build => self.selected_block_slot as usize,
        }
    }

    pub fn active_quickslots(self) -> [Option<InventorySlot>; QUICKSLOT_COUNT] {
        match self.manipulation_mode {
            ManipulationMode::Interaction => self.tool_quickslots,
            ManipulationMode::Build => self.block_quickslots,
        }
    }

    pub fn selected_tool(self) -> Option<ToolKind> {
        self.tool_quickslots
            .get(self.selected_tool_slot as usize)
            .copied()
            .flatten()
            .and_then(|slot| match slot.item {
                InventoryItem::Tool(tool) => Some(tool),
                InventoryItem::Block(_) => None,
            })
    }

    pub fn selected_block(self) -> Option<InventorySlot> {
        self.block_quickslots
            .get(self.selected_block_slot as usize)
            .copied()
            .flatten()
            .filter(|slot| matches!(slot.item, InventoryItem::Block(_)))
    }

    pub fn set_active_selected_slot(&mut self, slot_index: u8) {
        let clamped = slot_index.min((QUICKSLOT_COUNT - 1) as u8);
        match self.manipulation_mode {
            ManipulationMode::Interaction => self.selected_tool_slot = clamped,
            ManipulationMode::Build => self.selected_block_slot = clamped,
        }
    }

    pub fn cycle_active_selected_slot(&mut self, delta: i8) {
        if delta == 0 {
            return;
        }

        let current = self.active_selected_slot() as i32;
        let next = (current + delta as i32).rem_euclid(QUICKSLOT_COUNT as i32) as u8;
        self.set_active_selected_slot(next);
    }
}

pub(crate) fn apply_inventory_commands_system(
    local_player: Res<LocalPlayerEntity>,
    command_buffer: Res<PlayerCommandBuffer>,
    mut inventories: Query<&mut PlayerInventory, With<Player>>,
) {
    let Some(entity) = local_player.0 else {
        return;
    };
    let Ok(mut inventory) = inventories.get_mut(entity) else {
        return;
    };

    for command in &command_buffer.0 {
        match *command {
            PlayerCommand::ToggleManipulationMode => {
                inventory.manipulation_mode = match inventory.manipulation_mode {
                    ManipulationMode::Interaction => ManipulationMode::Build,
                    ManipulationMode::Build => ManipulationMode::Interaction,
                };
            }
            PlayerCommand::ToggleInventory => {
                inventory.inventory_open = !inventory.inventory_open;
            }
            PlayerCommand::CycleQuickslot { delta } => {
                inventory.cycle_active_selected_slot(delta);
            }
            PlayerCommand::SelectQuickslot { slot_index } => {
                inventory.set_active_selected_slot(slot_index);
            }
            PlayerCommand::PrimaryAction
            | PlayerCommand::PlaceBlock
            | PlayerCommand::RotateCamera { .. }
            | PlayerCommand::RecenterCamera => {}
        }
    }
}

pub(crate) fn local_player_inventory(world: &World) -> Option<PlayerInventory> {
    let entity = world.resource::<LocalPlayerEntity>().0?;
    world.get::<PlayerInventory>(entity).copied()
}

#[cfg(test)]
mod tests {
    use bevy_ecs::prelude::{Schedule, World};

    use super::*;
    use crate::ecs::PlayerCommandBuffer;

    #[test]
    fn toggling_mode_preserves_per_mode_selection() {
        let mut inventory = PlayerInventory::default();
        inventory.selected_tool_slot = 1;
        inventory.selected_block_slot = 4;

        inventory.manipulation_mode = ManipulationMode::Build;
        assert_eq!(inventory.active_selected_slot(), 4);

        inventory.manipulation_mode = ManipulationMode::Interaction;
        assert_eq!(inventory.active_selected_slot(), 1);
    }

    #[test]
    fn cycle_quickslot_wraps_for_active_mode_only() {
        let mut world = World::new();
        world.insert_resource(LocalPlayerEntity::default());
        let entity = world.spawn((Player, PlayerInventory::default())).id();
        world.resource_mut::<LocalPlayerEntity>().0 = Some(entity);
        world.insert_resource(PlayerCommandBuffer(vec![
            PlayerCommand::CycleQuickslot { delta: -1 },
            PlayerCommand::ToggleManipulationMode,
            PlayerCommand::CycleQuickslot { delta: 1 },
        ]));

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_inventory_commands_system);
        schedule.run(&mut world);

        let inventory = *world.get::<PlayerInventory>(entity).unwrap();
        assert_eq!(inventory.selected_tool_slot, 9);
        assert_eq!(inventory.selected_block_slot, 1);
        assert_eq!(inventory.manipulation_mode, ManipulationMode::Build);
    }
}
