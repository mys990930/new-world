use super::GameApp;

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

        self.frame_index = self.frame_index.saturating_add(1);
    }
}
