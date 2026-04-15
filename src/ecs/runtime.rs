use bevy_ecs::prelude::{IntoScheduleConfigs, Resource, Schedule, World};

use super::camera::{
    CameraState, apply_camera_commands_system, apply_camera_zoom_input_system,
    clear_camera_impulses_system, update_camera_follow_system,
};
use super::chunk::ChunkStates;
use super::command::{
    clear_player_command_buffer_system, MoveWorldIntent, PlayerCommand, PlayerCommandBuffer,
};
use super::inventory::{
    PlayerInventory, ToolCatalog, apply_inventory_commands_system, local_player_inventory,
};
use super::input::{interpret_input_system, EcsInputSnapshot};
use super::player::{
    place_local_player_on_surface, simulate_local_player_motion, spawn_default_player,
    sync_local_player_velocity_system, update_move_world_intent_system, FrameDeltaSeconds,
    LocalPlayerEntity, PlayerBody, PlayerMovementConfig, PlayerPhysicsState, Transform,
};
use super::selection::{SelectionState, update_selection_from_world};
use crate::world::WorldCore;

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
        world.insert_resource(FrameDeltaSeconds::default());
        world.insert_resource(PlayerMovementConfig::default());
        world.insert_resource(ToolCatalog::default());
        world.insert_resource(ChunkStates::default());
        world.insert_resource(SelectionState::default());

        let mut pre_update = Schedule::default();
        pre_update.add_systems((clear_player_command_buffer_system, clear_camera_impulses_system));

        let mut update = Schedule::default();
        update.add_systems(
            (
                interpret_input_system,
                apply_inventory_commands_system,
                apply_camera_commands_system,
                apply_camera_zoom_input_system,
                update_move_world_intent_system,
                sync_local_player_velocity_system,
            )
                .chain(),
        );
        let mut post_update = Schedule::default();
        post_update.add_systems(update_camera_follow_system);

        Self {
            world,
            pre_update,
            update,
            post_update,
            fixed_update: Schedule::default(),
        }
    }

    pub fn insert_resource<T: Resource>(&mut self, value: T) {
        self.world.insert_resource(value);
    }

    pub fn set_frame_delta_seconds(&mut self, dt_seconds: f32) {
        self.world.resource_mut::<FrameDeltaSeconds>().0 = dt_seconds.max(0.0);
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

    pub fn local_player_body(&self) -> Option<PlayerBody> {
        let entity = self.world.resource::<LocalPlayerEntity>().0?;
        self.world.get::<PlayerBody>(entity).copied()
    }

    pub fn local_player_physics_state(&self) -> Option<PlayerPhysicsState> {
        let entity = self.world.resource::<LocalPlayerEntity>().0?;
        self.world.get::<PlayerPhysicsState>(entity).copied()
    }

    pub fn local_player_inventory(&self) -> Option<PlayerInventory> {
        local_player_inventory(&self.world)
    }

    pub fn simulate_local_player_motion(&mut self, world: &WorldCore) {
        simulate_local_player_motion(&mut self.world, world);
    }

    pub fn place_local_player_on_surface(
        &mut self,
        world: &WorldCore,
        anchor_xz: [f32; 2],
    ) -> bool {
        place_local_player_on_surface(&mut self.world, world, anchor_xz)
    }

    pub fn update_selection_from_world(
        &mut self,
        world: &WorldCore,
        viewport_width: u32,
        viewport_height: u32,
    ) {
        let input = self.world.resource::<EcsInputSnapshot>().clone();
        let camera = *self.world.resource::<CameraState>();
        let tool_catalog = *self.world.resource::<ToolCatalog>();
        let player_transform = self.local_player_transform();
        let player_inventory = self.local_player_inventory();
        let mut selection = self.world.resource_mut::<SelectionState>();
        update_selection_from_world(
            &mut selection,
            world,
            &input,
            camera,
            viewport_width,
            viewport_height,
            player_transform,
            player_inventory,
            tool_catalog,
        );
    }

    pub fn selection_state(&self) -> SelectionState {
        self.world.resource::<SelectionState>().clone()
    }
}
