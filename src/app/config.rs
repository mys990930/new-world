use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub timing: TimingConfig,
    pub preferred_created_world_root: Option<PathBuf>,
    pub created_worlds_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct TimingConfig {
    pub target_frame_rate: Option<u32>,
    pub fixed_tick_rate: u32,
    pub max_fixed_steps_per_frame: u32,
}

impl TimingConfig {
    pub fn target_frame_interval(&self) -> Option<Duration> {
        self.target_frame_rate
            .filter(|target| *target > 0)
            .map(|target| Duration::from_secs_f64(1.0 / target as f64))
    }

    pub fn fixed_step_interval(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.fixed_tick_rate.max(1) as f64)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "Voxel Runtime".to_string(),
            width: 1280,
            height: 720,
            timing: TimingConfig::default(),
            preferred_created_world_root: Some(PathBuf::from(
                "target/world-create/runtime_seed_42_160_-144",
            )),
            created_worlds_dir: Some(PathBuf::from("target/world-create")),
        }
    }
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            target_frame_rate: Some(60),
            fixed_tick_rate: 20,
            max_fixed_steps_per_frame: 4,
        }
    }
}
