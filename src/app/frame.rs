use super::GameApp;
use crate::renderer::RenderFrameInput;

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

        let render_frame = self.bridge_ecs_to_render_frame();

        if let Err(error) = self.renderer.render(RenderFrameInput {
            camera: &render_frame.camera,
            visible_chunks: &render_frame.visible_chunks,
            clear_color_override: None,
        }) {
            eprintln!("[app] renderer frame failed: {:?}", error);
        }
    }
}
