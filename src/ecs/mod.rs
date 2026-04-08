use bevy_ecs::prelude::{Res, ResMut, Resource, Schedule, World};

#[derive(Resource, Debug, Clone, Default)]
pub struct EcsInputSnapshot {
    pub move_screen_x: i8,
    pub move_screen_y: i8,
    pub primary_down: bool,
    pub primary_just_pressed: bool,
    pub secondary_down: bool,
    pub secondary_just_pressed: bool,
    pub rotate_camera: i8,
    pub recenter_camera: bool,
    pub cursor_screen_pos: (f64, f64),
    pub cursor_screen_delta: (f64, f64),
    pub focused: bool,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerCommand {
    MoveScreen { x: i8, y: i8 },
    PrimaryAction,
    PlaceBlock,
    RotateCamera { quarter_turns: i8 },
    RecenterCamera,
}

#[derive(Resource, Debug, Default)]
pub struct PlayerCommandBuffer(pub Vec<PlayerCommand>);

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

fn clear_player_command_buffer_system(mut command_buffer: ResMut<PlayerCommandBuffer>) {
    command_buffer.0.clear();
}

fn interpret_input_system(
    input: Res<EcsInputSnapshot>,
    mut command_buffer: ResMut<PlayerCommandBuffer>,
) {
    if !input.active || !input.focused {
        return;
    }

    if input.move_screen_x != 0 || input.move_screen_y != 0 {
        command_buffer.0.push(PlayerCommand::MoveScreen {
            x: input.move_screen_x,
            y: input.move_screen_y,
        });
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
