use std::path::{Path, PathBuf};

use super::{AppConfig, AppMinimapCache, AppTimingState, AppUiState, GameApp};
use crate::ecs::{
    EcsRuntime, HORIZONTAL_INTEREST_CHUNK_RADIUS, QUARTER_VIEW_PERSPECTIVE_VERTICAL_FOV_RADIANS,
};
use crate::jobs::{JobConfig, JobRequest, JobSystem};
use crate::platform::{Platform, PlatformConfig};
use crate::renderer::{
    RenderConfig, RenderTextureArraySource, RenderTextureSource, RenderTextureTile, Renderer,
    RenderUiTextureSource, StubSurfaceTarget,
};
use crate::world::{
    CreatedWorldSource, CreateWorldConfig, BlockRegistry, CHUNK_EDGE_I32, ChunkCoord,
    TextureTileSource, WorldCore, WorldMeta, detect_latest_created_world_root, generate_chunk,
};

impl GameApp {
    pub fn new(config: AppConfig) -> Self {
        let platform = Platform::new(PlatformConfig {
            title: config.title.clone(),
            width: config.width,
            height: config.height,
        });
        let block_registry =
            std::sync::Arc::new(BlockRegistry::load_default().expect("failed to load block registry"));
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
        let mut world = WorldCore::new(initial_world_meta(created_world.as_ref()), block_registry);
        let spawn_anchor = preload_spawn_neighborhood(&mut world, created_world.as_ref());
        ecs.spawn_default_player();
        if let Some(anchor) = spawn_anchor {
            if !ecs.place_local_player_on_surface(&world, anchor) {
                eprintln!(
                    "[app] failed to place player on loaded surface near [{:.1}, {:.1}]",
                    anchor[0], anchor[1]
                );
            }
        }
        let jobs = JobSystem::new(JobConfig::default());
        let timing = AppTimingState::new(&config);
        let mut app = Self {
            config,
            platform,
            ecs,
            world,
            created_world,
            jobs,
            renderer,
            ui: AppUiState::default(),
            minimap: AppMinimapCache::default(),
            timing,
        };
        app.queue_loaded_world_minimap_rebuilds();
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

        let mut world = WorldCore::new(source.manifest().world_meta(), self.world.block_registry_handle());
        for coord in preload_created_column_coords(&source, preview_chunk) {
            let chunk = source
                .load_chunk(coord)
                .map_err(|error| format!("failed to load {:?}: {}", coord, error))?;
            world.insert_chunk(coord, chunk);
        }

        let mut ecs = EcsRuntime::new();
        ecs.spawn_default_player();
        let anchor = chunk_center_anchor(preview_chunk);
        if !ecs.place_local_player_on_surface(&world, anchor) {
            return Err(format!(
                "failed to place player near chunk {} {}",
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
        self.queue_loaded_world_minimap_rebuilds();
        println!(
            "[app] loaded created world {} at chunk {} {}",
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

fn preload_spawn_neighborhood(
    world: &mut WorldCore,
    created_world: Option<&CreatedWorldSource>,
) -> Option<[f32; 2]> {
    if let Some(source) = created_world {
        let preview = source.default_preview_chunk();
        for coord in preload_created_column_coords(source, preview) {
            match source.load_chunk(coord) {
                Ok(chunk) => world.insert_chunk(coord, chunk),
                Err(error) => eprintln!("[app] failed to preload created-world chunk {:?}: {}", coord, error),
            }
        }

        return Some(chunk_center_anchor(preview));
    }

    let preview = ChunkCoord(0, 0, 0);
    for coord in preload_generated_chunk_coords(preview) {
        let chunk = generate_chunk(coord, world.meta(), world.block_registry());
        world.insert_chunk(coord, chunk);
    }

    Some(chunk_center_anchor(preview))
}

fn preload_created_column_coords(
    created_world: &CreatedWorldSource,
    preview_chunk: ChunkCoord,
) -> Vec<ChunkCoord> {
    let min = created_world.manifest().min_chunk_coord();
    let max = created_world.manifest().max_chunk_coord();
    let mut coords = Vec::new();
    let radius = HORIZONTAL_INTEREST_CHUNK_RADIUS;

    for z in (preview_chunk.2 - radius)..=(preview_chunk.2 + radius) {
        for x in (preview_chunk.0 - radius)..=(preview_chunk.0 + radius) {
            for y in min.1..=max.1 {
                let coord = ChunkCoord(x, y, z);
                if created_world.contains_chunk(coord) {
                    coords.push(coord);
                }
            }
        }
    }

    coords
}

fn preload_generated_chunk_coords(preview_chunk: ChunkCoord) -> Vec<ChunkCoord> {
    let mut coords = Vec::new();
    let radius = HORIZONTAL_INTEREST_CHUNK_RADIUS;
    for z in (preview_chunk.2 - radius)..=(preview_chunk.2 + radius) {
        for x in (preview_chunk.0 - radius)..=(preview_chunk.0 + radius) {
            coords.push(ChunkCoord(x, preview_chunk.1, z));
        }
    }
    coords
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
        .join("pixel_ui_atlas.png")
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
