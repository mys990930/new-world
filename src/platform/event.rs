use winit::event::MouseButton;
use winit::keyboard::{KeyCode, ModifiersState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlatformModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub logo: bool,
}

impl From<ModifiersState> for PlatformModifiers {
    fn from(value: ModifiersState) -> Self {
        Self {
            shift: value.shift_key(),
            control: value.control_key(),
            alt: value.alt_key(),
            logo: value.super_key(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformMouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

impl From<MouseButton> for PlatformMouseButton {
    fn from(value: MouseButton) -> Self {
        match value {
            MouseButton::Left => Self::Left,
            MouseButton::Right => Self::Right,
            MouseButton::Middle => Self::Middle,
            MouseButton::Back => Self::Back,
            MouseButton::Forward => Self::Forward,
            MouseButton::Other(index) => Self::Other(index),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlatformEvent {
    WindowResized {
        physical_width: u32,
        physical_height: u32,
    },
    ScaleFactorChanged {
        scale_factor: f64,
    },
    FocusChanged {
        focused: bool,
    },
    MinimizedChanged {
        minimized: bool,
    },
    CloseRequested,
    CursorMoved {
        x: f64,
        y: f64,
    },
    MouseButtonChanged {
        button: PlatformMouseButton,
        pressed: bool,
    },
    MouseWheel {
        delta_x: f32,
        delta_y: f32,
    },
    KeyChanged {
        key: KeyCode,
        pressed: bool,
    },
    ModifiersChanged {
        modifiers: PlatformModifiers,
    },
    TextInput {
        text: String,
    },
    ActiveChanged {
        active: bool,
    },
    Suspended,
    Resumed,
    QuitRequested,
}
