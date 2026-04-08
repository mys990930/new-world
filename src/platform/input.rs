use std::collections::HashSet;

use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Debug, Clone, Default)]
pub struct RawInputState {
    pub mouse_position: (f64, f64),
    pub mouse_delta: (f64, f64),
    pub wheel_delta: (f32, f32),

    pub pressed_keys: HashSet<KeyCode>,
    pub just_pressed_keys: HashSet<KeyCode>,
    pub just_released_keys: HashSet<KeyCode>,

    pub left_pressed: bool,
    pub left_just_pressed: bool,
    pub left_just_released: bool,
}

impl RawInputState {
    pub fn begin_frame(&mut self) {
        self.mouse_delta = (0.0, 0.0);
        self.wheel_delta = (0.0, 0.0);
        self.just_pressed_keys.clear();
        self.just_released_keys.clear();
        self.left_just_pressed = false;
        self.left_just_released = false;
    }

    pub fn clear_pressed(&mut self) {
        self.pressed_keys.clear();
        self.just_pressed_keys.clear();
        self.just_released_keys.clear();
        self.left_pressed = false;
        self.left_just_pressed = false;
        self.left_just_released = false;
    }

    pub fn apply_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let next = (position.x, position.y);
                self.mouse_delta.0 += next.0 - self.mouse_position.0;
                self.mouse_delta.1 += next.1 - self.mouse_position.1;
                self.mouse_position = next;
            }

            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::LineDelta(x, y) => {
                    self.wheel_delta.0 += *x;
                    self.wheel_delta.1 += *y;
                }
                MouseScrollDelta::PixelDelta(pos) => {
                    self.wheel_delta.0 += pos.x as f32;
                    self.wheel_delta.1 += pos.y as f32;
                }
            },

            WindowEvent::MouseInput { state, button, .. } if *button == MouseButton::Left => {
                match state {
                    ElementState::Pressed => {
                        if !self.left_pressed {
                            self.left_just_pressed = true;
                        }
                        self.left_pressed = true;
                    }
                    ElementState::Released => {
                        self.left_pressed = false;
                        self.left_just_released = true;
                    }
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            if self.pressed_keys.insert(keycode) {
                                self.just_pressed_keys.insert(keycode);
                            }
                        }
                        ElementState::Released => {
                            self.pressed_keys.remove(&keycode);
                            self.just_released_keys.insert(keycode);
                        }
                    }
                }
            }

            _ => {}
        }
    }
}