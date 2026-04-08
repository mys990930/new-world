use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub timing: TimingConfig,
}

#[derive(Debug, Clone)]
pub struct TimingConfig {
    pub target_frame_rate: Option<u32>,
}

impl TimingConfig {
    pub fn target_frame_interval(&self) -> Option<Duration> {
        self.target_frame_rate
            .filter(|target| *target > 0)
            .map(|target| Duration::from_secs_f64(1.0 / target as f64))
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "Voxel Runtime".to_string(),
            width: 1280,
            height: 720,
            timing: TimingConfig::default(),
        }
    }
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            target_frame_rate: Some(60),
        }
    }
}
