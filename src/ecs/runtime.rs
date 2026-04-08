use bevy_ecs::prelude::{Resource, Schedule, World};

use super::command::{clear_player_command_buffer_system, PlayerCommand, PlayerCommandBuffer};
use super::input::{interpret_input_system, EcsInputSnapshot};

pub struct EcsRuntime {
    world: World,
    pre_update: Schedule,
    update: Schedule,
    post_update: Schedule,
    fixed_update: Schedule,
}

impl EcsRuntime {
    pub fn new() -> Self {
        let mut world = World::new();
        world.insert_resource(EcsInputSnapshot::default());
        world.insert_resource(PlayerCommandBuffer::default());

        let mut pre_update = Schedule::default();
        pre_update.add_systems(clear_player_command_buffer_system);

        let mut update = Schedule::default();
        update.add_systems(interpret_input_system);

        Self {
            world,
            pre_update,
            update,
            post_update: Schedule::default(),
            fixed_update: Schedule::default(),
        }
    }

    pub fn insert_resource<T: Resource>(&mut self, value: T) {
        self.world.insert_resource(value);
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn run_pre_update(&mut self) {
        self.pre_update.run(&mut self.world);
    }

    pub fn run_update(&mut self) {
        self.update.run(&mut self.world);
    }

    pub fn run_post_update(&mut self) {
        self.post_update.run(&mut self.world);
    }

    pub fn run_fixed_update(&mut self) {
        self.fixed_update.run(&mut self.world);
    }

    pub fn drain_player_commands(&mut self) -> Vec<PlayerCommand> {
        let mut buffer = self.world.resource_mut::<PlayerCommandBuffer>();
        std::mem::take(&mut buffer.0)
    }
}
