use std::collections::HashMap;

use bevy_ecs::prelude::{Component, Entity, Resource, World};

use super::inventory::{InventoryItem, InventorySlot, ManipulationMode, PlayerInventory, ToolKind};
use super::player::{FrameDeltaSeconds, LocalPlayerEntity, PlayerBody, Transform};
use super::player_visual::trigger_voxel_player_tool_swing;
use crate::world::{BlockId, WorldBlockCoord, WorldCore, world_to_chunk_local};

pub const TOOL_USE_COOLDOWN_SECONDS: f32 = 0.5;
pub const BLOCK_DAMAGE_RECOVERY_SECONDS: f32 = 5.0;
pub const DEFAULT_BLOCK_HP: f32 = 3.0;
pub const SHOVEL_TOOL_DAMAGE: f32 = 1.0;
pub const PICKAXE_TOOL_DAMAGE: f32 = 3.0;
pub const BLOCK_DROP_PICKUP_RADIUS: f32 = 0.5;
pub const BLOCK_DROP_ATTRACT_RADIUS: f32 = 3.0;
pub const BLOCK_DROP_ATTRACT_SPEED: f32 = 8.0;
const BLOCK_DROP_RANDOM_OFFSET_RADIUS: f32 = 0.32;

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct ToolUseCooldown {
    pub remaining_seconds: f32,
}

impl Default for ToolUseCooldown {
    fn default() -> Self {
        Self {
            remaining_seconds: 0.0,
        }
    }
}

impl ToolUseCooldown {
    pub fn tick(&mut self, dt_seconds: f32) {
        self.remaining_seconds = (self.remaining_seconds - dt_seconds.max(0.0)).max(0.0);
    }

    pub fn ready(self) -> bool {
        self.remaining_seconds <= f32::EPSILON
    }

