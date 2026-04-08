use super::{AppConfig, AppTimingState, GameApp};
use crate::ecs::EcsRuntime;
use crate::platform::{Platform, PlatformConfig};
use crate::renderer::{RenderConfig, Renderer, StubSurfaceTarget};

impl GameApp {
    pub fn new(config: AppConfig) -> Self {
        let platform = Platform::new(PlatformConfig {
            title: config.title.clone(),
            width: config.width,
            height: config.height,
        });
        let renderer = Renderer::new(
            &StubSurfaceTarget::new(config.width, config.height),
            RenderConfig::default(),
        )
        .expect("failed to create renderer");
        let mut ecs = EcsRuntime::new();
        ecs.spawn_default_player();
        let timing = AppTimingState::new(&config);

        Self {
            config,
            platform,
            ecs,
            renderer,
            timing,
        }
    }
}
