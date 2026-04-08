use bevy_ecs::prelude::{Component, Entity, Query, Res, ResMut, Resource, With, World};

use super::camera::CameraState;
use super::command::MoveWorldIntent;
use super::input::EcsInputSnapshot;

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

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct LocalPlayerEntity(pub Option<Entity>);

pub(crate) fn update_move_world_intent_system(
    input: Res<EcsInputSnapshot>,
    camera: Res<CameraState>,
    mut move_world_intent: ResMut<MoveWorldIntent>,
) {
    if !input.active || !input.focused {
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

    velocity.linear = [
        move_world_intent.east as f32,
        0.0,
        move_world_intent.north as f32,
    ];
}

pub(crate) fn spawn_default_player(world: &mut World) -> Entity {
    if let Some(entity) = world.resource::<LocalPlayerEntity>().0 {
        return entity;
    }

    let entity = world
        .spawn((
            Player,
            Transform {
                translation: [3.0, 1.5, 3.0],
            },
            Velocity::default(),
        ))
        .id();
    world.resource_mut::<LocalPlayerEntity>().0 = Some(entity);
    entity
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
