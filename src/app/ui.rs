use winit::keyboard::KeyCode;

use super::GameApp;

const WORLD_SELECT_SLOT_COUNT: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    InGame,
    WorldSelect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUiState {
    pub mode: AppMode,
    pub show_minimap_overlay: bool,
    pub selected_world_slot: usize,
}

impl Default for AppUiState {
    fn default() -> Self {
        Self {
            mode: AppMode::InGame,
            show_minimap_overlay: true,
            selected_world_slot: 0,
        }
    }
}

impl GameApp {
    pub fn handle_ui_shortcuts(&mut self) {
        let input = self.platform.raw_input_state();

        if input.just_pressed_keys.contains(&KeyCode::F1) {
            self.ui.mode = match self.ui.mode {
                AppMode::InGame => AppMode::WorldSelect,
                AppMode::WorldSelect => AppMode::InGame,
            };
        }

        if input.just_pressed_keys.contains(&KeyCode::Tab) {
            self.ui.show_minimap_overlay = !self.ui.show_minimap_overlay;
        }

        if self.ui.mode != AppMode::WorldSelect {
            return;
        }

        if input.just_pressed_keys.contains(&KeyCode::Escape)
            || input.just_pressed_keys.contains(&KeyCode::Enter)
        {
            self.ui.mode = AppMode::InGame;
            return;
        }

        if input.just_pressed_keys.contains(&KeyCode::ArrowLeft)
            || input.just_pressed_keys.contains(&KeyCode::KeyA)
        {
            self.ui.selected_world_slot = self
                .ui
                .selected_world_slot
                .checked_sub(1)
                .unwrap_or(WORLD_SELECT_SLOT_COUNT - 1);
        }

        if input.just_pressed_keys.contains(&KeyCode::ArrowRight)
            || input.just_pressed_keys.contains(&KeyCode::KeyD)
        {
            self.ui.selected_world_slot =
                (self.ui.selected_world_slot + 1) % WORLD_SELECT_SLOT_COUNT;
        }
    }

    pub fn gameplay_active(&self) -> bool {
        self.ui.mode == AppMode::InGame
    }
}
