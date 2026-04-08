use std::collections::HashSet;

use winit::keyboard::KeyCode;

use super::event::{PlatformEvent, PlatformModifiers, PlatformMouseButton};

#[derive(Debug, Clone, Default)]
pub struct RawInputState {
    pub mouse_position: (f64, f64),
    pub mouse_delta: (f64, f64),
    pub wheel_delta: (f32, f32),
    pub modifiers: PlatformModifiers,
    pub text_input: String,

    pub pressed_keys: HashSet<KeyCode>,
    pub just_pressed_keys: HashSet<KeyCode>,
    pub just_released_keys: HashSet<KeyCode>,

    pub left_pressed: bool,
    pub left_just_pressed: bool,
    pub left_just_released: bool,
    pub right_pressed: bool,
    pub right_just_pressed: bool,
    pub right_just_released: bool,
}

impl RawInputState {
    pub fn begin_frame(&mut self) {
        self.mouse_delta = (0.0, 0.0);
        self.wheel_delta = (0.0, 0.0);
        self.just_pressed_keys.clear();
        self.just_released_keys.clear();
        self.left_just_pressed = false;
        self.left_just_released = false;
        self.right_just_pressed = false;
        self.right_just_released = false;
        self.text_input.clear();
    }

    pub fn clear_pressed(&mut self) {
        self.pressed_keys.clear();
        self.just_pressed_keys.clear();
        self.just_released_keys.clear();
        self.modifiers = PlatformModifiers::default();
        self.text_input.clear();
        self.left_pressed = false;
        self.left_just_pressed = false;
        self.left_just_released = false;
        self.right_pressed = false;
        self.right_just_pressed = false;
        self.right_just_released = false;
    }

    pub fn apply_event(&mut self, event: &PlatformEvent) {
        match event {
            PlatformEvent::CursorMoved { x, y } => {
                let next = (*x, *y);
                self.mouse_delta.0 += next.0 - self.mouse_position.0;
                self.mouse_delta.1 += next.1 - self.mouse_position.1;
                self.mouse_position = next;
            }

            PlatformEvent::MouseWheel { delta_x, delta_y } => {
                self.wheel_delta.0 += *delta_x;
                self.wheel_delta.1 += *delta_y;
            }

            PlatformEvent::MouseButtonChanged { button, pressed } => match button {
                PlatformMouseButton::Left => {
                    if *pressed {
                        if !self.left_pressed {
                            self.left_just_pressed = true;
                        }
                        self.left_pressed = true;
                    } else {
                        if self.left_pressed {
                            self.left_just_released = true;
                        }
                        self.left_pressed = false;
                    }
                }
                PlatformMouseButton::Right => {
                    if *pressed {
                        if !self.right_pressed {
                            self.right_just_pressed = true;
                        }
                        self.right_pressed = true;
                    } else {
                        if self.right_pressed {
                            self.right_just_released = true;
                        }
                        self.right_pressed = false;
                    }
                }
                _ => {}
            },

            PlatformEvent::KeyChanged { key, pressed } => {
                if *pressed {
                    if self.pressed_keys.insert(*key) {
                        self.just_pressed_keys.insert(*key);
                    }
                } else {
                    self.pressed_keys.remove(key);
                    self.just_released_keys.insert(*key);
                }
            }

            PlatformEvent::ModifiersChanged { modifiers } => {
                self.modifiers = *modifiers;
            }

            PlatformEvent::TextInput { text } => {
                self.text_input.push_str(text);
            }

            PlatformEvent::FocusChanged { focused: false } | PlatformEvent::Suspended => {
                self.clear_pressed();
            }

            _ => {}
        }
    }
}
