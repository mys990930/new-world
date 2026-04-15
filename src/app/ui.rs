use std::fs;
use std::path::{Path, PathBuf};

use winit::keyboard::KeyCode;

use super::GameApp;
use crate::jobs::{JobError, JobRequest};
use crate::platform::RawInputState;
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
const WORLD_SELECT_INFO_LINE_HEIGHT_PX: f32 = 18.0;
const WORLD_SELECT_INFO_LINE_GAP_PX: f32 = 26.0;
const WORLD_SELECT_BUTTON_HEIGHT_PX: f32 = 44.0;
const WORLD_SELECT_BUTTON_GAP_PX: f32 = 12.0;
const WORLD_SELECT_CLOSE_BUTTON_WIDTH_PX: f32 = 176.0;
const WORLD_SELECT_LIST_ROW_HEIGHT_PX: f32 = 52.0;
const WORLD_SELECT_LIST_ROW_GAP_PX: f32 = 8.0;
const WORLD_SELECT_LIST_FOOTER_GAP_PX: f32 = 18.0;
const WORLD_SELECT_POPUP_WIDTH_PX: f32 = 448.0;
const WORLD_SELECT_POPUP_HEIGHT_PX: f32 = 172.0;

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
pub enum WorldSelectInputField {
    CreateSeed,
    CreateRadius,
    CreateCenterX,
    CreateCenterZ,
    SpawnChunkX,
    SpawnChunkZ,
}

impl WorldSelectInputField {
    fn section(self) -> WorldSelectSection {
        match self {
            Self::CreateSeed
            | Self::CreateRadius
            | Self::CreateCenterX
            | Self::CreateCenterZ => WorldSelectSection::CreateWorld,
            Self::SpawnChunkX | Self::SpawnChunkZ => WorldSelectSection::SpawnChunk,
        }
    }

    fn accepts_negative(self) -> bool {
        matches!(
            self,
            Self::CreateCenterX | Self::CreateCenterZ | Self::SpawnChunkX | Self::SpawnChunkZ
        )
    }

