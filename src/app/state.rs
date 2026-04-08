use std::time::{Duration, Instant};

use crate::app::config::AppConfig;
use crate::ecs::EcsRuntime;
use crate::platform::Platform;

pub struct GameApp {
    pub config: AppConfig,
    pub platform: Platform,
    pub ecs: EcsRuntime,
    pub timing: AppTimingState,
}

pub struct AppTimingState {
    pub frame_index: u64,
    pub frame_dt: Duration,
    pub last_frame_instant: Instant,
    pub next_frame_deadline: Option<Instant>,
}

impl AppTimingState {
    pub fn new(config: &AppConfig) -> Self {
        let now = Instant::now();
        let next_frame_deadline = if config.timing.target_frame_interval().is_some() {
            Some(now)
        } else {
            None
        };

        Self {
            frame_index: 0,
            frame_dt: Duration::ZERO,
            last_frame_instant: now,
            next_frame_deadline,
        }
    }
}

impl GameApp {
    pub fn frame_deadline(&self) -> Option<Instant> {
        self.timing.next_frame_deadline
    }

    pub fn should_run_frame(&self, now: Instant) -> bool {
        match self.timing.next_frame_deadline {
            Some(deadline) => now >= deadline,
            None => true,
        }
    }

    pub fn begin_timed_frame(&mut self, now: Instant) {
        self.timing.frame_dt = now.saturating_duration_since(self.timing.last_frame_instant);
        self.timing.last_frame_instant = now;
        self.timing.frame_index = self.timing.frame_index.saturating_add(1);
        self.timing.next_frame_deadline = self
            .config
            .timing
            .target_frame_interval()
            .and_then(|interval| now.checked_add(interval));
    }
}
