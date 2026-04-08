use super::{AppConfig, AppTimingState, GameApp};
use crate::ecs::EcsRuntime;
use crate::platform::{Platform, PlatformConfig};

impl GameApp {
    pub fn new(config: AppConfig) -> Self {
        let timing = AppTimingState::new(&config);
        let platform = Platform::new(PlatformConfig {
            title: config.title.clone(),
            width: config.width,
            height: config.height,
        });
        let mut ecs = EcsRuntime::new();
        ecs.spawn_default_player();

        Self {
            config,
            platform,
            ecs,
            timing,
        }
    }
}
