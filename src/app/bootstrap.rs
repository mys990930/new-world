use std::path::{Path, PathBuf};

use super::{AppConfig, AppMinimapCache, AppTimingState, AppUiState, GameApp};
use crate::ecs::{EcsRuntime, QUARTER_VIEW_PERSPECTIVE_VERTICAL_FOV_RADIANS};
use crate::jobs::{JobConfig, JobRequest, JobSystem};
use crate::platform::{Platform, PlatformConfig};
use crate::renderer::{
    RenderConfig, RenderTextureArraySource, RenderTextureSource, RenderTextureTile,
    RenderUiTextureSource, Renderer, StubSurfaceTarget,
};
use crate::simulation::{SimulationConfig, SimulationCore};
use crate::world::{
    BlockRegistry, CHUNK_EDGE_I32, ChunkCoord, CreateWorldConfig, CreatedWorldSource,
    TextureTileSource, WorldCore, WorldMeta, detect_latest_created_world_root,
};

impl GameApp {
    pub fn new(config: AppConfig) -> Self {
        println!(
            "[app] boot: window={}x{} created_worlds_dir={} auto_open_latest={}",
            config.width,
            config.height,
            config
                .created_worlds_dir
                .as_deref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "none".to_string()),
            config.auto_open_latest_created_world
        );
        let platform = Platform::new(PlatformConfig {
            title: config.title.clone(),
            width: config.width,
            height: config.height,
        });
        let block_registry = std::sync::Arc::new(
            BlockRegistry::load_default().expect("failed to load block registry"),
        );
        let mut render_config = RenderConfig::default();
        render_config.camera_projection.vertical_fov_radians =
            QUARTER_VIEW_PERSPECTIVE_VERTICAL_FOV_RADIANS;
        let mut renderer = Renderer::new(
            &StubSurfaceTarget::new(config.width, config.height),
            render_config,
        )
        .expect("failed to create renderer");
        renderer
            .set_block_textures(block_registry_to_render_textures(block_registry.as_ref()))
            .expect("failed to load block textures");
        if let Err(error) = renderer.set_ui_texture(RenderUiTextureSource::File(ui_atlas_path())) {
            eprintln!("[app] failed to load UI atlas: {:?}", error);
        }
        let mut ecs = EcsRuntime::new();
        let created_world = open_created_world(&config);
        let world = WorldCore::new(initial_world_meta(created_world.as_ref()), block_registry);
        ecs.spawn_default_player();
        let pending_player_spawn_anchor =
            stage_created_world_spawn_anchor(&mut ecs, created_world.as_ref());
        if let Some(source) = created_world.as_ref() {
            let min = source.manifest().min_chunk_coord();
            let max = source.manifest().max_chunk_coord();
            println!(
                "[app] boot created world manifest: root={} min=({}, {}, {}) max=({}, {}, {}) chunks={} preview=({}, {}) spawn_anchor={}",
                source.root().display(),
                min.0,
                min.1,
                min.2,
                max.0,
                max.1,
                max.2,
                created_world_chunk_count(source),
                source.manifest().default_preview_center[0],
                source.manifest().default_preview_center[1],
                format_spawn_anchor(pending_player_spawn_anchor)
            );
        }
        let jobs = JobSystem::new(JobConfig::default());
        let simulation = SimulationCore::new(SimulationConfig::with_fixed_ticks_per_second(
            config.timing.fixed_tick_rate,
        ));
        let timing = AppTimingState::new(&config);
        let mut app = Self {
            config,
            platform,
            ecs,
            world,
            simulation,
            created_world,
            jobs,
            renderer,
            ui: AppUiState::default(),
            minimap: AppMinimapCache::default(),
            pending_chunk_mesh_commits: std::collections::VecDeque::new(),
            pending_player_spawn_anchor,
            timing,
        };
        if app.created_world.is_none() {
            println!("[app] no created world loaded; entering world select startup screen");
            app.enter_startup_world_select();
        }
        app.queue_loaded_world_minimap_rebuilds();
        app.sync_renderer_environment_from_world();
        app
    }

    pub(crate) fn request_create_world_from_ui(
        &mut self,
        seed: u64,
        center_x: i32,
        center_z: i32,
        radius: i32,
    ) -> Result<PathBuf, String> {
        let root = create_world_root_path(
            self.config.created_worlds_dir.as_deref(),
            seed,
            center_x,
            center_z,
            radius,
        );
        let config = CreateWorldConfig {
            seed,
            center_x,
            center_z,
            radius,
            min_y_chunk: -2,
            max_y_chunk: 3,
        };
        let total_chunks = config.total_chunk_count().unwrap_or(0);
        println!(
            "[app] queue create world: root={} seed={} center=({}, {}) radius={} y={}..{} chunks={}",
            root.display(),
            seed,
            center_x,
            center_z,
            radius,
            config.min_y_chunk,
            config.max_y_chunk,
            total_chunks
        );

        self.jobs
            .submit(JobRequest::CreateWorld {
                root: root.clone(),
                config,
                registry: self.world.block_registry_handle(),
            })
            .map_err(|error| format!("failed to queue create-world job: {error:?}"))?;
        println!("[app] queued create world: {}", root.display());
        Ok(root)
    }

    pub(crate) fn load_created_world_from_ui(
        &mut self,
        root: &Path,
        spawn_chunk_x: i32,
        spawn_chunk_z: i32,
    ) -> Result<(), String> {
        let source = CreatedWorldSource::open(root).map_err(|error| error.to_string())?;
        let min = source.manifest().min_chunk_coord();
        let max = source.manifest().max_chunk_coord();
        let preview_chunk = ChunkCoord(
            spawn_chunk_x.clamp(min.0, max.0),
            0,
            spawn_chunk_z.clamp(min.2, max.2),
        );
        println!(
            "[app] opening created world: root={} requested_spawn=({}, {}) clamped_spawn=({}, {}) min=({}, {}, {}) max=({}, {}, {}) chunks={}",
            root.display(),
            spawn_chunk_x,
            spawn_chunk_z,
            preview_chunk.0,
            preview_chunk.2,
            min.0,
            min.1,
            min.2,
            max.0,
            max.1,
            max.2,
            created_world_chunk_count(&source)
        );

        let world = WorldCore::new(
            source.manifest().world_meta(),
            self.world.block_registry_handle(),
        );
        let mut ecs = EcsRuntime::new();
        ecs.spawn_default_player();
        let anchor = chunk_center_anchor(preview_chunk);
        if !ecs.stage_local_player_for_chunk_loading(anchor, max.1) {
            return Err(format!(
                "failed to stage player near chunk {} {}",
                preview_chunk.0, preview_chunk.2
            ));
        }

        self.jobs.shutdown();
        self.renderer.clear_chunk_meshes();
        self.ecs = ecs;
        self.world = world;
        self.created_world = Some(source);
        self.jobs = JobSystem::new(JobConfig::default());
        self.minimap = AppMinimapCache::default();
        self.pending_chunk_mesh_commits.clear();
        self.pending_player_spawn_anchor = Some(anchor);
        self.sync_renderer_environment_from_world();
        self.queue_environment_region_resolve_for_focus();
        println!(
            "[app] opened created world {} at chunk {} {}; chunk loads will stream through jobs",
            root.display(),
            preview_chunk.0,
            preview_chunk.2
        );
        Ok(())
    }
}

