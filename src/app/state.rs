use crate::app::config::AppConfig;
use crate::ecs::EcsRuntime;
use crate::platform::Platform;

pub struct GameApp {
    pub config: AppConfig,
    pub platform: Platform,
    pub ecs: EcsRuntime,
    pub frame_index: u64,
}
