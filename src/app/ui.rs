use std::fs;
use std::path::{Path, PathBuf};

use winit::keyboard::KeyCode;

use super::GameApp;
use crate::jobs::{JobError, JobRequest};
use crate::world::{CreatedWorldManifest, read_created_world_manifest};

const DEFAULT_CREATE_WORLD_SEED: u64 = 42;
const MIN_CREATE_WORLD_RADIUS: i32 = 2;
const MAX_CREATE_WORLD_RADIUS: i32 = 12;
const WORLD_SELECT_MARGIN_X_PX: f32 = 32.0;
const WORLD_SELECT_MARGIN_Y_PX: f32 = 28.0;
const WORLD_SELECT_TITLE_HEIGHT_PX: f32 = 92.0;
const WORLD_SELECT_FOOTER_HEIGHT_PX: f32 = 88.0;
const WORLD_SELECT_VERTICAL_GAP_PX: f32 = 18.0;
const WORLD_SELECT_SECTION_GAP_PX: f32 = 20.0;
const WORLD_SELECT_SECTION_PADDING_PX: f32 = 16.0;
const WORLD_SELECT_ROWS_TOP_PX: f32 = 74.0;
const WORLD_SELECT_ROW_HEIGHT_PX: f32 = 56.0;
const WORLD_SELECT_ROW_GAP_PX: f32 = 10.0;
const WORLD_SELECT_FIELD_GAP_PX: f32 = 10.0;
const WORLD_SELECT_LABEL_WIDTH_RATIO: f32 = 0.34;
const WORLD_SELECT_ARROW_WIDTH_PX: f32 = 48.0;
const WORLD_SELECT_ARROW_BUTTON_GAP_PX: f32 = 4.0;
const WORLD_SELECT_INFO_LINE_GAP_PX: f32 = 26.0;
const WORLD_SELECT_BUTTON_HEIGHT_PX: f32 = 44.0;
const WORLD_SELECT_BUTTON_GAP_PX: f32 = 12.0;
const WORLD_SELECT_CLOSE_BUTTON_WIDTH_PX: f32 = 176.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    InGame,
    WorldSelect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldSelectSection {
    CreateWorld,
    SelectCreatedWorld,
    SpawnChunk,
}

impl WorldSelectSection {
    const ORDER: [Self; 3] = [Self::CreateWorld, Self::SelectCreatedWorld, Self::SpawnChunk];