    fn display_label(self) -> &'static str {
        match self {
            Self::CreateSeed => "CREATE SEED",
            Self::CreateRadius => "CREATE RADIUS",
            Self::CreateCenterX => "CREATE CENTER X",
            Self::CreateCenterZ => "CREATE CENTER Z",
            Self::SpawnChunkX => "SPAWN CHUNK X",
            Self::SpawnChunkZ => "SPAWN CHUNK Z",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldSelectAction {
    CloseWorldSelect,
    IncrementCreateSeed,
    DecrementCreateSeed,
    IncrementCreateRadius,
    DecrementCreateRadius,
    IncrementCreateCenterX,
    DecrementCreateCenterX,
    IncrementCreateCenterZ,
    DecrementCreateCenterZ,
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
            | Self::IncrementCreateCenterX
            | Self::DecrementCreateCenterX
            | Self::IncrementCreateCenterZ
            | Self::DecrementCreateCenterZ
            | Self::CreateWorld => Some(WorldSelectSection::CreateWorld),
            Self::LoadSelectedWorld => Some(WorldSelectSection::SelectCreatedWorld),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldSelectPendingJobKind {
    CreateWorld,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSelectPendingJob {
    pub kind: WorldSelectPendingJobKind,
    pub root: PathBuf,
    pub label: String,
}

impl WorldSelectPendingJob {
    fn create_world(root: &Path) -> Self {
        Self {
            kind: WorldSelectPendingJobKind::CreateWorld,
            root: root.to_path_buf(),
            label: root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("WORLD")
                .to_ascii_uppercase(),
        }
    }

    pub fn popup_title(&self) -> &'static str {
        match self.kind {
            WorldSelectPendingJobKind::CreateWorld => "JOB RUNNING",
        }
    }

    pub fn popup_message(&self) -> &'static str {
        match self.kind {
            WorldSelectPendingJobKind::CreateWorld => "CREATING WORLD",
        }
    }

    pub fn status_line(&self) -> String {
        match self.kind {
            WorldSelectPendingJobKind::CreateWorld => format!("CREATE IN PROGRESS {}", self.label),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSelectState {
    pub section: WorldSelectSection,
    pub available_created_worlds: Vec<CreatedWorldOption>,
    pub selected_created_world_index: usize,
    pub selected_created_world_scroll: usize,
    pub create_world_seed: u64,
    pub create_world_radius: i32,
    pub create_world_center_x: i32,
    pub create_world_center_z: i32,
    pub spawn_chunk_x: i32,
    pub spawn_chunk_z: i32,
    pub focused_input_field: Option<WorldSelectInputField>,
    pub input_buffer: String,
    pub pending_job: Option<WorldSelectPendingJob>,
    pub status_line: String,
}

impl Default for WorldSelectState {
    fn default() -> Self {
        Self {
            section: WorldSelectSection::SelectCreatedWorld,
            available_created_worlds: Vec::new(),
            selected_created_world_index: 0,
            selected_created_world_scroll: 0,
            create_world_seed: DEFAULT_CREATE_WORLD_SEED,
            create_world_radius: 6,
            create_world_center_x: 0,
            create_world_center_z: 0,
            spawn_chunk_x: 0,
            spawn_chunk_z: 0,
            focused_input_field: None,
            input_buffer: String::new(),
            pending_job: None,
            status_line: "CLICK A VALUE TO TYPE OR USE THE ARROWS".to_string(),
        }
    }
}

impl WorldSelectState {
    pub fn selected_created_world(&self) -> Option<&CreatedWorldOption> {
        self.available_created_worlds
            .get(self.selected_created_world_index)
    }

    pub fn busy(&self) -> bool {
        self.pending_job.is_some()
    }

    fn value_string(&self, field: WorldSelectInputField) -> String {
        if self.focused_input_field == Some(field) {
            return self.input_buffer.clone();
        }

        match field {
            WorldSelectInputField::CreateSeed => self.create_world_seed.to_string(),
            WorldSelectInputField::CreateRadius => self.create_world_radius.to_string(),
            WorldSelectInputField::CreateCenterX => self.create_world_center_x.to_string(),
            WorldSelectInputField::CreateCenterZ => self.create_world_center_z.to_string(),
            WorldSelectInputField::SpawnChunkX => self.spawn_chunk_x.to_string(),
            WorldSelectInputField::SpawnChunkZ => self.spawn_chunk_z.to_string(),
        }
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
    pub field: WorldSelectInputField,
    pub label: &'static str,
    pub value: String,
    pub label_rect: UiRectPx,
    pub value_rect: UiRectPx,
    pub increase_rect: UiRectPx,
    pub decrease_rect: UiRectPx,
    pub increase_action: WorldSelectAction,
    pub decrease_action: WorldSelectAction,
    pub enabled: bool,
    pub focused: bool,
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
pub(crate) struct WorldSelectWorldRowLayout {
    pub world_index: usize,
    pub rect: UiRectPx,
    pub label: String,
    pub detail: String,
    pub selected: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorldSelectListLayout {
    pub rect: UiRectPx,
    pub rows: Vec<WorldSelectWorldRowLayout>,
    pub total_rows: usize,
    pub first_visible_index: usize,
    pub visible_rows: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorldSelectSectionLayout {
    pub section: WorldSelectSection,
    pub rect: UiRectPx,
    pub fields: Vec<WorldSelectFieldLayout>,
    pub info_lines: Vec<WorldSelectInfoLineLayout>,
    pub buttons: Vec<WorldSelectButtonLayout>,
    pub world_list: Option<WorldSelectListLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorldSelectLoadingPopupLayout {
    pub overlay_rect: UiRectPx,
    pub rect: UiRectPx,
    pub title_rect: UiRectPx,
    pub message_rect: UiRectPx,
    pub detail_rect: UiRectPx,
    pub title: String,
    pub message: String,
    pub detail: String,
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
    pub loading_popup: Option<WorldSelectLoadingPopupLayout>,
}

impl WorldSelectLayout {
    pub fn action_at(&self, point: (f64, f64)) -> Option<WorldSelectAction> {
        if self.close_button.enabled && self.close_button.rect.contains(point) {
            return Some(self.close_button.action);
        }

        if self.loading_popup.is_some() {
            return None;
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

    pub fn input_field_at(&self, point: (f64, f64)) -> Option<WorldSelectInputField> {
        if self.loading_popup.is_some() {
            return None;
        }

        for section in &self.sections {
            for field in &section.fields {
                if field.enabled && field.value_rect.contains(point) {
                    return Some(field.field);
                }
            }
        }

        None
    }

    pub fn created_world_index_at(&self, point: (f64, f64)) -> Option<usize> {
        if self.loading_popup.is_some() {
            return None;
        }

        self.sections
            .iter()
            .find_map(|section| section.world_list.as_ref())
            .and_then(|list| {
                list.rows
                    .iter()
                    .find(|row| row.enabled && row.rect.contains(point))
                    .map(|row| row.world_index)
            })
    }

    pub fn section_at(&self, point: (f64, f64)) -> Option<WorldSelectSection> {
        if self.loading_popup.is_some() {
            return None;
        }

        self.sections
            .iter()
            .find(|section| section.rect.contains(point))
            .map(|section| section.section)
    }
}

#[derive(Debug, Clone, Copy)]
struct WorldSelectFrameRects {
    outer_rect: UiRectPx,
    title_rect: UiRectPx,
    footer_rect: UiRectPx,
    create_rect: UiRectPx,
    select_rect: UiRectPx,
    spawn_rect: UiRectPx,
}

pub(crate) fn build_world_select_layout(
    state: &WorldSelectState,
    viewport: [f32; 2],
    current_loaded_label: Option<&str>,
) -> WorldSelectLayout {
    let frame = build_world_select_frame_rects(viewport);
    let busy = state.busy();
    let world_selected = state.selected_created_world();
    let world_selection_enabled = world_selected.is_some() && !busy;

    let close_button = WorldSelectButtonLayout {
        rect: UiRectPx {
            x: frame.footer_rect.x + frame.footer_rect.w - WORLD_SELECT_CLOSE_BUTTON_WIDTH_PX - 18.0,
            y: frame.footer_rect.y + frame.footer_rect.h - WORLD_SELECT_BUTTON_HEIGHT_PX - 16.0,
            w: WORLD_SELECT_CLOSE_BUTTON_WIDTH_PX,
            h: WORLD_SELECT_BUTTON_HEIGHT_PX,
        },
        label: "CLOSE",
        action: WorldSelectAction::CloseWorldSelect,
        enabled: true,
    };

    let create_fields = vec![
        build_field_row(
            frame.create_rect,
            0,
            WorldSelectInputField::CreateSeed,
            "SEED",
            state.value_string(WorldSelectInputField::CreateSeed),
            WorldSelectAction::IncrementCreateSeed,
            WorldSelectAction::DecrementCreateSeed,
            !busy,
            state.focused_input_field == Some(WorldSelectInputField::CreateSeed),
        ),
        build_field_row(
            frame.create_rect,
            1,
            WorldSelectInputField::CreateRadius,
            "RADIUS",
            state.value_string(WorldSelectInputField::CreateRadius),
            WorldSelectAction::IncrementCreateRadius,
            WorldSelectAction::DecrementCreateRadius,
            !busy,
            state.focused_input_field == Some(WorldSelectInputField::CreateRadius),
        ),
        build_field_row(
            frame.create_rect,
            2,
            WorldSelectInputField::CreateCenterX,
            "CENTER X",
            state.value_string(WorldSelectInputField::CreateCenterX),
            WorldSelectAction::IncrementCreateCenterX,
            WorldSelectAction::DecrementCreateCenterX,
            !busy,
            state.focused_input_field == Some(WorldSelectInputField::CreateCenterX),
        ),
        build_field_row(
            frame.create_rect,
            3,
            WorldSelectInputField::CreateCenterZ,
            "CENTER Z",
            state.value_string(WorldSelectInputField::CreateCenterZ),
            WorldSelectAction::IncrementCreateCenterZ,
            WorldSelectAction::DecrementCreateCenterZ,
            !busy,
            state.focused_input_field == Some(WorldSelectInputField::CreateCenterZ),
        ),
    ];

    let create_last_bottom = create_fields
        .last()
        .map(|field| field.label_rect.y + field.label_rect.h)
        .unwrap_or(frame.create_rect.y + WORLD_SELECT_ROWS_TOP_PX);
    let create_info_start = create_last_bottom + 18.0;
    let create_section = WorldSelectSectionLayout {
        section: WorldSelectSection::CreateWorld,
        rect: frame.create_rect,
        fields: create_fields,
        info_lines: vec![
            build_info_line(frame.create_rect, create_info_start, 0, "CLICK A VALUE TO TYPE".to_string()),
            build_info_line(
                frame.create_rect,
                create_info_start,
                1,
                "CREATE WORLD QUEUES A BACKGROUND JOB".to_string(),
            ),
        ],
        buttons: vec![build_bottom_button(
            frame.create_rect,
            0,
            "CREATE WORLD",
            WorldSelectAction::CreateWorld,
            !busy,
        )],
        world_list: None,
    };

    let load_selected_button = build_bottom_button(
        frame.select_rect,
        0,
        "LOAD SELECTED",
        WorldSelectAction::LoadSelectedWorld,
        world_selection_enabled,
    );
    let select_list_layout =
        build_created_world_list_layout(state, frame.select_rect, load_selected_button.rect, 3);
    let selected_info_lines = if let Some(selected) = world_selected {
        let min = selected.manifest.min_chunk_coord();
        let max = selected.manifest.max_chunk_coord();
        let info_start =
            select_list_layout.rect.y + select_list_layout.rect.h + WORLD_SELECT_LIST_FOOTER_GAP_PX;
        vec![
            build_info_line(
                frame.select_rect,
                info_start,
                0,
                format!(
                    "{} TO {} OF {}",
                    select_list_layout.first_visible_index.saturating_add(1),
                    (select_list_layout.first_visible_index + select_list_layout.rows.len())
                        .min(select_list_layout.total_rows),
                    select_list_layout.total_rows
                ),
            ),
            build_info_line(frame.select_rect, info_start, 1, format!("SEED {}", selected.manifest.seed)),
            build_info_line(
                frame.select_rect,
                info_start,
                2,
                format!("BOUNDS {} {} {} {}", min.0, min.2, max.0, max.2),
            ),
        ]
    } else {
        let info_start =
            select_list_layout.rect.y + select_list_layout.rect.h + WORLD_SELECT_LIST_FOOTER_GAP_PX;
        vec![
            build_info_line(frame.select_rect, info_start, 0, "NO CREATED WORLDS".to_string()),
            build_info_line(frame.select_rect, info_start, 1, "CREATE A WORLD FIRST".to_string()),
            build_info_line(frame.select_rect, info_start, 2, "THEN CLICK IT TO SELECT".to_string()),
        ]
    };
    let select_section = WorldSelectSectionLayout {
        section: WorldSelectSection::SelectCreatedWorld,
        rect: frame.select_rect,
        fields: Vec::new(),
        info_lines: selected_info_lines,
        buttons: vec![load_selected_button],
        world_list: Some(select_list_layout),
    };

    let spawn_fields = vec![
        build_field_row(
            frame.spawn_rect,
            0,
            WorldSelectInputField::SpawnChunkX,
            "CHUNK X",
            state.value_string(WorldSelectInputField::SpawnChunkX),
            WorldSelectAction::IncrementSpawnChunkX,
            WorldSelectAction::DecrementSpawnChunkX,
            world_selection_enabled,
            state.focused_input_field == Some(WorldSelectInputField::SpawnChunkX),
        ),
        build_field_row(
            frame.spawn_rect,
            1,
            WorldSelectInputField::SpawnChunkZ,
            "CHUNK Z",
            state.value_string(WorldSelectInputField::SpawnChunkZ),
            WorldSelectAction::IncrementSpawnChunkZ,
            WorldSelectAction::DecrementSpawnChunkZ,
            world_selection_enabled,
            state.focused_input_field == Some(WorldSelectInputField::SpawnChunkZ),
        ),
    ];
    let spawn_last_bottom = spawn_fields
        .last()
        .map(|field| field.label_rect.y + field.label_rect.h)
        .unwrap_or(frame.spawn_rect.y + WORLD_SELECT_ROWS_TOP_PX);
    let spawn_info_start = spawn_last_bottom + 22.0;
    let spawn_info_lines = if let Some(selected) = world_selected {
        let min = selected.manifest.min_chunk_coord();
        let max = selected.manifest.max_chunk_coord();
        vec![
            build_info_line(
                frame.spawn_rect,
                spawn_info_start,
                0,
                format!("WORLD {}", selected.label.to_ascii_uppercase()),
            ),
            build_info_line(
                frame.spawn_rect,
                spawn_info_start,
                1,
                format!("BOUNDS {} {} {} {}", min.0, min.2, max.0, max.2),
            ),
            build_info_line(
                frame.spawn_rect,
                spawn_info_start,
                2,
                "RESET PREVIEW RESTORES THE DEFAULT CENTER".to_string(),
            ),
        ]
    } else {
        vec![
            build_info_line(frame.spawn_rect, spawn_info_start, 0, "SELECT A WORLD FIRST".to_string()),
            build_info_line(frame.spawn_rect, spawn_info_start, 1, "THEN ADJUST THE LOAD CHUNK".to_string()),
            build_info_line(frame.spawn_rect, spawn_info_start, 2, "LOAD AT CHUNK USES THIS DRAFT".to_string()),
        ]
    };
    let spawn_section = WorldSelectSectionLayout {
        section: WorldSelectSection::SpawnChunk,
        rect: frame.spawn_rect,
        fields: spawn_fields,
        info_lines: spawn_info_lines,
        buttons: vec![
            build_bottom_button(
                frame.spawn_rect,
                1,
                "RESET PREVIEW",
                WorldSelectAction::ResetSpawnChunk,
                world_selection_enabled,
            ),
            build_bottom_button(
                frame.spawn_rect,
                0,
                "LOAD AT CHUNK",
                WorldSelectAction::LoadSelectedWorldAtSpawn,
                world_selection_enabled,
            ),
        ],
        world_list: None,
    };

    WorldSelectLayout {
        outer_rect: frame.outer_rect,
        title_rect: frame.title_rect,
        footer_rect: frame.footer_rect,
        footer_status_rect: UiRectPx {
            x: frame.footer_rect.x + 18.0,
            y: frame.footer_rect.y + 16.0,
            w: frame.footer_rect.w - close_button.rect.w - 54.0,
            h: 22.0,
        },
        footer_current_rect: UiRectPx {
            x: frame.footer_rect.x + 18.0,
            y: frame.footer_rect.y + 46.0,
            w: frame.footer_rect.w - close_button.rect.w - 54.0,
            h: 18.0,
        },
        footer_current_label: current_loaded_label.unwrap_or("NONE").to_string(),
        close_button,
        sections: [create_section, select_section, spawn_section],
        loading_popup: state
            .pending_job
            .as_ref()
            .map(|pending| build_loading_popup_layout(viewport, pending)),
    }
}

fn build_world_select_frame_rects(viewport: [f32; 2]) -> WorldSelectFrameRects {
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

    WorldSelectFrameRects {
        outer_rect,
        title_rect,
        footer_rect,
        create_rect,
        select_rect,
        spawn_rect,
    }
}

fn build_field_row(
    panel: UiRectPx,
    row_index: usize,
    field: WorldSelectInputField,
    label: &'static str,
    value: String,
    increase_action: WorldSelectAction,
    decrease_action: WorldSelectAction,
    enabled: bool,
    focused: bool,
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
        field,
        label,
        value,
        label_rect,
        value_rect,
        increase_rect,
        decrease_rect,
        increase_action,
        decrease_action,
        enabled,
        focused,
    }
}

fn build_info_line(
    panel: UiRectPx,
    start_y: f32,
    index: usize,
    text: String,
) -> WorldSelectInfoLineLayout {
    WorldSelectInfoLineLayout {
        rect: UiRectPx {
            x: panel.x + WORLD_SELECT_SECTION_PADDING_PX,
            y: start_y + index as f32 * WORLD_SELECT_INFO_LINE_GAP_PX,
            w: panel.w - WORLD_SELECT_SECTION_PADDING_PX * 2.0,
            h: WORLD_SELECT_INFO_LINE_HEIGHT_PX,
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

fn build_created_world_list_layout(
    state: &WorldSelectState,
    panel: UiRectPx,
    load_button_rect: UiRectPx,
    info_line_count: usize,
) -> WorldSelectListLayout {
    let info_height = if info_line_count == 0 {
        0.0
    } else {
        WORLD_SELECT_INFO_LINE_HEIGHT_PX
            + (info_line_count.saturating_sub(1)) as f32 * WORLD_SELECT_INFO_LINE_GAP_PX
    };
    let list_top = panel.y + WORLD_SELECT_ROWS_TOP_PX;
    let list_bottom =
        load_button_rect.y - WORLD_SELECT_LIST_FOOTER_GAP_PX - info_height - WORLD_SELECT_LIST_FOOTER_GAP_PX;
    let rect = UiRectPx {
        x: panel.x + WORLD_SELECT_SECTION_PADDING_PX,
        y: list_top,
        w: panel.w - WORLD_SELECT_SECTION_PADDING_PX * 2.0,
        h: (list_bottom - list_top).max(WORLD_SELECT_LIST_ROW_HEIGHT_PX),
    };
    let visible_rows = ((rect.h + WORLD_SELECT_LIST_ROW_GAP_PX)
        / (WORLD_SELECT_LIST_ROW_HEIGHT_PX + WORLD_SELECT_LIST_ROW_GAP_PX))
        .floor()
        .max(1.0) as usize;
    let max_start = state
        .available_created_worlds
        .len()
        .saturating_sub(visible_rows);
    let first_visible_index = state.selected_created_world_scroll.min(max_start);
    let last_visible_index = (first_visible_index + visible_rows).min(state.available_created_worlds.len());

    let mut rows = Vec::new();
    for (visible_row, (world_index, world)) in state
        .available_created_worlds
        .iter()
        .enumerate()
        .skip(first_visible_index)
        .take(last_visible_index.saturating_sub(first_visible_index))
        .map(|(index, world)| (index - first_visible_index, (index, world)))
    {
        let row_rect = UiRectPx {
            x: rect.x,
            y: rect.y + visible_row as f32 * (WORLD_SELECT_LIST_ROW_HEIGHT_PX + WORLD_SELECT_LIST_ROW_GAP_PX),
            w: rect.w,
            h: WORLD_SELECT_LIST_ROW_HEIGHT_PX,
        };
        rows.push(WorldSelectWorldRowLayout {
            world_index,
            rect: row_rect,
            label: world.label.clone(),
            detail: format!(
                "#{}  SEED {}",
                world_index.saturating_add(1),
                world.manifest.seed
            ),
            selected: world_index == state.selected_created_world_index,
            enabled: !state.busy(),
        });
    }

    WorldSelectListLayout {
        rect,
        rows,
        total_rows: state.available_created_worlds.len(),
        first_visible_index,
        visible_rows,
    }
}

fn build_loading_popup_layout(
    viewport: [f32; 2],
    pending: &WorldSelectPendingJob,
) -> WorldSelectLoadingPopupLayout {
    let rect = UiRectPx {
        x: ((viewport[0] - WORLD_SELECT_POPUP_WIDTH_PX) * 0.5).max(40.0),
        y: ((viewport[1] - WORLD_SELECT_POPUP_HEIGHT_PX) * 0.5).max(40.0),
        w: WORLD_SELECT_POPUP_WIDTH_PX.min(viewport[0] - 80.0),
        h: WORLD_SELECT_POPUP_HEIGHT_PX.min(viewport[1] - 80.0),
    };

    WorldSelectLoadingPopupLayout {
        overlay_rect: UiRectPx {
            x: 0.0,
            y: 0.0,
            w: viewport[0],
            h: viewport[1],
        },
        title_rect: UiRectPx {
            x: rect.x + 24.0,
            y: rect.y + 22.0,
            w: rect.w - 48.0,
            h: 28.0,
        },
        message_rect: UiRectPx {
            x: rect.x + 24.0,
            y: rect.y + 72.0,
            w: rect.w - 48.0,
            h: 24.0,
        },
        detail_rect: UiRectPx {
            x: rect.x + 24.0,
            y: rect.y + 112.0,
            w: rect.w - 48.0,
            h: 18.0,
        },
        rect,
        title: pending.popup_title().to_string(),
        message: pending.popup_message().to_string(),
        detail: pending.label.clone(),
    }
}

impl GameApp {
    pub fn handle_ui_shortcuts(&mut self) {
        let input = self.platform.raw_input_state().clone();

        if input.just_pressed_keys.contains(&KeyCode::F1) {
            match self.ui.mode {
                AppMode::InGame => self.open_world_select(),
                AppMode::WorldSelect => {
                    self.cancel_world_select_input_edit();
                    self.close_world_select();
                }
            }
            return;
        }

        if self.ui.mode == AppMode::InGame && input.just_pressed_keys.contains(&KeyCode::KeyM) {
            self.ui.show_minimap_overlay = !self.ui.show_minimap_overlay;
        }

        if self.ui.mode != AppMode::WorldSelect {
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

        if input.just_pressed_keys.contains(&KeyCode::Escape) {
            self.cancel_world_select_input_edit();
            self.close_world_select();
            return;
        }

        if self.handle_world_select_text_input(&input) {
            return;
        }

        if !self.ui.world_select.busy() && input.wheel_delta.1.abs() > f32::EPSILON {
            if layout.section_at(input.mouse_position) == Some(WorldSelectSection::SelectCreatedWorld)
            {
                self.scroll_created_world_list(wheel_scroll_steps(input.wheel_delta.1));
            }
        }

        if input.left_just_pressed {
            if let Some(action) = layout.action_at(input.mouse_position) {
                if !self.commit_world_select_input_edit() {
                    return;
                }
                self.execute_world_select_action(action);
                return;
            }

            if self.ui.world_select.busy() {
                return;
            }

            if let Some(field) = layout.input_field_at(input.mouse_position) {
                if self.ui.world_select.focused_input_field != Some(field)
                    && !self.commit_world_select_input_edit()
                {
                    return;
                }
                self.focus_world_select_input_field(field);
                return;
            }

            if let Some(index) = layout.created_world_index_at(input.mouse_position) {
                if !self.commit_world_select_input_edit() {
                    return;
                }
                self.select_created_world_by_index(index);
                return;
            }

            if !self.commit_world_select_input_edit() {
                return;
            }

            if let Some(section) = layout.section_at(input.mouse_position) {
                self.ui.world_select.section = section;
            } else {
                self.cancel_world_select_input_edit();
            }
        }

        if self.ui.world_select.busy() || self.ui.world_select.focused_input_field.is_some() {
            return;
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
            self.set_spawn_status_line();
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

                if input.just_pressed_keys.contains(&KeyCode::KeyQ)
                    || input.just_pressed_keys.contains(&KeyCode::KeyW)
                {
                    self.adjust_world_create_center(0, -1);
                }

                if input.just_pressed_keys.contains(&KeyCode::KeyE)
                    || input.just_pressed_keys.contains(&KeyCode::KeyS)
                {
                    self.adjust_world_create_center(0, 1);
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

    fn handle_world_select_text_input(&mut self, input: &RawInputState) -> bool {
        let Some(field) = self.ui.world_select.focused_input_field else {
            return false;
        };

        if self.ui.world_select.busy() {
            return false;
        }

        let mut consumed = false;
        if input.just_pressed_keys.contains(&KeyCode::Backspace) {
            self.ui.world_select.input_buffer.pop();
            consumed = true;
        }

        if !input.text_input.is_empty() {
            for character in input.text_input.chars() {
                if self.try_push_world_select_input_char(field, character) {
                    consumed = true;
                }
            }
        }

        if input.just_pressed_keys.contains(&KeyCode::Enter) {
            let _ = self.commit_world_select_input_edit();
            return true;
        }

        consumed
    }

    fn try_push_world_select_input_char(
        &mut self,
        field: WorldSelectInputField,
        character: char,
    ) -> bool {
        if character.is_ascii_digit() {
            self.ui.world_select.input_buffer.push(character);
            return true;
        }

        if field.accepts_negative()
            && character == '-'
            && self.ui.world_select.input_buffer.is_empty()
        {
            self.ui.world_select.input_buffer.push(character);
            return true;
        }

        false
    }

    fn focus_world_select_input_field(&mut self, field: WorldSelectInputField) {
        self.ui.world_select.section = field.section();
        self.ui.world_select.focused_input_field = Some(field);
        self.ui.world_select.input_buffer = self.world_select_field_value_string(field);
        self.ui.world_select.status_line = format!("EDIT {}", field.display_label());
    }

    fn cancel_world_select_input_edit(&mut self) {
        self.ui.world_select.focused_input_field = None;
        self.ui.world_select.input_buffer.clear();
    }

    fn commit_world_select_input_edit(&mut self) -> bool {
        let Some(field) = self.ui.world_select.focused_input_field else {
            return true;
        };

        let buffer = self.ui.world_select.input_buffer.clone();
        if buffer.is_empty() || buffer == "-" {
            self.ui.world_select.status_line = format!("ENTER {}", field.display_label());
            return false;
        }

        let result = match field {
            WorldSelectInputField::CreateSeed => buffer
                .parse::<u64>()
                .map(|value| {
                    self.ui.world_select.create_world_seed = value;
                    self.set_create_status_line();
                })
                .map_err(|_| "INVALID CREATE SEED".to_string()),
            WorldSelectInputField::CreateRadius => buffer
                .parse::<i32>()
                .map(|value| {
                    self.ui.world_select.create_world_radius =
                        value.clamp(MIN_CREATE_WORLD_RADIUS, MAX_CREATE_WORLD_RADIUS);
                    self.set_create_status_line();
                })
                .map_err(|_| "INVALID CREATE RADIUS".to_string()),
            WorldSelectInputField::CreateCenterX => buffer
                .parse::<i32>()
                .map(|value| {
                    self.ui.world_select.create_world_center_x = value;
                    self.set_create_status_line();
                })
                .map_err(|_| "INVALID CREATE CENTER X".to_string()),
            WorldSelectInputField::CreateCenterZ => buffer
                .parse::<i32>()
                .map(|value| {
                    self.ui.world_select.create_world_center_z = value;
                    self.set_create_status_line();
                })
                .map_err(|_| "INVALID CREATE CENTER Z".to_string()),
            WorldSelectInputField::SpawnChunkX => buffer
                .parse::<i32>()
                .map(|value| {
                    self.ui.world_select.spawn_chunk_x = value;
                    self.clamp_world_select_spawn_to_selected_created_world();
                    self.set_spawn_status_line();
                })
                .map_err(|_| "INVALID SPAWN CHUNK X".to_string()),
            WorldSelectInputField::SpawnChunkZ => buffer
                .parse::<i32>()
                .map(|value| {
                    self.ui.world_select.spawn_chunk_z = value;
                    self.clamp_world_select_spawn_to_selected_created_world();
                    self.set_spawn_status_line();
                })
                .map_err(|_| "INVALID SPAWN CHUNK Z".to_string()),
        };

        match result {
            Ok(()) => {
                self.cancel_world_select_input_edit();
                true
            }
            Err(message) => {
                self.ui.world_select.status_line = message;
                false
            }
        }
    }

    fn world_select_field_value_string(&self, field: WorldSelectInputField) -> String {
        match field {
            WorldSelectInputField::CreateSeed => self.ui.world_select.create_world_seed.to_string(),
            WorldSelectInputField::CreateRadius => self.ui.world_select.create_world_radius.to_string(),
            WorldSelectInputField::CreateCenterX => {
                self.ui.world_select.create_world_center_x.to_string()
            }
            WorldSelectInputField::CreateCenterZ => {
                self.ui.world_select.create_world_center_z.to_string()
            }
            WorldSelectInputField::SpawnChunkX => self.ui.world_select.spawn_chunk_x.to_string(),
            WorldSelectInputField::SpawnChunkZ => self.ui.world_select.spawn_chunk_z.to_string(),
        }
    }

    fn sync_focused_input_field(&mut self, field: WorldSelectInputField) {
        if self.ui.world_select.focused_input_field == Some(field) {
            self.ui.world_select.input_buffer = self.world_select_field_value_string(field);
        }
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
            WorldSelectAction::IncrementCreateCenterX => self.adjust_world_create_center(1, 0),
            WorldSelectAction::DecrementCreateCenterX => self.adjust_world_create_center(-1, 0),
            WorldSelectAction::IncrementCreateCenterZ => self.adjust_world_create_center(0, 1),
            WorldSelectAction::DecrementCreateCenterZ => self.adjust_world_create_center(0, -1),
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
                self.set_spawn_status_line();
            }
        }
    }

    fn open_world_select(&mut self) {
        self.refresh_created_worlds();
        self.sync_world_select_with_runtime();
        self.cancel_world_select_input_edit();
        self.ui.mode = AppMode::WorldSelect;
        if let Some(pending) = self.ui.world_select.pending_job.as_ref() {
            self.ui.world_select.status_line = pending.status_line();
        } else if self.ui.world_select.available_created_worlds.is_empty() {
            self.ui.world_select.status_line = "NO CREATED WORLDS FOUND. CREATE ONE FIRST.".to_string();
        } else {
            self.ui.world_select.status_line =
                "CLICK A VALUE TO TYPE. SCROLL THE WORLD LIST.".to_string();
        }
    }

    fn close_world_select(&mut self) {
        self.cancel_world_select_input_edit();
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
            self.ui.world_select.selected_created_world_scroll = 0;
            if self.ui.world_select.pending_job.is_none() {
                self.ui.world_select.status_line =
                    "NO CREATED WORLDS FOUND. CREATE ONE FIRST.".to_string();
            }
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

        self.ensure_selected_created_world_visible();
    }

    fn sync_world_select_with_runtime(&mut self) {
        if let Some(current_root) = self
            .created_world
            .as_ref()
            .map(|source| source.root().to_path_buf())
        {
            if let Some(index) = self
                .ui
                .world_select
                .available_created_worlds
                .iter()
                .position(|created_world| created_world.root == current_root)
            {
                self.ui.world_select.selected_created_world_index = index;
                self.ensure_selected_created_world_visible();
            }
        }

        if let Some(player_chunk) = self.current_player_chunk_coord() {
            if self.ui.world_select.create_world_center_x == 0
                && self.ui.world_select.create_world_center_z == 0
            {
                self.ui.world_select.create_world_center_x = player_chunk[0];
                self.ui.world_select.create_world_center_z = player_chunk[1];
            }
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
        self.sync_focused_input_field(WorldSelectInputField::CreateSeed);
        self.set_create_status_line();
    }

    fn adjust_world_create_radius(&mut self, delta: i32) {
        self.ui.world_select.create_world_radius =
            (self.ui.world_select.create_world_radius + delta)
                .clamp(MIN_CREATE_WORLD_RADIUS, MAX_CREATE_WORLD_RADIUS);
        self.sync_focused_input_field(WorldSelectInputField::CreateRadius);
        self.set_create_status_line();
    }

    fn adjust_world_create_center(&mut self, dx: i32, dz: i32) {
        self.ui.world_select.create_world_center_x =
            self.ui.world_select.create_world_center_x.saturating_add(dx);
        self.ui.world_select.create_world_center_z =
            self.ui.world_select.create_world_center_z.saturating_add(dz);
        self.sync_focused_input_field(WorldSelectInputField::CreateCenterX);
        self.sync_focused_input_field(WorldSelectInputField::CreateCenterZ);
        self.set_create_status_line();
    }

    fn select_created_world(&mut self, delta: i32) {
        let count = self.ui.world_select.available_created_worlds.len();
        if count == 0 {
            self.ui.world_select.status_line = "NO CREATED WORLDS AVAILABLE".to_string();
            return;
        }

        let current = self.ui.world_select.selected_created_world_index as i32;
        let next = (current + delta).rem_euclid(count as i32) as usize;
        self.select_created_world_by_index(next);
    }

    fn select_created_world_by_index(&mut self, index: usize) {
        if self.ui.world_select.available_created_worlds.is_empty() {
            self.ui.world_select.status_line = "NO CREATED WORLDS AVAILABLE".to_string();
            return;
        }

        self.ui.world_select.selected_created_world_index = index
            .min(self.ui.world_select.available_created_worlds.len().saturating_sub(1));
        self.ensure_selected_created_world_visible();
        self.reset_world_select_spawn_to_selected_created_world();

        if let Some(selected) = self.ui.world_select.selected_created_world() {
            self.ui.world_select.status_line =
                format!("SELECTED {}", selected.label.to_ascii_uppercase());
        }
    }

    fn scroll_created_world_list(&mut self, delta_rows: i32) {
        if delta_rows == 0 || self.ui.world_select.available_created_worlds.is_empty() {
            return;
        }

        let visible_rows = self.visible_created_world_rows();
        let max_start = self
            .ui
            .world_select
            .available_created_worlds
            .len()
            .saturating_sub(visible_rows);
        let next = (self.ui.world_select.selected_created_world_scroll as i32 + delta_rows)
            .clamp(0, max_start as i32) as usize;
        self.ui.world_select.selected_created_world_scroll = next;
    }

    fn ensure_selected_created_world_visible(&mut self) {
        let visible_rows = self.visible_created_world_rows();
        if visible_rows == 0 || self.ui.world_select.available_created_worlds.is_empty() {
            self.ui.world_select.selected_created_world_scroll = 0;
            return;
        }

        let selected = self.ui.world_select.selected_created_world_index;
        if selected < self.ui.world_select.selected_created_world_scroll {
            self.ui.world_select.selected_created_world_scroll = selected;
        } else if selected
            >= self.ui.world_select.selected_created_world_scroll.saturating_add(visible_rows)
        {
            self.ui.world_select.selected_created_world_scroll =
                selected.saturating_add(1).saturating_sub(visible_rows);
        }
    }

    fn visible_created_world_rows(&self) -> usize {
        let window = self.platform.window_state();
        let frame = build_world_select_frame_rects([window.width as f32, window.height as f32]);
        let load_button_rect = build_bottom_button(
            frame.select_rect,
            0,
            "LOAD SELECTED",
            WorldSelectAction::LoadSelectedWorld,
            true,
        )
        .rect;
        build_created_world_list_layout(&self.ui.world_select, frame.select_rect, load_button_rect, 3)
            .visible_rows
    }

    fn adjust_world_select_spawn(&mut self, dx: i32, dz: i32) {
        self.ui.world_select.spawn_chunk_x = self.ui.world_select.spawn_chunk_x.saturating_add(dx);
        self.ui.world_select.spawn_chunk_z = self.ui.world_select.spawn_chunk_z.saturating_add(dz);
        self.clamp_world_select_spawn_to_selected_created_world();
        self.sync_focused_input_field(WorldSelectInputField::SpawnChunkX);
        self.sync_focused_input_field(WorldSelectInputField::SpawnChunkZ);
        self.set_spawn_status_line();
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
        self.sync_focused_input_field(WorldSelectInputField::SpawnChunkX);
        self.sync_focused_input_field(WorldSelectInputField::SpawnChunkZ);
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
        if self.ui.world_select.busy() || !self.commit_world_select_input_edit() {
            return;
        }

        let center_x = self.ui.world_select.create_world_center_x;
        let center_z = self.ui.world_select.create_world_center_z;
        let radius = self.ui.world_select.create_world_radius;
        let seed = self.ui.world_select.create_world_seed;

        match self.request_create_world_from_ui(seed, center_x, center_z, radius) {
            Ok(root) => {
                let pending = WorldSelectPendingJob::create_world(root.as_path());
                self.ui.world_select.pending_job = Some(pending.clone());
                self.ui.world_select.status_line = pending.status_line();
                self.cancel_world_select_input_edit();
            }
            Err(message) => {
                self.ui.world_select.status_line = format!("CREATE FAILED {}", message);
            }
        }
    }

    fn execute_world_select_load(&mut self) {
        if self.ui.world_select.busy() || !self.commit_world_select_input_edit() {
            return;
        }

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
                self.ui.world_select.status_line =
                    format!("LOADED {}", selected.label.to_ascii_uppercase());
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
        self.clear_pending_job_for_root(root.as_path());
        self.refresh_created_worlds();
        if let Some(index) = self
            .ui
            .world_select
            .available_created_worlds
            .iter()
            .position(|created_world| created_world.root == root)
        {
            self.ui.world_select.selected_created_world_index = index;
            self.ensure_selected_created_world_visible();
        }
        self.ui.world_select.create_world_center_x = manifest.default_preview_center[0];
        self.ui.world_select.create_world_center_z = manifest.default_preview_center[1];
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
            self.clear_pending_job_for_root(root.as_path());
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

    fn clear_pending_job_for_root(&mut self, root: &Path) {
        if self
            .ui
            .world_select
            .pending_job
            .as_ref()
            .map(|pending| pending.root.as_path() == root)
            .unwrap_or(false)
        {
            self.ui.world_select.pending_job = None;
        }
    }

    fn set_create_status_line(&mut self) {
        self.ui.world_select.status_line = format!(
            "CREATE SEED {}  RADIUS {}  CENTER {} {}",
            self.ui.world_select.create_world_seed,
            self.ui.world_select.create_world_radius,
            self.ui.world_select.create_world_center_x,
            self.ui.world_select.create_world_center_z
        );
    }

    fn set_spawn_status_line(&mut self) {
        self.ui.world_select.status_line = format!(
            "SPAWN CHUNK {} {}",
            self.ui.world_select.spawn_chunk_x, self.ui.world_select.spawn_chunk_z
        );
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

fn wheel_scroll_steps(delta_y: f32) -> i32 {
    if delta_y.abs() <= f32::EPSILON {
        return 0;
    }

    let steps = delta_y.abs().ceil() as i32;
    if delta_y > 0.0 { -steps } else { steps }
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

    fn sample_world(index: usize) -> CreatedWorldOption {
        CreatedWorldOption {
            root: PathBuf::from(format!("target/world-create/runtime_seed_{}", index)),
            label: format!("runtime_seed_{}", index),
            manifest: CreatedWorldManifest {
                format_version: 1,
                seed: index as u64,
                generator_version: 7,
                save_format_version: 1,
                min_chunk: [-2, -2, -2],
                max_chunk: [2, 3, 2],
                default_preview_center: [index as i32, index as i32],
                stacks: Vec::new(),
            },
        }
    }

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
    fn world_select_layout_hits_value_fields_and_world_rows() {
        let mut state = WorldSelectState::default();
        state.available_created_worlds = (0..4).map(sample_world).collect();

        let layout = build_world_select_layout(&state, [1280.0, 720.0], Some("runtime"));
        let list = layout.sections[1]
            .world_list
            .as_ref()
            .expect("world list layout should exist");

        assert_eq!(
            layout.input_field_at(layout.sections[0].fields[2].value_rect.center()),
            Some(WorldSelectInputField::CreateCenterX)
        );
        assert_eq!(
            layout.created_world_index_at(list.rows[1].rect.center()),
            Some(1)
        );
    }

    #[test]
    fn loading_popup_blocks_background_interactions() {
        let mut state = WorldSelectState::default();
        state.pending_job = Some(WorldSelectPendingJob::create_world(Path::new(
            "target/world-create/runtime_seed_42",
        )));

        let layout = build_world_select_layout(&state, [1280.0, 720.0], Some("runtime"));

        assert_eq!(
            layout.action_at(layout.close_button.rect.center()),
            Some(WorldSelectAction::CloseWorldSelect)
        );
        assert_eq!(
            layout.input_field_at(layout.sections[0].fields[0].value_rect.center()),
            None
        );
    }
}
