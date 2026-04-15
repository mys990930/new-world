use bevy_ecs::prelude::{Component, Entity, Query, Res, ResMut, Resource, With, World};

use super::camera::CameraState;
use super::command::MoveWorldIntent;
use super::inventory::PlayerInventory;
use super::input::EcsInputSnapshot;
use crate::world::{CHUNK_EDGE_I32, WorldBlockCoord, WorldCore};

const COLLISION_EPSILON: f32 = 0.001;
const GROUND_PROBE_DEPTH: f32 = 0.05;
const MOTION_SWEEP_STEP: f32 = 0.25;
const CONTACT_BINARY_STEPS: usize = 8;

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Player;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub translation: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Velocity {
    pub linear: [f32; 3],
}

impl Default for Velocity {
    fn default() -> Self {
        Self {
            linear: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct PlayerBody {
    pub half_extents: [f32; 3],
}

impl Default for PlayerBody {
    fn default() -> Self {
        Self {
            half_extents: [1.0, 2.0, 1.0],
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerPhysicsState {
    pub grounded: bool,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct LocalPlayerEntity(pub Option<Entity>);

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct FrameDeltaSeconds(pub f32);

impl Default for FrameDeltaSeconds {
    fn default() -> Self {
        Self(0.0)
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct PlayerMovementConfig {
    pub horizontal_units_per_second: f32,
    pub gravity_units_per_second_sq: f32,
    pub terminal_fall_speed: f32,
    pub max_step_height: f32,
}

impl Default for PlayerMovementConfig {
    fn default() -> Self {
        Self {
            horizontal_units_per_second: 8.0,
            gravity_units_per_second_sq: 28.0,
            terminal_fall_speed: 32.0,
            max_step_height: 1.0,
        }
    }
}

pub(crate) fn update_move_world_intent_system(
    input: Res<EcsInputSnapshot>,
    camera: Res<CameraState>,
    local_player: Option<Res<LocalPlayerEntity>>,
    inventories: Query<&PlayerInventory, With<Player>>,
    mut move_world_intent: ResMut<MoveWorldIntent>,
) {
    if !input.active || !input.focused {
        *move_world_intent = MoveWorldIntent::default();
        return;
    }

    let inventory_open = local_player
        .as_deref()
        .and_then(|local_player| local_player.0)
        .and_then(|entity| inventories.get(entity).ok())
        .is_some_and(|inventory| inventory.inventory_open);
    if inventory_open {
        *move_world_intent = MoveWorldIntent::default();
        return;
    }

    let base_east = axis(input.move_screen_x + input.move_screen_y);
    let base_north = axis(input.move_screen_x - input.move_screen_y);
    let (east, north) = rotate_quarter_view_axes(base_east, base_north, camera.quarter_turns);

    *move_world_intent = MoveWorldIntent { east, north };
}

pub(crate) fn sync_local_player_velocity_system(
    local_player: Res<LocalPlayerEntity>,
    move_world_intent: Res<MoveWorldIntent>,
    mut velocities: Query<&mut Velocity, With<Player>>,
) {
    let Some(entity) = local_player.0 else {
        return;
    };

    let Ok(mut velocity) = velocities.get_mut(entity) else {
        return;
    };

    velocity.linear[0] = move_world_intent.east as f32;
    velocity.linear[2] = move_world_intent.north as f32;
}

pub(crate) fn spawn_default_player(world: &mut World) -> Entity {
    if let Some(entity) = world.resource::<LocalPlayerEntity>().0 {
        return entity;
    }

    let entity = world
        .spawn((
            Player,
            PlayerInventory::default(),
            PlayerBody::default(),
            PlayerPhysicsState::default(),
            Transform {
                translation: [16.0, 8.0, 16.0],
            },
            Velocity::default(),
        ))
        .id();
    world.resource_mut::<LocalPlayerEntity>().0 = Some(entity);
    entity
}

pub(crate) fn simulate_local_player_motion(ecs_world: &mut World, world: &WorldCore) {
    let Some(entity) = ecs_world.resource::<LocalPlayerEntity>().0 else {
        return;
    };

    let frame_delta = ecs_world.resource::<FrameDeltaSeconds>().0.max(0.0);
    if frame_delta <= f32::EPSILON {
        return;
    }

    let movement = *ecs_world.resource::<PlayerMovementConfig>();
    let mut query = ecs_world
        .query_filtered::<
            (
                &mut Transform,
                &mut Velocity,
                &PlayerBody,
                &mut PlayerPhysicsState,
            ),
            With<Player>,
        >();
    let Ok((mut transform, mut velocity, body, mut physics)) = query.get_mut(ecs_world, entity)
    else {
        return;
    };

    let mut position = transform.translation;
    let body = *body;

    if is_grounded(world, position, body) && velocity.linear[1] < 0.0 {
        velocity.linear[1] = 0.0;
    }

    let horizontal_delta = horizontal_motion_delta(
        velocity.linear,
        movement.horizontal_units_per_second,
        frame_delta,
    );
    if horizontal_delta != [0.0, 0.0] {
        position = move_horizontally_with_step_up(
            world,
            position,
            horizontal_delta,
            body,
            movement.max_step_height,
        );
    }

    let grounded_before_vertical = is_grounded(world, position, body);
    if grounded_before_vertical {
        if velocity.linear[1] < 0.0 {
            velocity.linear[1] = 0.0;
        }
    } else {
        velocity.linear[1] = (velocity.linear[1] - movement.gravity_units_per_second_sq * frame_delta)
            .max(-movement.terminal_fall_speed);
    }

    let vertical_delta = velocity.linear[1] * frame_delta;
    let (resolved_position, grounded, hit_ceiling) =
        move_vertically(world, position, vertical_delta, body);
    position = resolved_position;
    if grounded && velocity.linear[1] < 0.0 {
        velocity.linear[1] = 0.0;
    }
    if hit_ceiling && velocity.linear[1] > 0.0 {
        velocity.linear[1] = 0.0;
    }

    physics.grounded = grounded;
    transform.translation = position;
}

pub(crate) fn place_local_player_on_surface(
    ecs_world: &mut World,
    world: &WorldCore,
    anchor_xz: [f32; 2],
) -> bool {
    let Some((min_chunk, max_chunk)) = world.loaded_chunk_bounds() else {
        return false;
    };
    let Some(entity) = ecs_world.resource::<LocalPlayerEntity>().0 else {
        return false;
    };

    let mut query = ecs_world
        .query_filtered::<
            (
                &mut Transform,
                &mut Velocity,
                &PlayerBody,
                &mut PlayerPhysicsState,
            ),
            With<Player>,
        >();
    let Ok((mut transform, mut velocity, body, mut physics)) = query.get_mut(ecs_world, entity)
    else {
        return false;
    };

    let body = *body;
    let highest_center_y =
        ((max_chunk.1 + 1) * CHUNK_EDGE_I32) as f32 + body.half_extents[1] + 2.0;
    let lowest_center_y = (min_chunk.1 * CHUNK_EDGE_I32) as f32 + body.half_extents[1];

    let mut center_y = highest_center_y.floor();
    while center_y >= lowest_center_y {
        let candidate = [anchor_xz[0], center_y, anchor_xz[1]];
        if !body_collides(world, candidate, body) && is_grounded(world, candidate, body) {
            transform.translation = candidate;
            velocity.linear = [0.0, 0.0, 0.0];
            physics.grounded = true;
            return true;
        }
        center_y -= 1.0;
    }

    false
}

fn horizontal_motion_delta(velocity: [f32; 3], speed: f32, frame_delta: f32) -> [f32; 2] {
    let horizontal = [velocity[0], velocity[2]];
    let length_sq = horizontal[0] * horizontal[0] + horizontal[1] * horizontal[1];
    if length_sq <= f32::EPSILON {
        return [0.0, 0.0];
    }

    let inv_length = length_sq.sqrt().recip();
    let distance = speed * frame_delta;
    [
        horizontal[0] * inv_length * distance,
        horizontal[1] * inv_length * distance,
    ]
}

fn move_horizontally_with_step_up(
    world: &WorldCore,
    start: [f32; 3],
    horizontal_delta: [f32; 2],
    body: PlayerBody,
    max_step_height: f32,
) -> [f32; 3] {
    let distance =
        (horizontal_delta[0] * horizontal_delta[0] + horizontal_delta[1] * horizontal_delta[1])
            .sqrt();
    let steps = sweep_steps(distance);
    let step = [
        horizontal_delta[0] / steps as f32,
        horizontal_delta[1] / steps as f32,
    ];
    let mut position = start;

    for _ in 0..steps {
        let flat = [position[0] + step[0], position[1], position[2] + step[1]];
        if !body_collides(world, flat, body) {
            position = flat;
            continue;
        }

        let Some(stepped) = try_step_up(world, position, step, body, max_step_height) else {
            break;
        };
        position = stepped;
    }

    position
}

fn try_step_up(
    world: &WorldCore,
    start: [f32; 3],
    horizontal_step: [f32; 2],
    body: PlayerBody,
    max_step_height: f32,
) -> Option<[f32; 3]> {
    let step_count = sweep_steps(max_step_height);
    for step_index in 1..=step_count {
        let lift = (step_index as f32 / step_count as f32) * max_step_height;
        let lifted = [start[0], start[1] + lift, start[2]];
        if body_collides(world, lifted, body) {
            continue;
        }

        let moved = [
            lifted[0] + horizontal_step[0],
            lifted[1],
            lifted[2] + horizontal_step[1],
        ];
        if body_collides(world, moved, body) {
            continue;
        }

        let settled = settle_down(world, moved, body, lift);
        if is_grounded(world, settled, body) {
            return Some(settled);
        }
    }

    None
}

fn settle_down(
    world: &WorldCore,
    start: [f32; 3],
    body: PlayerBody,
    max_drop: f32,
) -> [f32; 3] {
    let steps = sweep_steps(max_drop);
    let step = max_drop / steps as f32;
    let mut position = start;

    for _ in 0..steps {
        let trial = [position[0], position[1] - step, position[2]];
        if body_collides(world, trial, body) {
            return last_free_position(world, position, trial, body);
        }
        position = trial;
    }

    position
}

fn move_vertically(
    world: &WorldCore,
    start: [f32; 3],
    vertical_delta: f32,
    body: PlayerBody,
) -> ([f32; 3], bool, bool) {
    if vertical_delta.abs() <= f32::EPSILON {
        return (start, is_grounded(world, start, body), false);
    }

    let steps = sweep_steps(vertical_delta.abs());
    let step = vertical_delta / steps as f32;
    let mut position = start;

    for _ in 0..steps {
        let trial = [position[0], position[1] + step, position[2]];
        if body_collides(world, trial, body) {
            let resolved = last_free_position(world, position, trial, body);
            if vertical_delta < 0.0 {
                return (resolved, true, false);
            }
            return (resolved, false, true);
        }
        position = trial;
    }

    (position, is_grounded(world, position, body), false)
}

fn last_free_position(
    world: &WorldCore,
    free_position: [f32; 3],
    colliding_position: [f32; 3],
    body: PlayerBody,
) -> [f32; 3] {
    let mut safe = free_position;
    let mut blocked = colliding_position;

    for _ in 0..CONTACT_BINARY_STEPS {
        let midpoint = midpoint3(safe, blocked);
        if body_collides(world, midpoint, body) {
            blocked = midpoint;
        } else {
            safe = midpoint;
        }
    }

    safe
}

fn is_grounded(world: &WorldCore, center: [f32; 3], body: PlayerBody) -> bool {
    body_collides(
        world,
        [center[0], center[1] - GROUND_PROBE_DEPTH, center[2]],
        body,
    )
}

fn body_collides(world: &WorldCore, center: [f32; 3], body: PlayerBody) -> bool {
    let min = [
        center[0] - body.half_extents[0] + COLLISION_EPSILON,
        center[1] - body.half_extents[1] + COLLISION_EPSILON,
        center[2] - body.half_extents[2] + COLLISION_EPSILON,
    ];
    let max = [
        center[0] + body.half_extents[0] - COLLISION_EPSILON,
        center[1] + body.half_extents[1] - COLLISION_EPSILON,
        center[2] + body.half_extents[2] - COLLISION_EPSILON,
    ];

    for y in min[1].floor() as i32..=max[1].floor() as i32 {
        for z in min[2].floor() as i32..=max[2].floor() as i32 {
            for x in min[0].floor() as i32..=max[0].floor() as i32 {
                match world.get_block(WorldBlockCoord(x, y, z)) {
                    Some(block_id) if world.block_registry().is_solid(block_id) => return true,
                    Some(_) => {}
                    None => return true,
                }
            }
        }
    }

    false
}

fn midpoint3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        (left[0] + right[0]) * 0.5,
        (left[1] + right[1]) * 0.5,
        (left[2] + right[2]) * 0.5,
    ]
}

fn sweep_steps(distance: f32) -> usize {
    ((distance / MOTION_SWEEP_STEP).ceil().max(1.0)) as usize
}

fn axis(value: i8) -> i8 {
    if value > 0 {
        1
    } else if value < 0 {
        -1
    } else {
        0
    }
}

// The quarter-view world basis is skewed relative to the window:
// screen top-right is north and screen bottom-right is east.
fn rotate_quarter_view_axes(east: i8, north: i8, quarter_turns: u8) -> (i8, i8) {
    match quarter_turns % 4 {
        0 => (east, north),
        1 => (north, -east),
        2 => (-east, -north),
        3 => (-north, east),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bevy_ecs::prelude::World;

    use super::*;
    use crate::world::{BlockId, BlockRegistry, ChunkCoord, ChunkData, LocalBlockCoord, WorldMeta};

    fn test_registry() -> Arc<BlockRegistry> {
        Arc::new(BlockRegistry::load_default().expect("default registry should load"))
    }

    fn solid_world() -> WorldCore {
        let registry = test_registry();
        let mut world = WorldCore::new(WorldMeta::new(7), registry);
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));

        for z in 0..32 {
            for x in 0..32 {
                chunk
                    .set_block(LocalBlockCoord::new(x, 0, z).unwrap(), BlockId::STONE)
                    .unwrap();
            }
        }

        world.insert_chunk(ChunkCoord(0, 0, 0), chunk);
        world
    }

    #[test]
    fn place_local_player_on_surface_keeps_body_above_ground() {
        let mut ecs_world = World::new();
        ecs_world.insert_resource(LocalPlayerEntity::default());
        let entity = spawn_default_player(&mut ecs_world);
        let world = solid_world();

        assert!(place_local_player_on_surface(&mut ecs_world, &world, [16.0, 16.0]));

        let transform = ecs_world.get::<Transform>(entity).copied().unwrap();
        assert!((transform.translation[1] - 3.0).abs() < 1e-5);
    }

    #[test]
    fn one_block_step_is_climbable_but_two_block_step_is_not() {
        let registry = test_registry();
        let mut world = WorldCore::new(WorldMeta::new(7), registry);
        let mut chunk = ChunkData::new_empty(ChunkCoord(0, 0, 0));

        for z in 0..32 {
            for x in 0..32 {
                chunk
                    .set_block(LocalBlockCoord::new(x, 0, z).unwrap(), BlockId::STONE)
                    .unwrap();
            }
        }
        for y in 1..=1 {
            for z in 15..=16 {
                for x in 18..=19 {
                    chunk
                        .set_block(LocalBlockCoord::new(x, y, z).unwrap(), BlockId::STONE)
                        .unwrap();
                }
            }
        }
        for y in 1..=2 {
            for z in 20..=21 {
                for x in 23..=24 {
                    chunk
                        .set_block(LocalBlockCoord::new(x, y, z).unwrap(), BlockId::STONE)
                        .unwrap();
                }
            }
        }
        world.insert_chunk(ChunkCoord(0, 0, 0), chunk);

        let body = PlayerBody::default();
        let climbed = move_horizontally_with_step_up(
            &world,
            [16.0, 3.0, 16.0],
            [3.0, 0.0],
            body,
            1.0,
        );
        assert!(climbed[0] > 18.0);
        assert!(climbed[1] > 3.0);

        let blocked = move_horizontally_with_step_up(
            &world,
            [20.0, 3.0, 20.5],
            [4.0, 0.0],
            body,
            1.0,
        );
        assert!(blocked[0] < 23.0);
    }
}