    fn offset(self, delta: i32) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0) as i32;
        let next = (index + delta).rem_euclid(Self::ORDER.len() as i32) as usize;
        Self::ORDER[next]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CreateWorld => "CREATE WORLD",
            Self::SelectCreatedWorld => "SELECT WORLD",
            Self::SpawnChunk => "SPAWN CHUNK",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct UiRectPx {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl UiRectPx {
    pub fn inset(self, amount: f32) -> Self {
        Self {
            x: self.x + amount,
            y: self.y + amount,
            w: (self.w - amount * 2.0).max(0.0),
            h: (self.h - amount * 2.0).max(0.0),
        }
    }

    pub fn contains(self, point: (f64, f64)) -> bool {
        let (x, y) = (point.0 as f32, point.1 as f32);
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }

    #[cfg(test)]
    pub fn center(self) -> (f64, f64) {
        (
            (self.x + self.w * 0.5) as f64,
            (self.y + self.h * 0.5) as f64,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldSelectAction {
    CloseWorldSelect,
    IncrementCreateSeed,
    DecrementCreateSeed,
    IncrementCreateRadius,
    DecrementCreateRadius,
    SelectNextCreatedWorld,
    SelectPreviousCreatedWorld,
    IncrementSpawnChunkX,
    DecrementSpawnChunkX,
    IncrementSpawnChunkZ,
    DecrementSpawnChunkZ,
    CreateWorld,
    LoadSelectedWorld,
    LoadSelectedWorldAtSpawn,
    ResetSpawnChunk,
}

impl WorldSelectAction {
    pub fn section(self) -> Option<WorldSelectSection> {
        match self {
            Self::CloseWorldSelect => None,
            Self::IncrementCreateSeed
            | Self::DecrementCreateSeed
            | Self::IncrementCreateRadius
            | Self::DecrementCreateRadius
            | Self::CreateWorld => Some(WorldSelectSection::CreateWorld),
            Self::SelectNextCreatedWorld
            | Self::SelectPreviousCreatedWorld
            | Self::LoadSelectedWorld => Some(WorldSelectSection::SelectCreatedWorld),
            Self::IncrementSpawnChunkX
            | Self::DecrementSpawnChunkX
            | Self::IncrementSpawnChunkZ
            | Self::DecrementSpawnChunkZ
            | Self::LoadSelectedWorldAtSpawn
            | Self::ResetSpawnChunk => Some(WorldSelectSection::SpawnChunk),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedWorldOption {
    pub root: PathBuf,
    pub label: String,
    pub manifest: CreatedWorldManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSelectState {
    pub section: WorldSelectSection,
    pub available_created_worlds: Vec<CreatedWorldOption>,
    pub selected_created_world_index: usize,
    pub create_world_seed: u64,
    pub create_world_radius: i32,
    pub spawn_chunk_x: i32,
    pub spawn_chunk_z: i32,
    pub status_line: String,
}

impl Default for WorldSelectState {
    fn default() -> Self {
        Self {
            section: WorldSelectSection::SelectCreatedWorld,
            available_created_worlds: Vec::new(),
            selected_created_world_index: 0,
            create_world_seed: DEFAULT_CREATE_WORLD_SEED,
            create_world_radius: 6,
            spawn_chunk_x: 0,
            spawn_chunk_z: 0,
            status_line: "CLICK BUTTONS TO CREATE OR LOAD WORLDS".to_string(),
        }
    }
}

impl WorldSelectState {
    pub fn selected_created_world(&self) -> Option<&CreatedWorldOption> {
        self.available_created_worlds
            .get(self.selected_created_world_index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUiState {
    pub mode: AppMode,
    pub show_minimap_overlay: bool,
    pub world_select: WorldSelectState,
}

impl Default for AppUiState {
    fn default() -> Self {
        Self {
            mode: AppMode::InGame,
            show_minimap_overlay: true,
            world_select: WorldSelectState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorldSelectFieldLayout {
    pub label: &'static str,
    pub value: String,
    pub label_rect: UiRectPx,
    pub value_rect: UiRectPx,
    pub increase_rect: UiRectPx,
    pub decrease_rect: UiRectPx,
    pub increase_action: WorldSelectAction,
    pub decrease_action: WorldSelectAction,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorldSelectInfoLineLayout {
    pub rect: UiRectPx,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorldSelectButtonLayout {
    pub rect: UiRectPx,
    pub label: &'static str,
    pub action: WorldSelectAction,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorldSelectSectionLayout {
    pub section: WorldSelectSection,
    pub rect: UiRectPx,
    pub fields: Vec<WorldSelectFieldLayout>,
    pub info_lines: Vec<WorldSelectInfoLineLayout>,
    pub buttons: Vec<WorldSelectButtonLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorldSelectLayout {
    pub outer_rect: UiRectPx,
    pub title_rect: UiRectPx,
    pub footer_rect: UiRectPx,
    pub footer_status_rect: UiRectPx,
    pub footer_current_rect: UiRectPx,
    pub footer_current_label: String,
    pub close_button: WorldSelectButtonLayout,
    pub sections: [WorldSelectSectionLayout; 3],
}

impl WorldSelectLayout {
    pub fn action_at(&self, point: (f64, f64)) -> Option<WorldSelectAction> {
        if self.close_button.enabled && self.close_button.rect.contains(point) {
            return Some(self.close_button.action);
        }

        for section in &self.sections {
            for field in &section.fields {
                if !field.enabled {
                    continue;
                }
                if field.increase_rect.contains(point) {
                    return Some(field.increase_action);
                }
                if field.decrease_rect.contains(point) {
                    return Some(field.decrease_action);
                }
            }
            for button in &section.buttons {
                if button.enabled && button.rect.contains(point) {
                    return Some(button.action);
                }
            }
        }

        None
    }

    pub fn section_at(&self, point: (f64, f64)) -> Option<WorldSelectSection> {
        self.sections
            .iter()
            .find(|section| section.rect.contains(point))
            .map(|section| section.section)
    }
}

pub(crate) fn build_world_select_layout(
    state: &WorldSelectState,
    viewport: [f32; 2],
    current_loaded_label: Option<&str>,
) -> WorldSelectLayout {
    let outer_rect = UiRectPx {
        x: WORLD_SELECT_MARGIN_X_PX,
        y: WORLD_SELECT_MARGIN_Y_PX,
        w: (viewport[0] - WORLD_SELECT_MARGIN_X_PX * 2.0).max(640.0),
        h: (viewport[1] - WORLD_SELECT_MARGIN_Y_PX * 2.0).max(420.0),
    };
    let title_rect = UiRectPx {
        x: outer_rect.x,
        y: outer_rect.y,
        w: outer_rect.w,
        h: WORLD_SELECT_TITLE_HEIGHT_PX,
    };
    let footer_rect = UiRectPx {
        x: outer_rect.x,
        y: outer_rect.y + outer_rect.h - WORLD_SELECT_FOOTER_HEIGHT_PX,
        w: outer_rect.w,
        h: WORLD_SELECT_FOOTER_HEIGHT_PX,
    };
    let body_rect = UiRectPx {
        x: outer_rect.x,
        y: title_rect.y + title_rect.h + WORLD_SELECT_VERTICAL_GAP_PX,
        w: outer_rect.w,
        h: footer_rect.y - (title_rect.y + title_rect.h + WORLD_SELECT_VERTICAL_GAP_PX)
            - WORLD_SELECT_VERTICAL_GAP_PX,
    };

    let section_width = ((body_rect.w - WORLD_SELECT_SECTION_GAP_PX * 2.0) / 3.0).floor();
    let create_rect = UiRectPx {
        x: body_rect.x,
        y: body_rect.y,
        w: section_width,
        h: body_rect.h,
    };
    let select_rect = UiRectPx {
        x: create_rect.x + create_rect.w + WORLD_SELECT_SECTION_GAP_PX,
        y: body_rect.y,
        w: section_width,
        h: body_rect.h,
    };
    let spawn_rect = UiRectPx {
        x: select_rect.x + select_rect.w + WORLD_SELECT_SECTION_GAP_PX,
        y: body_rect.y,
        w: section_width,
        h: body_rect.h,
    };

    let world_selected = state.selected_created_world();
    let world_selection_enabled = world_selected.is_some();
    let close_button = WorldSelectButtonLayout {
        rect: UiRectPx {
            x: footer_rect.x + footer_rect.w - WORLD_SELECT_CLOSE_BUTTON_WIDTH_PX - 18.0,
            y: footer_rect.y + footer_rect.h - WORLD_SELECT_BUTTON_HEIGHT_PX - 16.0,
            w: WORLD_SELECT_CLOSE_BUTTON_WIDTH_PX,
            h: WORLD_SELECT_BUTTON_HEIGHT_PX,
        },
        label: "CLOSE",
        action: WorldSelectAction::CloseWorldSelect,
        enabled: true,
    };

    let create_section = WorldSelectSectionLayout {
        section: WorldSelectSection::CreateWorld,
        rect: create_rect,
        fields: vec![
            build_field_row(
                create_rect,
                0,
                "SEED",
                state.create_world_seed.to_string(),
                WorldSelectAction::IncrementCreateSeed,
                WorldSelectAction::DecrementCreateSeed,
                true,
            ),
            build_field_row(
                create_rect,
                1,
                "RADIUS",
                state.create_world_radius.to_string(),
                WorldSelectAction::IncrementCreateRadius,
                WorldSelectAction::DecrementCreateRadius,
                true,
            ),
        ],
        info_lines: vec![
            build_info_line(
                create_rect,
                0,
                format!("CENTER X {}", state.spawn_chunk_x),
            ),
            build_info_line(
                create_rect,
                1,
                format!("CENTER Z {}", state.spawn_chunk_z),
            ),
            build_info_line(create_rect, 2, "USE SPAWN PANEL TO MOVE CENTER".to_string()),
        ],
        buttons: vec![build_bottom_button(
            create_rect,
            0,
            "CREATE WORLD",
            WorldSelectAction::CreateWorld,
            true,
        )],
    };

    let select_info_lines = if let Some(selected) = world_selected {
        let min = selected.manifest.min_chunk_coord();
        let max = selected.manifest.max_chunk_coord();
        vec![
            build_info_line(
                select_rect,
                0,
                format!(
                    "{} OF {}",
                    state.selected_created_world_index.saturating_add(1),
                    state.available_created_worlds.len()
                ),
            ),
            build_info_line(select_rect, 1, format!("SEED {}", selected.manifest.seed)),
            build_info_line(
                select_rect,
                2,
                format!("BOUNDS {} {} {} {}", min.0, min.2, max.0, max.2),
            ),
        ]
    } else {
        vec![
            build_info_line(select_rect, 0, "NO CREATED WORLDS".to_string()),
            build_info_line(select_rect, 1, "PRESS CREATE WORLD".to_string()),
            build_info_line(select_rect, 2, "TO BUILD A RUNTIME".to_string()),
        ]
    };
    let select_section = WorldSelectSectionLayout {
        section: WorldSelectSection::SelectCreatedWorld,
        rect: select_rect,
        fields: vec![build_field_row(
            select_rect,
            0,
            "WORLD",
            world_selected
                .map(|selected| selected.label.clone())
                .unwrap_or_else(|| "NONE".to_string()),
            WorldSelectAction::SelectNextCreatedWorld,
            WorldSelectAction::SelectPreviousCreatedWorld,
            world_selection_enabled,
        )],
        info_lines: select_info_lines,
        buttons: vec![build_bottom_button(
            select_rect,
            0,
            "LOAD SELECTED",
            WorldSelectAction::LoadSelectedWorld,
            world_selection_enabled,
        )],
    };

    let spawn_section = WorldSelectSectionLayout {
        section: WorldSelectSection::SpawnChunk,
        rect: spawn_rect,
        fields: vec![
            build_field_row(
                spawn_rect,
                0,
                "X",
                state.spawn_chunk_x.to_string(),
                WorldSelectAction::IncrementSpawnChunkX,
                WorldSelectAction::DecrementSpawnChunkX,
                world_selection_enabled,
            ),
            build_field_row(
                spawn_rect,
                1,
                "Z",
                state.spawn_chunk_z.to_string(),
                WorldSelectAction::IncrementSpawnChunkZ,
                WorldSelectAction::DecrementSpawnChunkZ,
                world_selection_enabled,
            ),
        ],
        info_lines: vec![
            build_info_line(spawn_rect, 0, "LOADS THE SELECTED WORLD".to_string()),
            build_info_line(spawn_rect, 1, "AT THE CHUNK ABOVE".to_string()),
        ],
        buttons: vec![
            build_bottom_button(
                spawn_rect,
                1,
                "RESET PREVIEW",
                WorldSelectAction::ResetSpawnChunk,
                world_selection_enabled,
            ),
            build_bottom_button(
                spawn_rect,
                0,
                "LOAD AT CHUNK",
                WorldSelectAction::LoadSelectedWorldAtSpawn,
                world_selection_enabled,
            ),
        ],
    };

    WorldSelectLayout {
        outer_rect,
        title_rect,
        footer_rect,
        footer_status_rect: UiRectPx {
            x: footer_rect.x + 18.0,
            y: footer_rect.y + 16.0,
            w: footer_rect.w - close_button.rect.w - 54.0,
            h: 22.0,
        },
        footer_current_rect: UiRectPx {
            x: footer_rect.x + 18.0,
            y: footer_rect.y + 46.0,
            w: footer_rect.w - close_button.rect.w - 54.0,
            h: 18.0,
        },
        footer_current_label: current_loaded_label.unwrap_or("NONE").to_string(),
        close_button,
        sections: [create_section, select_section, spawn_section],
    }
}

fn build_field_row(
    panel: UiRectPx,
    row_index: usize,
    label: &'static str,
    value: String,
    increase_action: WorldSelectAction,
    decrease_action: WorldSelectAction,
    enabled: bool,
) -> WorldSelectFieldLayout {
    let row_rect = UiRectPx {
        x: panel.x + WORLD_SELECT_SECTION_PADDING_PX,
        y: panel.y
            + WORLD_SELECT_ROWS_TOP_PX
            + row_index as f32 * (WORLD_SELECT_ROW_HEIGHT_PX + WORLD_SELECT_ROW_GAP_PX),
        w: panel.w - WORLD_SELECT_SECTION_PADDING_PX * 2.0,
        h: WORLD_SELECT_ROW_HEIGHT_PX,
    };
    let label_width = ((row_rect.w
        - WORLD_SELECT_ARROW_WIDTH_PX
        - WORLD_SELECT_FIELD_GAP_PX * 2.0)
        * WORLD_SELECT_LABEL_WIDTH_RATIO)
        .floor();
    let value_width =
        row_rect.w - label_width - WORLD_SELECT_ARROW_WIDTH_PX - WORLD_SELECT_FIELD_GAP_PX * 2.0;
    let arrow_button_height =
        ((row_rect.h - WORLD_SELECT_ARROW_BUTTON_GAP_PX) * 0.5).floor();

    let label_rect = UiRectPx {
        x: row_rect.x,
        y: row_rect.y,
        w: label_width,
        h: row_rect.h,
    };
    let value_rect = UiRectPx {
        x: label_rect.x + label_rect.w + WORLD_SELECT_FIELD_GAP_PX,
        y: row_rect.y,
        w: value_width,
        h: row_rect.h,
    };
    let arrow_x = value_rect.x + value_rect.w + WORLD_SELECT_FIELD_GAP_PX;
    let increase_rect = UiRectPx {
        x: arrow_x,
        y: row_rect.y,
        w: WORLD_SELECT_ARROW_WIDTH_PX,
        h: arrow_button_height,
    };
    let decrease_rect = UiRectPx {
        x: arrow_x,
        y: row_rect.y + row_rect.h - arrow_button_height,
        w: WORLD_SELECT_ARROW_WIDTH_PX,
        h: arrow_button_height,
    };

    WorldSelectFieldLayout {
        label,
        value,
        label_rect,
        value_rect,
        increase_rect,
        decrease_rect,
        increase_action,
        decrease_action,
        enabled,
    }
}

fn build_info_line(panel: UiRectPx, index: usize, text: String) -> WorldSelectInfoLineLayout {
    WorldSelectInfoLineLayout {
        rect: UiRectPx {
            x: panel.x + WORLD_SELECT_SECTION_PADDING_PX,
            y: panel.y
                + WORLD_SELECT_ROWS_TOP_PX
                + 2.0 * (WORLD_SELECT_ROW_HEIGHT_PX + WORLD_SELECT_ROW_GAP_PX)
                + 14.0
                + index as f32 * WORLD_SELECT_INFO_LINE_GAP_PX,
            w: panel.w - WORLD_SELECT_SECTION_PADDING_PX * 2.0,
            h: 18.0,
        },
        text,
    }
}

fn build_bottom_button(
    panel: UiRectPx,
    index_from_bottom: usize,
    label: &'static str,
    action: WorldSelectAction,
    enabled: bool,
) -> WorldSelectButtonLayout {
    WorldSelectButtonLayout {
        rect: UiRectPx {
            x: panel.x + WORLD_SELECT_SECTION_PADDING_PX,
            y: panel.y
                + panel.h
                - WORLD_SELECT_SECTION_PADDING_PX
                - WORLD_SELECT_BUTTON_HEIGHT_PX
                - index_from_bottom as f32
                    * (WORLD_SELECT_BUTTON_HEIGHT_PX + WORLD_SELECT_BUTTON_GAP_PX),
            w: panel.w - WORLD_SELECT_SECTION_PADDING_PX * 2.0,
            h: WORLD_SELECT_BUTTON_HEIGHT_PX,
        },
        label,
        action,
        enabled,
    }
}

impl GameApp {
    pub fn handle_ui_shortcuts(&mut self) {
        let input = self.platform.raw_input_state().clone();

        if input.just_pressed_keys.contains(&KeyCode::F1) {
            match self.ui.mode {
                AppMode::InGame => self.open_world_select(),
                AppMode::WorldSelect => self.close_world_select(),
            }
            return;
        }

        if self.ui.mode == AppMode::InGame && input.just_pressed_keys.contains(&KeyCode::KeyM) {
            self.ui.show_minimap_overlay = !self.ui.show_minimap_overlay;
        }

        if self.ui.mode != AppMode::WorldSelect {
            return;
        }

        if input.just_pressed_keys.contains(&KeyCode::Escape) {
            self.close_world_select();
            return;
        }

        let viewport = {
            let window = self.platform.window_state();
            [window.width as f32, window.height as f32]
        };
        let current_loaded_label = self
            .created_world
            .as_ref()
            .and_then(|source| source.root().file_name())
            .and_then(|name| name.to_str());
        let layout = build_world_select_layout(&self.ui.world_select, viewport, current_loaded_label);

        if input.left_just_pressed {
            if let Some(action) = layout.action_at(input.mouse_position) {
                self.execute_world_select_action(action);
                return;
            }
            if let Some(section) = layout.section_at(input.mouse_position) {
                self.ui.world_select.section = section;
                return;
            }
        }

        if input.just_pressed_keys.contains(&KeyCode::ArrowUp)
            || input.just_pressed_keys.contains(&KeyCode::PageUp)
        {
            self.ui.world_select.section = self.ui.world_select.section.offset(-1);
        }

        if input.just_pressed_keys.contains(&KeyCode::ArrowDown)
            || input.just_pressed_keys.contains(&KeyCode::PageDown)
        {
            self.ui.world_select.section = self.ui.world_select.section.offset(1);
        }

        if input.just_pressed_keys.contains(&KeyCode::KeyR) {
            self.reset_world_select_spawn_to_selected_created_world();
            self.ui.world_select.status_line = format!(
                "RESET SPAWN {} {}",
                self.ui.world_select.spawn_chunk_x, self.ui.world_select.spawn_chunk_z
            );
        }

        if input.just_pressed_keys.contains(&KeyCode::KeyB) {
            self.execute_world_create_request();
            return;
        }

        match self.ui.world_select.section {
            WorldSelectSection::CreateWorld => {
                if input.just_pressed_keys.contains(&KeyCode::ArrowLeft)
                    || input.just_pressed_keys.contains(&KeyCode::KeyA)
                {
                    self.adjust_world_create_radius(-1);
                }

                if input.just_pressed_keys.contains(&KeyCode::ArrowRight)
                    || input.just_pressed_keys.contains(&KeyCode::KeyD)
                {
                    self.adjust_world_create_radius(1);
                }

                if input.just_pressed_keys.contains(&KeyCode::Enter) {
                    self.execute_world_create_request();
                }
            }
            WorldSelectSection::SelectCreatedWorld => {
                if input.just_pressed_keys.contains(&KeyCode::ArrowLeft)
                    || input.just_pressed_keys.contains(&KeyCode::KeyA)
                {
                    self.select_created_world(-1);
                }

                if input.just_pressed_keys.contains(&KeyCode::ArrowRight)
                    || input.just_pressed_keys.contains(&KeyCode::KeyD)
                {
                    self.select_created_world(1);
                }

                if input.just_pressed_keys.contains(&KeyCode::Enter) {
                    self.execute_world_select_load();
                }
            }
            WorldSelectSection::SpawnChunk => {
                let mut dx = 0;
                let mut dz = 0;

                if input.just_pressed_keys.contains(&KeyCode::ArrowLeft)
                    || input.just_pressed_keys.contains(&KeyCode::KeyA)
                {
                    dx -= 1;
                }

                if input.just_pressed_keys.contains(&KeyCode::ArrowRight)
                    || input.just_pressed_keys.contains(&KeyCode::KeyD)
                {
                    dx += 1;
                }

                if input.just_pressed_keys.contains(&KeyCode::KeyQ)
                    || input.just_pressed_keys.contains(&KeyCode::KeyW)
                {
                    dz -= 1;
                }

                if input.just_pressed_keys.contains(&KeyCode::KeyE)
                    || input.just_pressed_keys.contains(&KeyCode::KeyS)
                {
                    dz += 1;
                }

                if dx != 0 || dz != 0 {
                    self.adjust_world_select_spawn(dx, dz);
                }

                if input.just_pressed_keys.contains(&KeyCode::Enter) {
                    self.execute_world_select_load();
                }
            }
        }
    }

    pub fn gameplay_active(&self) -> bool {
        self.ui.mode == AppMode::InGame
    }

    fn execute_world_select_action(&mut self, action: WorldSelectAction) {
        if let Some(section) = action.section() {
            self.ui.world_select.section = section;
        }

        match action {
            WorldSelectAction::CloseWorldSelect => self.close_world_select(),
            WorldSelectAction::IncrementCreateSeed => self.adjust_world_create_seed(1),
            WorldSelectAction::DecrementCreateSeed => self.adjust_world_create_seed(-1),
            WorldSelectAction::IncrementCreateRadius => self.adjust_world_create_radius(1),
            WorldSelectAction::DecrementCreateRadius => self.adjust_world_create_radius(-1),
            WorldSelectAction::SelectNextCreatedWorld => self.select_created_world(1),
            WorldSelectAction::SelectPreviousCreatedWorld => self.select_created_world(-1),
            WorldSelectAction::IncrementSpawnChunkX => self.adjust_world_select_spawn(1, 0),
            WorldSelectAction::DecrementSpawnChunkX => self.adjust_world_select_spawn(-1, 0),
            WorldSelectAction::IncrementSpawnChunkZ => self.adjust_world_select_spawn(0, 1),
            WorldSelectAction::DecrementSpawnChunkZ => self.adjust_world_select_spawn(0, -1),
            WorldSelectAction::CreateWorld => self.execute_world_create_request(),
            WorldSelectAction::LoadSelectedWorld | WorldSelectAction::LoadSelectedWorldAtSpawn => {
                self.execute_world_select_load()
            }
            WorldSelectAction::ResetSpawnChunk => {
                self.reset_world_select_spawn_to_selected_created_world();
                self.ui.world_select.status_line = format!(
                    "RESET SPAWN {} {}",
                    self.ui.world_select.spawn_chunk_x, self.ui.world_select.spawn_chunk_z
                );
            }
        }
    }

    fn open_world_select(&mut self) {
        self.refresh_created_worlds();
        self.sync_world_select_with_runtime();
        self.ui.mode = AppMode::WorldSelect;
        self.ui.world_select.status_line =
            "CLICK CREATE WORLD OR LOAD SELECTED TO APPLY".to_string();
    }

    fn close_world_select(&mut self) {
        self.ui.mode = AppMode::InGame;
    }

    fn refresh_created_worlds(&mut self) {
        let previously_selected = self
            .ui
            .world_select
            .selected_created_world()
            .map(|created_world| created_world.root.clone());
        self.ui.world_select.available_created_worlds =
            discover_created_world_options(self.created_worlds_dir_path());

        if self.ui.world_select.available_created_worlds.is_empty() {
            self.ui.world_select.selected_created_world_index = 0;
            self.ui.world_select.status_line =
                "NO CREATED WORLDS FOUND. PRESS CREATE WORLD.".to_string();
            return;
        }

        if let Some(previously_selected) = previously_selected {
            if let Some(index) = self
                .ui
                .world_select
                .available_created_worlds
                .iter()
                .position(|created_world| created_world.root == previously_selected)
            {
                self.ui.world_select.selected_created_world_index = index;
            } else {
                self.ui.world_select.selected_created_world_index = self
                    .ui
                    .world_select
                    .selected_created_world_index
                    .min(self.ui.world_select.available_created_worlds.len().saturating_sub(1));
            }
        } else {
            self.ui.world_select.selected_created_world_index = self
                .ui
                .world_select
                .selected_created_world_index
                .min(self.ui.world_select.available_created_worlds.len().saturating_sub(1));
        }
    }

    fn sync_world_select_with_runtime(&mut self) {
        if let Some(current_root) = self.created_world.as_ref().map(|source| source.root().to_path_buf()) {
            if let Some(index) = self
                .ui
                .world_select
                .available_created_worlds
                .iter()
                .position(|created_world| created_world.root == current_root)
            {
                self.ui.world_select.selected_created_world_index = index;
            }
        }

        if let Some(player_chunk) = self.current_player_chunk_coord() {
            self.ui.world_select.spawn_chunk_x = player_chunk[0];
            self.ui.world_select.spawn_chunk_z = player_chunk[1];
            self.clamp_world_select_spawn_to_selected_created_world();
        } else {
            self.reset_world_select_spawn_to_selected_created_world();
        }
    }

    fn adjust_world_create_seed(&mut self, delta: i64) {
        if delta >= 0 {
            self.ui.world_select.create_world_seed = self
                .ui
                .world_select
                .create_world_seed
                .saturating_add(delta as u64);
        } else {
            self.ui.world_select.create_world_seed = self
                .ui
                .world_select
                .create_world_seed
                .saturating_sub(delta.unsigned_abs());
        }
        self.ui.world_select.status_line = format!(
            "CREATE SEED {}  RADIUS {}",
            self.ui.world_select.create_world_seed, self.ui.world_select.create_world_radius
        );
    }

    fn adjust_world_create_radius(&mut self, delta: i32) {
        self.ui.world_select.create_world_radius =
            (self.ui.world_select.create_world_radius + delta)
                .clamp(MIN_CREATE_WORLD_RADIUS, MAX_CREATE_WORLD_RADIUS);
        self.ui.world_select.status_line = format!(
            "CREATE RADIUS {}  CENTER {} {}",
            self.ui.world_select.create_world_radius,
            self.ui.world_select.spawn_chunk_x,
            self.ui.world_select.spawn_chunk_z
        );
    }

    fn select_created_world(&mut self, delta: i32) {
        let count = self.ui.world_select.available_created_worlds.len();
        if count == 0 {
            self.ui.world_select.status_line = "NO CREATED WORLDS AVAILABLE".to_string();
            return;
        }

        let current = self.ui.world_select.selected_created_world_index as i32;
        let next = (current + delta).rem_euclid(count as i32) as usize;
        self.ui.world_select.selected_created_world_index = next;
        self.reset_world_select_spawn_to_selected_created_world();

        if let Some(selected) = self.ui.world_select.selected_created_world() {
            self.ui.world_select.status_line =
                format!("SELECTED {}", selected.label.to_ascii_uppercase());
        }
    }

    fn adjust_world_select_spawn(&mut self, dx: i32, dz: i32) {
        self.ui.world_select.spawn_chunk_x = self.ui.world_select.spawn_chunk_x.saturating_add(dx);
        self.ui.world_select.spawn_chunk_z = self.ui.world_select.spawn_chunk_z.saturating_add(dz);
        self.clamp_world_select_spawn_to_selected_created_world();
        self.ui.world_select.status_line = format!(
            "SPAWN CHUNK {} {}",
            self.ui.world_select.spawn_chunk_x, self.ui.world_select.spawn_chunk_z
        );
    }

    fn reset_world_select_spawn_to_selected_created_world(&mut self) {
        if let Some(default_preview_center) = self
            .ui
            .world_select
            .selected_created_world()
            .map(|selected| selected.manifest.default_preview_center)
        {
            self.ui.world_select.spawn_chunk_x = default_preview_center[0];
            self.ui.world_select.spawn_chunk_z = default_preview_center[1];
        } else {
            self.ui.world_select.spawn_chunk_x = 0;
            self.ui.world_select.spawn_chunk_z = 0;
        }
    }

    fn clamp_world_select_spawn_to_selected_created_world(&mut self) {
        if let Some((min, max)) = self
            .ui
            .world_select
            .selected_created_world()
            .map(|selected| {
                (
                    selected.manifest.min_chunk_coord(),
                    selected.manifest.max_chunk_coord(),
                )
            })
        {
            self.ui.world_select.spawn_chunk_x =
                self.ui.world_select.spawn_chunk_x.clamp(min.0, max.0);
            self.ui.world_select.spawn_chunk_z =
                self.ui.world_select.spawn_chunk_z.clamp(min.2, max.2);
        }
    }

    fn execute_world_create_request(&mut self) {
        let center_x = self.ui.world_select.spawn_chunk_x;
        let center_z = self.ui.world_select.spawn_chunk_z;
        let radius = self.ui.world_select.create_world_radius;
        let seed = self.ui.world_select.create_world_seed;

        match self.request_create_world_from_ui(seed, center_x, center_z, radius) {
            Ok(root) => {
                self.ui.world_select.status_line = format!(
                    "CREATE QUEUED {}",
                    root.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("WORLD")
                        .to_ascii_uppercase()
                );
            }
            Err(message) => {
                self.ui.world_select.status_line = format!("CREATE FAILED {}", message);
            }
        }
    }

    fn execute_world_select_load(&mut self) {
        let Some(selected) = self.ui.world_select.selected_created_world().cloned() else {
            self.ui.world_select.status_line = "NO WORLD SELECTED".to_string();
            return;
        };

        match self.load_created_world_from_ui(
            selected.root.as_path(),
            self.ui.world_select.spawn_chunk_x,
            self.ui.world_select.spawn_chunk_z,
        ) {
            Ok(()) => {
                self.ui.world_select.status_line = format!(
                    "LOADED {}",
                    selected.label.to_ascii_uppercase()
                );
                self.close_world_select();
            }
            Err(message) => {
                self.ui.world_select.status_line = format!("LOAD FAILED {}", message);
            }
        }
    }

    pub(crate) fn handle_world_created_result(
        &mut self,
        root: PathBuf,
        manifest: CreatedWorldManifest,
    ) {
        self.refresh_created_worlds();
        if let Some(index) = self
            .ui
            .world_select
            .available_created_worlds
            .iter()
            .position(|created_world| created_world.root == root)
        {
            self.ui.world_select.selected_created_world_index = index;
        }
        self.ui.world_select.spawn_chunk_x = manifest.default_preview_center[0];
        self.ui.world_select.spawn_chunk_z = manifest.default_preview_center[1];
        self.ui.world_select.status_line = format!(
            "WORLD CREATED {}",
            root.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("WORLD")
                .to_ascii_uppercase()
        );
    }

    pub(crate) fn handle_job_failure(&mut self, request: &JobRequest, error: JobError) {
        if let JobRequest::CreateWorld { root, .. } = request {
            self.ui.world_select.status_line = format!(
                "CREATE FAILED {} {}",
                root.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("WORLD")
                    .to_ascii_uppercase(),
                format_job_error(error)
            );
        }
    }

    fn created_worlds_dir_path(&self) -> &Path {
        self.config
            .created_worlds_dir
            .as_deref()
            .unwrap_or_else(|| Path::new("target/world-create"))
    }

    fn current_player_chunk_coord(&self) -> Option<[i32; 2]> {
        let transform = self.ecs.local_player_transform()?;
        let block = crate::world::WorldBlockCoord(
            transform.translation[0].floor() as i32,
            transform.translation[1].floor() as i32,
            transform.translation[2].floor() as i32,
        );
        let chunk = crate::world::world_to_chunk_local(block).0;
        Some([chunk.0, chunk.2])
    }
}

fn discover_created_world_options(base_dir: &Path) -> Vec<CreatedWorldOption> {
    let Ok(entries) = fs::read_dir(base_dir) else {
        return Vec::new();
    };

    let mut discovered = Vec::new();
    for entry in entries.flatten() {
        let root = entry.path();
        if !root.is_dir() {
            continue;
        }

        let Ok(manifest) = read_created_world_manifest(root.as_path()) else {
            continue;
        };
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok();
        let label = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("CREATED_WORLD")
            .to_string();
        discovered.push((modified, CreatedWorldOption { root, label, manifest }));
    }

    discovered.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.label.cmp(&right.1.label))
    });

    discovered.into_iter().map(|(_, option)| option).collect()
}

fn format_job_error(error: JobError) -> String {
    match error {
        JobError::Shutdown => "SHUTDOWN".to_string(),
        JobError::WorkerDisconnected { worker_id } => format!("WORKER {worker_id} DISCONNECTED"),
        JobError::ExecutionFailed { message } => message.to_ascii_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_select_layout_hits_create_and_close_buttons() {
        let state = WorldSelectState::default();
        let layout = build_world_select_layout(&state, [1280.0, 720.0], Some("runtime"));

        assert_eq!(
            layout.action_at(layout.sections[0].buttons[0].rect.center()),
            Some(WorldSelectAction::CreateWorld)
        );
        assert_eq!(
            layout.action_at(layout.close_button.rect.center()),
            Some(WorldSelectAction::CloseWorldSelect)
        );
    }

    #[test]
    fn world_select_layout_hits_spinner_controls() {
        let mut state = WorldSelectState::default();
        state.available_created_worlds.push(CreatedWorldOption {
            root: PathBuf::from("target/world-create/runtime_seed_42"),
            label: "runtime_seed_42".to_string(),
            manifest: CreatedWorldManifest {
                format_version: 1,
                seed: 42,
                generator_version: 7,
                save_format_version: 1,
                min_chunk: [-2, -2, -2],
                max_chunk: [2, 3, 2],
                default_preview_center: [0, 0],
                stacks: Vec::new(),
            },
        });

        let layout = build_world_select_layout(&state, [1280.0, 720.0], Some("runtime"));

        assert_eq!(
            layout.action_at(layout.sections[1].fields[0].increase_rect.center()),
            Some(WorldSelectAction::SelectNextCreatedWorld)
        );
        assert_eq!(
            layout.action_at(layout.sections[2].fields[1].decrease_rect.center()),
            Some(WorldSelectAction::DecrementSpawnChunkZ)
        );
    }
}
