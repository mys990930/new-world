use winit::event::WindowEvent;

#[derive(Debug, Clone)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
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

    pub fn apply_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.width = size.width;
                self.height = size.height;
                self.resized_this_frame = true;
            }
            WindowEvent::Focused(focused) => {
                self.focused = *focused;
            }
            WindowEvent::CloseRequested => {
                self.close_requested = true;
            }
            _ => {}
        }
    }
}