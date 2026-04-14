use super::{AppConfig, AppTimingState, GameApp};
use crate::ecs::{EcsRuntime, HORIZONTAL_INTEREST_CHUNK_RADIUS};
use crate::jobs::{JobConfig, JobSystem};
use crate::platform::{Platform, PlatformConfig};
use crate::renderer::{
    RenderConfig, RenderTextureArraySource, RenderTextureSource, RenderTextureTile, Renderer,
    StubSurfaceTarget,
};
use crate::world::{
    BakedWorldSource, BlockRegistry, CHUNK_EDGE_I32, ChunkCoord, TextureTileSource, WorldCore,
    WorldMeta, detect_latest_baked_world_root, generate_chunk,
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
        let mut renderer = Renderer::new(
            &StubSurfaceTarget::new(config.width, config.height),
            RenderConfig::default(),
        )
        .expect("failed to create renderer");
        renderer
            .set_block_textures(block_registry_to_render_textures(block_registry.as_ref()))
            .expect("failed to load block textures");
        let mut ecs = EcsRuntime::new();
        let baked_world = open_baked_world(&config);
        let mut world = WorldCore::new(initial_world_meta(baked_world.as_ref()), block_registry);
        let spawn_anchor = preload_spawn_neighborhood(&mut world, baked_world.as_ref());
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

        Self {
            config,
            platform,
            ecs,
            world,
            baked_world,
            jobs,
            renderer,
            timing,
        }
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

fn open_baked_world(config: &AppConfig) -> Option<BakedWorldSource> {
    let Some(base_dir) = config.baked_worlds_dir.as_deref() else {
        return None;
    };

    let Ok(root) = detect_latest_baked_world_root(base_dir) else {
        eprintln!(
            "[app] failed to inspect baked worlds directory {}",
            base_dir.display()
        );
        return None;
    };
    let Some(root) = root else {
        return None;
    };

    match BakedWorldSource::open(&root) {
        Ok(source) => {
            println!("[app] using baked world: {}", root.display());
            Some(source)
        }
        Err(error) => {
            eprintln!(
                "[app] failed to open baked world {}: {}",
                root.display(),
                error
            );
            None
        }
    }
}

fn initial_world_meta(baked_world: Option<&BakedWorldSource>) -> WorldMeta {
    baked_world
        .map(|source| source.manifest().world_meta())
        .unwrap_or_else(|| WorldMeta::new(7))
}

fn preload_spawn_neighborhood(
    world: &mut WorldCore,
    baked_world: Option<&BakedWorldSource>,
) -> Option<[f32; 2]> {
    if let Some(source) = baked_world {
        let preview = source.default_preview_chunk();
        for coord in preload_baked_column_coords(source, preview) {
            match source.load_chunk(coord) {
                Ok(chunk) => world.insert_chunk(coord, chunk),
                Err(error) => eprintln!("[app] failed to preload baked chunk {:?}: {}", coord, error),
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

fn preload_baked_column_coords(
    baked_world: &BakedWorldSource,
    preview_chunk: ChunkCoord,
) -> Vec<ChunkCoord> {
    let min = baked_world.manifest().min_chunk_coord();
    let max = baked_world.manifest().max_chunk_coord();
    let mut coords = Vec::new();
    let radius = HORIZONTAL_INTEREST_CHUNK_RADIUS;

    for z in (preview_chunk.2 - radius)..=(preview_chunk.2 + radius) {
        for x in (preview_chunk.0 - radius)..=(preview_chunk.0 + radius) {
            for y in min.1..=max.1 {
                let coord = ChunkCoord(x, y, z);
                if baked_world.contains_chunk(coord) {
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
