pub mod event;
pub mod input;
pub mod lifecycle;
pub mod runtime;
pub mod window;

#[allow(unused_imports)]
pub use event::{PlatformEvent, PlatformModifiers, PlatformMouseButton};
pub use input::RawInputState;
pub use lifecycle::LifecycleState;
pub use window::WindowState;

use std::sync::Arc;

use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use runtime::PlatformRuntime;

#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

pub struct Platform {
    config: PlatformConfig,
    runtime: PlatformRuntime,
    window: WindowState,
    input: RawInputState,
    lifecycle: LifecycleState,
}

impl Platform {
    pub fn new(config: PlatformConfig) -> Self {
        Self {
            config,
            runtime: PlatformRuntime::new(),
            window: WindowState::default(),
            input: RawInputState::default(),
            lifecycle: LifecycleState::default(),
        }
    }

    pub fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.runtime.resumed(
            event_loop,
            &self.config,
            &mut self.window,
            &mut self.input,
            &mut self.lifecycle,
        );
    }

    pub fn suspended(&mut self) {
        self.runtime
            .suspended(&mut self.window, &mut self.input, &mut self.lifecycle);
    }

    pub fn begin_frame(&mut self) {
        self.window.begin_frame();
        self.input.begin_frame();
    }

    pub fn end_frame(&mut self) {
        // 지금은 no-op.
        // 이후 profiling / frame stats / transient finalize가 필요하면 여기.
    }

    pub fn handle_window_event(&mut self, window_id: WindowId, event: &WindowEvent) -> bool {
        self.runtime.handle_window_event(
            window_id,
            event,
            &mut self.window,
            &mut self.input,
            &mut self.lifecycle,
        )
    }

    pub fn request_redraw(&self) {
        self.runtime.request_redraw();
    }

    pub fn set_window_title(&self, title: &str) {
        self.runtime.set_title(title);
    }

    pub fn window_handle(&self) -> Option<Arc<Window>> {
        self.runtime.window_handle()
    }

    pub fn window_state(&self) -> &WindowState {
        &self.window
    }

    pub fn raw_input_state(&self) -> &RawInputState {
        &self.input
    }

    pub fn lifecycle_state(&self) -> &LifecycleState {
        &self.lifecycle
    }
}