    pub fn trigger(&mut self) {
        self.remaining_seconds = TOOL_USE_COOLDOWN_SECONDS;
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct BlockDamageTracker {
    damaged_blocks: HashMap<WorldBlockCoord, BlockDamage>,
}

impl BlockDamageTracker {
    pub fn len(&self) -> usize {
        self.damaged_blocks.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockDamage {
    pub hp_remaining: f32,
    pub untouched_seconds: f32,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct FloatingBlockDrop {
    pub block: BlockId,
    pub count: u8,
    pub base_center: [f32; 3],
    pub age_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatingBlockDropRender {
    pub block: BlockId,
    pub center: [f32; 3],
    pub age_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamagedBlockRender {
    pub pos: WorldBlockCoord,
    pub block: BlockId,
    pub hp_fraction: f32,
    pub untouched_seconds: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolActionOutcome {
    pub touched_blocks: Vec<ToolBlockDamage>,
    pub broken_blocks: Vec<ToolBlockBreak>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolBlockDamage {
    pub pos: WorldBlockCoord,
    pub block: BlockId,
    pub hp_remaining: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolBlockBreak {
    pub pos: WorldBlockCoord,
    pub block: BlockId,
}

pub(crate) fn apply_primary_tool_action(
    ecs_world: &mut World,
    world: &WorldCore,
) -> ToolActionOutcome {
    let mut outcome = ToolActionOutcome {
        touched_blocks: Vec::new(),
        broken_blocks: Vec::new(),
    };
    let Some(entity) = ecs_world.resource::<LocalPlayerEntity>().0 else {
        return outcome;
    };
    let Some(inventory) = ecs_world.get::<PlayerInventory>(entity).copied() else {
        return outcome;
    };
    if !matches!(inventory.manipulation_mode, ManipulationMode::Interaction) {
        return outcome;
    }
    let Some(tool) = inventory.selected_tool() else {
        return outcome;
    };

    let targets: Vec<_> = ecs_world
        .resource::<super::selection::SelectionState>()
        .interaction_preview_blocks
        .iter()
        .map(|preview| preview.block)
        .collect();
    if targets.is_empty() {
        return outcome;
    }

    trigger_voxel_player_tool_swing(ecs_world);
    let tool_damage = tool_damage(tool);
    let mut tracker = ecs_world.resource_mut::<BlockDamageTracker>();
    for pos in targets {
        let Some(block) = world.get_block(pos).filter(|block| !block.is_air()) else {
            tracker.damaged_blocks.remove(&pos);
            continue;
        };

        let damage = tracker.damaged_blocks.entry(pos).or_insert(BlockDamage {
            hp_remaining: DEFAULT_BLOCK_HP,
            untouched_seconds: 0.0,
        });
        damage.hp_remaining = (damage.hp_remaining - tool_damage).max(0.0);
        damage.untouched_seconds = 0.0;
        outcome.touched_blocks.push(ToolBlockDamage {
            pos,
            block,
            hp_remaining: damage.hp_remaining,
        });

        if damage.hp_remaining <= f32::EPSILON {
            tracker.damaged_blocks.remove(&pos);
            outcome.broken_blocks.push(ToolBlockBreak { pos, block });
        }
    }

    outcome
}

pub(crate) fn tick_tool_interaction_state(ecs_world: &mut World, world: &WorldCore) {
    let dt_seconds = ecs_world.resource::<FrameDeltaSeconds>().0.max(0.0);
    prune_block_damage(ecs_world, world, dt_seconds);
    update_floating_block_drops(ecs_world, dt_seconds);
}

pub(crate) fn spawn_block_drop(ecs_world: &mut World, pos: WorldBlockCoord, block: BlockId) {
    if block.is_air() {
        return;
    }

    let [offset_x, offset_z] = random_drop_offset(pos, block);
    let base_center = [
        pos.0 as f32 + 0.5 + offset_x,
        pos.1 as f32 + 0.65,
        pos.2 as f32 + 0.5 + offset_z,
    ];
    ecs_world.spawn((
        FloatingBlockDrop {
            block,
            count: 1,
            base_center,
            age_seconds: 0.0,
        },
        Transform {
            translation: base_center,
        },
    ));
}

pub(crate) fn damaged_block_renders(
    ecs_world: &mut World,
    world: &WorldCore,
) -> Vec<DamagedBlockRender> {
    let tracker = ecs_world.resource::<BlockDamageTracker>();
    tracker
        .damaged_blocks
        .iter()
        .filter_map(|(pos, damage)| {
            world
                .get_block(*pos)
                .filter(|block| !block.is_air())
                .map(|block| DamagedBlockRender {
                    pos: *pos,
                    block,
                    hp_fraction: (damage.hp_remaining / DEFAULT_BLOCK_HP).clamp(0.0, 1.0),
                    untouched_seconds: damage.untouched_seconds,
                })
        })
        .collect()
}

pub(crate) fn floating_block_drop_renders(ecs_world: &mut World) -> Vec<FloatingBlockDropRender> {
    let mut query = ecs_world.query::<&FloatingBlockDrop>();
    query
        .iter(ecs_world)
        .map(|drop| {
            let bob = (drop.age_seconds * 4.0).sin() * 0.08;
            FloatingBlockDropRender {
                block: drop.block,
                center: [
                    drop.base_center[0],
                    drop.base_center[1] + bob,
                    drop.base_center[2],
                ],
                age_seconds: drop.age_seconds,
            }
        })
        .collect()
}

fn prune_block_damage(ecs_world: &mut World, world: &WorldCore, dt_seconds: f32) {
    let mut tracker = ecs_world.resource_mut::<BlockDamageTracker>();
    tracker.damaged_blocks.retain(|pos, damage| {
        damage.untouched_seconds += dt_seconds;
        let chunk_loaded = world.has_chunk(world_to_chunk_local(*pos).0);
        damage.untouched_seconds < BLOCK_DAMAGE_RECOVERY_SECONDS && chunk_loaded
    });
}

fn update_floating_block_drops(ecs_world: &mut World, dt_seconds: f32) {
    let Some(player_entity) = ecs_world.resource::<LocalPlayerEntity>().0 else {
        return;
    };
    let Some(player_transform) = ecs_world.get::<Transform>(player_entity).copied() else {
        return;
    };
    let player_body = ecs_world
        .get::<PlayerBody>(player_entity)
        .copied()
        .unwrap_or_default();
    let player_min = [
        player_transform.translation[0] - player_body.half_extents[0],
        player_transform.translation[1] - player_body.half_extents[1],
        player_transform.translation[2] - player_body.half_extents[2],
    ];
    let player_max = [
        player_transform.translation[0] + player_body.half_extents[0],
        player_transform.translation[1] + player_body.half_extents[1],
        player_transform.translation[2] + player_body.half_extents[2],
    ];

    let mut pickups = Vec::new();
    {
        let mut query = ecs_world.query::<(Entity, &mut FloatingBlockDrop)>();
        for (entity, mut drop) in query.iter_mut(ecs_world) {
            drop.age_seconds += dt_seconds;
            let distance_to_player = distance_to_aabb(drop.base_center, player_min, player_max);
            if distance_to_player <= BLOCK_DROP_PICKUP_RADIUS {
                pickups.push((entity, drop.block, drop.count));
            } else if distance_to_player <= BLOCK_DROP_ATTRACT_RADIUS {
                let target = closest_point_on_aabb(drop.base_center, player_min, player_max);
                drop.base_center = move_towards(
                    drop.base_center,
                    target,
                    BLOCK_DROP_ATTRACT_SPEED * dt_seconds,
                );
            }
        }
    }

    if pickups.is_empty() {
        return;
    }

    for (drop_entity, block, count) in pickups {
        let remaining = add_block_to_local_inventory(ecs_world, block, count);
        if remaining == 0 {
            let _ = ecs_world.despawn(drop_entity);
        } else if let Some(mut drop) = ecs_world.get_mut::<FloatingBlockDrop>(drop_entity) {
            drop.count = remaining;
        }
    }
}

pub(crate) fn add_block_to_local_inventory(ecs_world: &mut World, block: BlockId, count: u8) -> u8 {
    let Some(entity) = ecs_world.resource::<LocalPlayerEntity>().0 else {
        return count;
    };
    let Some(mut inventory) = ecs_world.get_mut::<PlayerInventory>(entity) else {
        return count;
    };

    add_block_to_inventory(&mut inventory, block, count)
}

fn add_block_to_inventory(inventory: &mut PlayerInventory, block: BlockId, count: u8) -> u8 {
    let remaining = add_to_slot_group(&mut inventory.block_quickslots, block, count);
    if remaining == 0 {
        return 0;
    }
    add_to_slot_group(&mut inventory.general_slots, block, remaining)
}

fn add_to_slot_group<const N: usize>(
    slots: &mut [Option<InventorySlot>; N],
    block: BlockId,
    mut remaining: u8,
) -> u8 {
    for slot in slots.iter_mut().flatten() {
        if slot.item == InventoryItem::Block(block) && slot.count < u8::MAX {
            let capacity = u8::MAX - slot.count;
            let inserted = capacity.min(remaining);
            slot.count += inserted;
            remaining -= inserted;
            if remaining == 0 {
                return 0;
            }
        }
    }

    for slot in slots.iter_mut() {
        if slot.is_none() {
            *slot = Some(InventorySlot::block(block, remaining));
            return 0;
        }
    }

    remaining
}

fn tool_damage(tool: ToolKind) -> f32 {
    match tool {
        ToolKind::Shovel => SHOVEL_TOOL_DAMAGE,
        ToolKind::Pickaxe => PICKAXE_TOOL_DAMAGE,
    }
}

fn random_drop_offset(pos: WorldBlockCoord, block: BlockId) -> [f32; 2] {
    let hash = hash_block_drop(pos, block);
    let unit_x = ((hash & 0xffff) as f32 / 65535.0) * 2.0 - 1.0;
    let unit_z = (((hash >> 16) & 0xffff) as f32 / 65535.0) * 2.0 - 1.0;
    let length = (unit_x * unit_x + unit_z * unit_z).sqrt();
    if length <= f32::EPSILON {
        return [0.0, 0.0];
    }
    let radius = BLOCK_DROP_RANDOM_OFFSET_RADIUS * (((hash >> 32) & 0xffff) as f32 / 65535.0);
    [unit_x / length * radius, unit_z / length * radius]
}

fn hash_block_drop(pos: WorldBlockCoord, block: BlockId) -> u64 {
    let mut value = 0x9e37_79b9_7f4a_7c15_u64;
    value ^= (pos.0 as i64 as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = value.rotate_left(21);
    value ^= (pos.1 as i64 as u64).wrapping_mul(0x94d0_49bb_1331_11eb);
    value = value.rotate_left(17);
    value ^= (pos.2 as i64 as u64).wrapping_mul(0x2545_f491_4f6c_dd1d);
    value ^= u64::from(block.raw()).wrapping_mul(0x9ddf_ea08_eb38_2d69);
    value ^ (value >> 33)
}

fn distance_to_aabb(point: [f32; 3], min: [f32; 3], max: [f32; 3]) -> f32 {
    let dx = distance_to_interval(point[0], min[0], max[0]);
    let dy = distance_to_interval(point[1], min[1], max[1]);
    let dz = distance_to_interval(point[2], min[2], max[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn distance_to_interval(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min - value
    } else if value > max {
        value - max
    } else {
        0.0
    }
}

fn closest_point_on_aabb(point: [f32; 3], min: [f32; 3], max: [f32; 3]) -> [f32; 3] {
    [
        point[0].clamp(min[0], max[0]),
        point[1].clamp(min[1], max[1]),
        point[2].clamp(min[2], max[2]),
    ]
}

fn move_towards(current: [f32; 3], target: [f32; 3], max_delta: f32) -> [f32; 3] {
    let delta = [
        target[0] - current[0],
        target[1] - current[1],
        target[2] - current[2],
    ];
    let distance = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    if distance <= f32::EPSILON || distance <= max_delta {
        return target;
    }
    let scale = max_delta / distance;
    [
        current[0] + delta[0] * scale,
        current[1] + delta[1] * scale,
        current[2] + delta[2] * scale,
    ]
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::ecs::{Player, SelectionState, VoxelPlayerToolSwingState};
    use crate::world::{BlockRegistry, ChunkCoord, ChunkData, LocalBlockCoord, WorldMeta};

    fn test_world_with_block(block: BlockId) -> WorldCore {
        let registry =
            Arc::new(BlockRegistry::load_default().expect("default registry should load"));
        let mut world = WorldCore::new(WorldMeta::default(), registry);
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));
        chunk
            .set_block(LocalBlockCoord::new(1, 1, 1).unwrap(), block)
            .unwrap();
        world.insert_chunk(ChunkCoord(0, 0, 0), chunk);
        world
    }

    fn test_ecs_world() -> World {
        let mut ecs_world = World::new();
        ecs_world.insert_resource(LocalPlayerEntity::default());
        ecs_world.insert_resource(BlockDamageTracker::default());
        ecs_world.insert_resource(FrameDeltaSeconds(0.016));
        ecs_world.insert_resource(VoxelPlayerToolSwingState::default());
        ecs_world.insert_resource(SelectionState {
            hovered_block: Some(WorldBlockCoord(1, 1, 1)),
            hovered_face: None,
            hit_point: None,
            interaction_preview_blocks: vec![super::super::selection::SelectionPreviewBlock {
                block: WorldBlockCoord(1, 1, 1),
            }],
            build_preview_block: None,
        });
        let entity = ecs_world
            .spawn((
                Player,
                PlayerInventory::default(),
                PlayerBody::default(),
                Transform {
                    translation: [1.5, 3.0, 1.5],
                },
            ))
            .id();
        ecs_world.resource_mut::<LocalPlayerEntity>().0 = Some(entity);
        ecs_world
    }

    #[test]
    fn tool_damage_breaks_after_three_uses() {
        let world = test_world_with_block(BlockId::STONE);
        let mut ecs_world = test_ecs_world();

        let first = apply_primary_tool_action(&mut ecs_world, &world);
        let second = apply_primary_tool_action(&mut ecs_world, &world);
        let third = apply_primary_tool_action(&mut ecs_world, &world);

        assert_eq!(first.broken_blocks.len(), 0);
        assert_eq!(second.broken_blocks.len(), 0);
        assert_eq!(third.broken_blocks.len(), 1);
        assert_eq!(third.broken_blocks[0].pos, WorldBlockCoord(1, 1, 1));
    }

    #[test]
    fn pickaxe_breaks_default_block_in_one_use() {
        let world = test_world_with_block(BlockId::STONE);
        let mut ecs_world = test_ecs_world();
        let player_entity = ecs_world.resource::<LocalPlayerEntity>().0.unwrap();
        ecs_world
            .get_mut::<PlayerInventory>(player_entity)
            .unwrap()
            .selected_tool_slot = 1;

        let outcome = apply_primary_tool_action(&mut ecs_world, &world);

        assert_eq!(outcome.broken_blocks.len(), 1);
        assert_eq!(outcome.broken_blocks[0].pos, WorldBlockCoord(1, 1, 1));
    }

    #[test]
    fn block_damage_recovers_after_timeout() {
        let world = test_world_with_block(BlockId::STONE);
        let mut ecs_world = test_ecs_world();
        apply_primary_tool_action(&mut ecs_world, &world);
        ecs_world.resource_mut::<FrameDeltaSeconds>().0 = BLOCK_DAMAGE_RECOVERY_SECONDS + 0.1;

        tick_tool_interaction_state(&mut ecs_world, &world);

        assert_eq!(ecs_world.resource::<BlockDamageTracker>().len(), 0);
    }

    #[test]
    fn pickup_prioritizes_block_quickslots() {
        let mut ecs_world = test_ecs_world();
        let player_entity = ecs_world.resource::<LocalPlayerEntity>().0.unwrap();
        {
            let mut inventory = ecs_world.get_mut::<PlayerInventory>(player_entity).unwrap();
            inventory.block_quickslots = [None; super::super::inventory::QUICKSLOT_COUNT];
            inventory.general_slots = [None; super::super::inventory::GENERAL_SLOT_COUNT];
        }

        let remaining = add_block_to_local_inventory(&mut ecs_world, BlockId::DIRT, 1);

        assert_eq!(remaining, 0);
        assert_eq!(
            ecs_world
                .get::<PlayerInventory>(player_entity)
                .unwrap()
                .block_quickslots[0],
            Some(InventorySlot::block(BlockId::DIRT, 1))
        );
    }

    #[test]
    fn drop_pickup_uses_player_body_distance() {
        let world = test_world_with_block(BlockId::STONE);
        let mut ecs_world = test_ecs_world();
        let player_entity = ecs_world.resource::<LocalPlayerEntity>().0.unwrap();
        {
            let mut inventory = ecs_world.get_mut::<PlayerInventory>(player_entity).unwrap();
            inventory.block_quickslots = [None; super::super::inventory::QUICKSLOT_COUNT];
            inventory.general_slots = [None; super::super::inventory::GENERAL_SLOT_COUNT];
        }
        ecs_world.spawn((
            FloatingBlockDrop {
                block: BlockId::DIRT,
                count: 1,
                base_center: [2.85, 3.0, 1.5],
                age_seconds: 0.0,
            },
            Transform {
                translation: [2.85, 3.0, 1.5],
            },
        ));

        tick_tool_interaction_state(&mut ecs_world, &world);

        assert_eq!(
            ecs_world
                .get::<PlayerInventory>(player_entity)
                .unwrap()
                .block_quickslots[0],
            Some(InventorySlot::block(BlockId::DIRT, 1))
        );
    }

    #[test]
    fn nearby_drop_moves_towards_player_before_pickup() {
        let world = test_world_with_block(BlockId::STONE);
        let mut ecs_world = test_ecs_world();
        ecs_world.resource_mut::<FrameDeltaSeconds>().0 = 0.1;
        let drop_entity = ecs_world
            .spawn((
                FloatingBlockDrop {
                    block: BlockId::DIRT,
                    count: 1,
                    base_center: [4.2, 3.0, 1.5],
                    age_seconds: 0.0,
                },
                Transform {
                    translation: [4.2, 3.0, 1.5],
                },
            ))
            .id();

        tick_tool_interaction_state(&mut ecs_world, &world);

        assert!(
            ecs_world
                .get::<FloatingBlockDrop>(drop_entity)
                .unwrap()
                .base_center[0]
                < 4.2
        );
    }
}