fn block_registry_to_render_textures(registry: &BlockRegistry) -> RenderTextureArraySource {
    RenderTextureArraySource {
        tile_size: registry.tile_size(),
        tiles: registry
            .texture_tiles()
            .iter()
            .map(|tile| RenderTextureTile {
                layer: u32::from(tile.id.0),
                key: tile.key.clone(),
                source: match &tile.source {
                    TextureTileSource::BuiltinWhite => RenderTextureSource::BuiltinWhite,
                    TextureTileSource::File(path) => RenderTextureSource::File(path.clone()),
                },
            })
            .collect(),
    }
}

fn open_created_world(config: &AppConfig) -> Option<CreatedWorldSource> {
    if let Some(root) = config.preferred_created_world_root.as_deref() {
        if let Some(source) = try_open_created_world_root(root, "preferred") {
            return Some(source);
        }
    }

    if !config.auto_open_latest_created_world {
        return None;
    }

    let Some(base_dir) = config.created_worlds_dir.as_deref() else {
        return None;
    };

    let Ok(root) = detect_latest_created_world_root(base_dir) else {
        eprintln!(
            "[app] failed to inspect created worlds directory {}",
            base_dir.display()
        );
        return None;
    };
    let Some(root) = root else {
        return None;
    };

    try_open_created_world_root(&root, "detected")
}

fn try_open_created_world_root(root: &Path, label: &str) -> Option<CreatedWorldSource> {
    match CreatedWorldSource::open(root) {
        Ok(source) => {
            println!("[app] using {label} created world: {}", root.display());
            Some(source)
        }
        Err(error) => {
            eprintln!(
                "[app] failed to open {label} created world {}: {}",
                root.display(),
                error
            );
            None
        }
    }
}

fn initial_world_meta(created_world: Option<&CreatedWorldSource>) -> WorldMeta {
    created_world
        .map(|source| source.manifest().world_meta())
        .unwrap_or_else(|| WorldMeta::new(7))
}

fn stage_created_world_spawn_anchor(
    ecs: &mut EcsRuntime,
    created_world: Option<&CreatedWorldSource>,
) -> Option<[f32; 2]> {
    let source = created_world?;
    let preview = source.default_preview_chunk();
    let anchor = chunk_center_anchor(preview);
    if ecs.stage_local_player_for_chunk_loading(anchor, source.manifest().max_chunk_coord().1) {
        Some(anchor)
    } else {
        None
    }
}

fn created_world_chunk_count(source: &CreatedWorldSource) -> u32 {
    let min = source.manifest().min_chunk_coord();
    let max = source.manifest().max_chunk_coord();
    let x = i64::from(max.0) - i64::from(min.0) + 1;
    let y = i64::from(max.1) - i64::from(min.1) + 1;
    let z = i64::from(max.2) - i64::from(min.2) + 1;
    x.checked_mul(y)
        .and_then(|value| value.checked_mul(z))
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
}

fn format_spawn_anchor(anchor: Option<[f32; 2]>) -> String {
    anchor
        .map(|anchor| format!("[{:.1}, {:.1}]", anchor[0], anchor[1]))
        .unwrap_or_else(|| "none".to_string())
}

fn chunk_center_anchor(coord: ChunkCoord) -> [f32; 2] {
    [
        coord.0 as f32 * CHUNK_EDGE_I32 as f32 + CHUNK_EDGE_I32 as f32 * 0.5,
        coord.2 as f32 * CHUNK_EDGE_I32 as f32 + CHUNK_EDGE_I32 as f32 * 0.5,
    ]
}

fn ui_atlas_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("ui")
        .join("new_world_pixel_ui_atlas.png")
}

fn create_world_root_path(
    base_dir: Option<&Path>,
    seed: u64,
    center_x: i32,
    center_z: i32,
    radius: i32,
) -> PathBuf {
    let base_dir = base_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("target/world-create"));
    let meta = WorldMeta::new(seed);
    base_dir.join(format!(
        "runtime_seed_{seed}_cx{center_x}_cz{center_z}_r{radius}_v{}",
        meta.generator_version
    ))
}
