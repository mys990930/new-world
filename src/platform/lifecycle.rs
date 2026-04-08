use winit::event::WindowEvent;

#[derive(Debug, Clone)]
pub struct LifecycleState {
    pub active: bool,
    pub suspended: bool,
    pub quit_requested: bool,
}

impl Default for LifecycleState {
    fn default() -> Self {
        Self {
            active: false,
            suspended: false,
            quit_requested: false,
        }
    }
}

impl LifecycleState {
    pub fn on_resumed(&mut self) {
        self.active = true;
        self.suspended = false;
    }

    pub fn on_suspended(&mut self) {
        self.active = false;
        self.suspended = true;
    }

    pub fn apply_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Focused(focused) => {
                self.active = *focused;
            }
            WindowEvent::CloseRequested => {
                self.quit_requested = true;
            }
            _ => {}
        }
    }
}