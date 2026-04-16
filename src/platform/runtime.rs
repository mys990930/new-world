use std::sync::Arc;

use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use super::event::PlatformEvent;
use super::{LifecycleState, PlatformConfig, RawInputState, WindowState};

pub struct PlatformRuntime {
    window: Option<Arc<Window>>,
}

impl PlatformRuntime {
    pub fn new() -> Self {
        Self { window: None }
    }

    pub fn resumed(
        &mut self,
        event_loop: &ActiveEventLoop,
        config: &PlatformConfig,
        window_state: &mut WindowState,
        input_state: &mut RawInputState,
        lifecycle_state: &mut LifecycleState,
    ) {
        self.ensure_window(event_loop, config);

        let mut events = Vec::new();
        if let Some(window) = self.window.as_ref() {
            let size = window.inner_size();
            events.push(PlatformEvent::WindowResized {
                physical_width: size.width,
                physical_height: size.height,
            });
            events.push(PlatformEvent::ScaleFactorChanged {
                scale_factor: window.scale_factor(),
            });
            events.push(PlatformEvent::MinimizedChanged {
                minimized: size.width == 0 || size.height == 0,
            });
        }
        events.push(PlatformEvent::Resumed);
        events.push(PlatformEvent::ActiveChanged { active: true });

        self.dispatch_events(events, window_state, input_state, lifecycle_state);
    }

    pub fn suspended(
        &mut self,
        window_state: &mut WindowState,
        input_state: &mut RawInputState,
        lifecycle_state: &mut LifecycleState,
    ) {
        self.dispatch_events(
            [
                PlatformEvent::Suspended,
                PlatformEvent::ActiveChanged { active: false },
            ],
            window_state,
            input_state,
            lifecycle_state,
        );
    }

    pub fn handle_window_event(
        &mut self,
        window_id: WindowId,
        event: &WindowEvent,
        window_state: &mut WindowState,
        input_state: &mut RawInputState,
        lifecycle_state: &mut LifecycleState,
    ) -> bool {
        if self.window_id() != Some(window_id) {
            return false;
        }

        let events = self.normalize_window_event(event);
        self.dispatch_events(events, window_state, input_state, lifecycle_state);
        true
    }

    fn ensure_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        config: &PlatformConfig,
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
        window.set_ime_allowed(true);

        self.window = Some(Arc::new(window));
    }

    fn normalize_window_event(&self, event: &WindowEvent) -> Vec<PlatformEvent> {
        let mut events = Vec::new();

        match event {
            WindowEvent::Resized(size) => {
                events.push(PlatformEvent::WindowResized {
                    physical_width: size.width,
                    physical_height: size.height,
                });
                events.push(PlatformEvent::MinimizedChanged {
                    minimized: size.width == 0 || size.height == 0,
                });
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                events.push(PlatformEvent::ScaleFactorChanged {
                    scale_factor: *scale_factor,
                });
            }
            WindowEvent::Focused(focused) => {
                events.push(PlatformEvent::FocusChanged { focused: *focused });
            }
            WindowEvent::CloseRequested => {
                events.push(PlatformEvent::CloseRequested);
                events.push(PlatformEvent::QuitRequested);
            }
            WindowEvent::CursorMoved { position, .. } => {
                events.push(PlatformEvent::CursorMoved {
                    x: position.x,
                    y: position.y,
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (delta_x, delta_y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(position) => {
                        (position.x as f32, position.y as f32)
                    }
                };
                events.push(PlatformEvent::MouseWheel { delta_x, delta_y });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                events.push(PlatformEvent::MouseButtonChanged {
                    button: (*button).into(),
                    pressed: matches!(state, ElementState::Pressed),
                });
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    events.push(PlatformEvent::KeyChanged {
                        key,
                        pressed: matches!(event.state, ElementState::Pressed),
                    });
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                events.push(PlatformEvent::ModifiersChanged {
                    modifiers: modifiers.state().into(),
                });
            }
            WindowEvent::Ime(Ime::Commit(text)) if !text.is_empty() => {
                events.push(PlatformEvent::TextInput { text: text.clone() });
            }
            WindowEvent::Occluded(occluded) => {
                events.push(PlatformEvent::MinimizedChanged {
                    minimized: *occluded,
                });
            }
            _ => {}
        }

        events
    }

    fn dispatch_events<I>(
        &self,
        events: I,
        window_state: &mut WindowState,
        input_state: &mut RawInputState,
        lifecycle_state: &mut LifecycleState,
    ) where
        I: IntoIterator<Item = PlatformEvent>,
    {
        for event in events {
            window_state.apply_event(&event);
            input_state.apply_event(&event);
            lifecycle_state.apply_event(&event);
        }
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

    pub fn window_handle(&self) -> Option<Arc<Window>> {
        self.window.clone()
    }
}
