use super::{AppConfig, GameApp};
use crate::platform::{Platform, PlatformConfig};

impl GameApp {
    pub fn new(config: AppConfig) -> Self {
        let platform = Platform::new(PlatformConfig {
            title: config.title.clone(),
            width: config.width,
            height: config.height,
        });

        Self {
            config,
            platform,
            frame_index: 0,
        }
    }
}