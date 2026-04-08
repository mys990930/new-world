use crate::platform::Platform;
use crate::app::config::AppConfig;

pub struct GameApp {
    pub config: AppConfig,
    pub platform: Platform,
    pub frame_index: u64,
}