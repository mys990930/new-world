use super::GameApp;
use crate::renderer::{RenderCameraState, RenderFrameInput};

impl GameApp {
    pub fn update(&mut self) {
        self.bridge_platform_to_ecs();
        self.ecs.run_pre_update();
        self.ecs.run_update();
        self.ecs.run_post_update();

        let commands = self.ecs.drain_player_commands();
        if !commands.is_empty() {
            println!("[app] ecs commands: {:?}", commands);
        }
    }

    pub fn render(&mut self) {
        let (width, height) = {
            let window = self.platform.window_state();
            (window.width, window.height)
        };

        if self.renderer.surface_state().width() != width
            || self.renderer.surface_state().height() != height
        {
            if let Err(error) = self.renderer.resize(width, height) {
                eprintln!("[app] renderer resize failed: {:?}", error);
                return;
            }
        }

        let camera = RenderCameraState::default();
        let visible_chunks = [];

        if let Err(error) = self.renderer.render(RenderFrameInput {
            camera: &camera,
            visible_chunks: &visible_chunks,
            clear_color_override: None,
        }) {
            eprintln!("[app] renderer frame failed: {:?}", error);
        }
    }
}
