use bevy_ecs::prelude::{Component, Entity, Resource, World};

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

pub(crate) fn spawn_default_player(world: &mut World) -> Entity {
    if let Some(entity) = world.resource::<LocalPlayerEntity>().0 {
        return entity;
    }

    let entity = world.spawn((Player, Transform::default(), Velocity::default())).id();
    world.resource_mut::<LocalPlayerEntity>().0 = Some(entity);
    entity
}
