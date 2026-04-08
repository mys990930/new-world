use super::event::PlatformEvent;

#[derive(Debug, Clone)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub focused: bool,
    pub minimized: bool,
    pub resized_this_frame: bool,
    pub close_requested: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            scale_factor: 1.0,
            focused: false,
            minimized: false,
            resized_this_frame: false,
            close_requested: false,
        }
    }
}

impl WindowState {
    pub fn begin_frame(&mut self) {
        self.resized_this_frame = false;
    }

    pub fn apply_event(&mut self, event: &PlatformEvent) {
        match event {
            PlatformEvent::WindowResized {
                physical_width,
                physical_height,
            } => {
                self.width = *physical_width;
                self.height = *physical_height;
                self.minimized = *physical_width == 0 || *physical_height == 0;
                self.resized_this_frame = true;
            }
            PlatformEvent::ScaleFactorChanged { scale_factor } => {
                self.scale_factor = *scale_factor;
            }
            PlatformEvent::FocusChanged { focused } => {
                self.focused = *focused;
            }
            PlatformEvent::MinimizedChanged { minimized } => {
                self.minimized = *minimized;
            }
            PlatformEvent::CloseRequested => {
                self.close_requested = true;
            }
            _ => {}
        }
    }
}
