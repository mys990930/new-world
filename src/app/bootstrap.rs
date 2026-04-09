use super::{AppConfig, AppTimingState, GameApp};
use crate::ecs::EcsRuntime;
use crate::jobs::{JobConfig, JobSystem};
use crate::platform::{Platform, PlatformConfig};
use crate::renderer::{
    RenderConfig, RenderTextureArraySource, RenderTextureSource, RenderTextureTile, Renderer,
    StubSurfaceTarget,
};
use crate::world::{BlockRegistry, TextureTileSource, WorldCore, WorldMeta};

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
        ecs.spawn_default_player();
        let world = WorldCore::new(WorldMeta::new(7), block_registry);
        let jobs = JobSystem::new(JobConfig::default());
        let timing = AppTimingState::new(&config);

        Self {
            config,
            platform,
            ecs,
            world,
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
