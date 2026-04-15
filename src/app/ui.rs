use std::fs;
use std::path::{Path, PathBuf};

use winit::keyboard::KeyCode;

use super::GameApp;
use crate::jobs::{JobError, JobRequest};
use crate::world::{CreatedWorldManifest, read_created_world_manifest};

const DEFAULT_CREATE_WORLD_SEED: u64 = 42;
const MIN_CREATE_WORLD_RADIUS: i32 = 2;
const MAX_CREATE_WORLD_RADIUS: i32 = 12;

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
            status_line: "F1 CLOSE  UP DOWN SECTION  ENTER APPLY  B CREATE".to_string(),
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

        if self.ui.mode == AppMode::InGame && input.just_pressed_keys.contains(&KeyCode::Tab) {
            self.ui.show_minimap_overlay = !self.ui.show_minimap_overlay;
        }

        if self.ui.mode != AppMode::WorldSelect {
            return;
        }

        if input.just_pressed_keys.contains(&KeyCode::Escape) {
            self.close_world_select();
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

    fn open_world_select(&mut self) {
        self.refresh_created_worlds();
        self.sync_world_select_with_runtime();
        self.ui.mode = AppMode::WorldSelect;
        self.ui.world_select.status_line =
            "UP DOWN SECTION  LEFT RIGHT CHANGE  Q E DEPTH  ENTER APPLY  B CREATE".to_string();
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
                "NO CREATED WORLDS FOUND. PRESS B TO CREATE.".to_string();
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
        self.ui.world_select.spawn_chunk_x += dx;
        self.ui.world_select.spawn_chunk_z += dz;
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
