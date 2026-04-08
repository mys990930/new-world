use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use super::{PlatformConfig, WindowState};

pub struct PlatformRuntime {
    window: Option<Window>,
}

impl PlatformRuntime {
    pub fn new() -> Self {
        Self { window: None }
    }

    pub fn ensure_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        config: &PlatformConfig,
        window_state: &mut WindowState,
    ) {
        if self.window.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title(config.title.clone())
            .with_inner_size(LogicalSize::new(config.width as f64, config.height as f64));

        let window = event_loop
            .create_window(attributes)
            .expect("failed to create window");

        let size = window.inner_size();
        window_state.width = size.width;
        window_state.height = size.height;

        self.window = Some(window);
    }

    pub fn window_id(&self) -> Option<WindowId> {
        self.window.as_ref().map(|window| window.id())
    }

    pub fn request_redraw(&self) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    pub fn set_title(&self, title: &str) {
        if let Some(window) = self.window.as_ref() {
            window.set_title(title);
        }
    }
}