use super::GameApp;

impl GameApp {
    pub fn update(&mut self) {
        // Current vertical slice only verifies that app can observe platform snapshots.
        let _window = self.platform.window_state();
        let _input = self.platform.raw_input_state();
        let _lifecycle = self.platform.lifecycle_state();

        self.frame_index = self.frame_index.saturating_add(1);
    }
}
