use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::platform::{Platform, PlatformConfig};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "Voxel Runtime".to_string(),
            width: 1280,
            height: 720,
        }
    }
}

pub struct GameApp {
    platform: Platform,
    frame_index: u64,
    last_frame_at: Instant,
}

impl GameApp {
    pub fn new(config: AppConfig) -> Self {
        let platform_config = PlatformConfig {
            title: config.title,
            width: config.width,
            height: config.height,
        };

        Self {
            platform: Platform::new(platform_config),
            frame_index: 0,
            last_frame_at: Instant::now(),
        }
    }

    fn update(&mut self) {
        self.frame_index += 1;

        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_at);
        self.last_frame_at = now;

        let window = self.platform.window_state();
        let input = self.platform.raw_input_state();

        let title = format!(
            "Voxel Runtime | frame={} | dt={:.2}ms | size={}x{} | mouse=({:.0}, {:.0})",
            self.frame_index,
            dt.as_secs_f64() * 1000.0,
            window.width,
            window.height,
            input.mouse_position.0,
            input.mouse_position.1,
        );

        self.platform.set_window_title(&title);
    }
}

impl ApplicationHandler for GameApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.platform.resumed(event_loop);
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.platform.suspended();
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {
        self.platform.begin_frame();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if !self.platform.handle_window_event(window_id, &event) {
            return;
        }

        if self.platform.window_state().close_requested
            || self.platform.lifecycle_state().quit_requested
        {
            event_loop.exit();
        }

        if let WindowEvent::RedrawRequested = event {
            // 나중에 renderer.render() 같은 연결부가 여기 들어오면 된다.
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.platform.window_state().close_requested
            || self.platform.lifecycle_state().quit_requested
        {
            event_loop.exit();
            return;
        }

        // 현재 최소 vertical slice 에서는
        // "platform snapshot 읽기 -> app update"만 실제로 수행한다.
        self.update();

        // 연속 프레임을 보고 싶으면 redraw 요청.
        self.platform.request_redraw();
        self.platform.end_frame();
    }
}