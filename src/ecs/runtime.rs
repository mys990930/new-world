use bevy_ecs::prelude::{IntoScheduleConfigs, Resource, Schedule, World};

use super::camera::{apply_camera_commands_system, clear_camera_impulses_system, CameraState};
use super::command::{
    clear_player_command_buffer_system, MoveWorldIntent, PlayerCommand, PlayerCommandBuffer,
};
use super::input::{interpret_input_system, EcsInputSnapshot};
use super::player::{
    spawn_default_player, sync_local_player_velocity_system, update_move_world_intent_system,
    LocalPlayerEntity, Transform,
};

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
        world.insert_resource(MoveWorldIntent::default());
        world.insert_resource(CameraState::default());
        world.insert_resource(LocalPlayerEntity::default());

        let mut pre_update = Schedule::default();
        pre_update.add_systems((clear_player_command_buffer_system, clear_camera_impulses_system));

        let mut update = Schedule::default();
        update.add_systems(
            (
                interpret_input_system,
                apply_camera_commands_system,
                update_move_world_intent_system,
                sync_local_player_velocity_system,
            )
                .chain(),
        );

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

    pub fn spawn_default_player(&mut self) {
        let entity = spawn_default_player(&mut self.world);
        println!("[ecs] spawned local player entity: {:?}", entity);
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

    pub fn move_world_intent(&self) -> MoveWorldIntent {
        *self.world.resource::<MoveWorldIntent>()
    }

    pub fn camera_state(&self) -> CameraState {
        *self.world.resource::<CameraState>()
    }

    pub fn local_player_transform(&self) -> Option<Transform> {
        let entity = self.world.resource::<LocalPlayerEntity>().0?;
        self.world.get::<Transform>(entity).copied()
    }
}
