use super::event::PlatformEvent;

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
    pub fn apply_event(&mut self, event: &PlatformEvent) {
        match event {
            PlatformEvent::ActiveChanged { active } => {
                self.active = *active;
            }
            PlatformEvent::Suspended => {
                self.active = false;
                self.suspended = true;
            }
            PlatformEvent::Resumed => {
                self.suspended = false;
            }
            PlatformEvent::QuitRequested => {
                self.quit_requested = true;
            }
            _ => {}
        }
    }
}
