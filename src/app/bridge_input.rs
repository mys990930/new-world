use std::collections::HashSet;

use winit::keyboard::KeyCode;

use super::GameApp;
use crate::ecs::EcsInputSnapshot;

impl GameApp {
    pub fn bridge_platform_to_ecs(&mut self) {
        let window = self.platform.window_state();
        let input = self.platform.raw_input_state();
        let lifecycle = self.platform.lifecycle_state();

        self.ecs.insert_resource(EcsInputSnapshot {
            move_screen_x: axis(
                input.pressed_keys.contains(&KeyCode::KeyA),
                input.pressed_keys.contains(&KeyCode::KeyD),
            ),
            move_screen_y: axis(
                input.pressed_keys.contains(&KeyCode::KeyW),
                input.pressed_keys.contains(&KeyCode::KeyS),
            ),
            sprint_down: input.pressed_keys.contains(&KeyCode::ShiftLeft)
                || input.pressed_keys.contains(&KeyCode::ShiftRight),
            jump_just_pressed: jump_just_pressed(&input.just_pressed_keys),
            zoom_scroll_delta: if input.modifiers.control {
                input.wheel_delta.1
            } else {
                0.0
            },
            quickslot_scroll_steps: if input.modifiers.control {
                0
            } else {
                wheel_steps(input.wheel_delta.1)
            },
            primary_down: input.left_pressed,
            primary_just_pressed: input.left_just_pressed,
            secondary_down: input.right_pressed,
            secondary_just_pressed: input.right_just_pressed,
            rotate_camera: camera_rotation_axis(
                input.just_pressed_keys.contains(&KeyCode::KeyQ),
                input.just_pressed_keys.contains(&KeyCode::KeyE),
            ),
            recenter_camera: input.just_pressed_keys.contains(&KeyCode::KeyY),
            toggle_manipulation_mode: input.just_pressed_keys.contains(&KeyCode::Tab),
            toggle_inventory: input.just_pressed_keys.contains(&KeyCode::KeyI),
            select_quickslot: direct_quickslot_selection(&input.just_pressed_keys),
            cursor_screen_pos: input.mouse_position,
            cursor_screen_delta: input.mouse_delta,
            focused: window.focused,
            active: lifecycle.active,
        });
    }
}

fn axis(negative: bool, positive: bool) -> i8 {
    (positive as i8) - (negative as i8)
}

fn camera_rotation_axis(q_pressed: bool, e_pressed: bool) -> i8 {
    axis(e_pressed, q_pressed)
}

fn jump_just_pressed(keys: &HashSet<KeyCode>) -> bool {
    keys.contains(&KeyCode::Space)
}

fn wheel_steps(delta_y: f32) -> i8 {
    if !delta_y.is_finite() || delta_y.abs() <= f32::EPSILON {
        0
    } else if delta_y > 0.0 {
        -1
    } else {
        1
    }
}

fn direct_quickslot_selection(keys: &HashSet<KeyCode>) -> Option<u8> {
    const DIGITS: [(KeyCode, u8); 10] = [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
        (KeyCode::Digit5, 4),
        (KeyCode::Digit6, 5),
        (KeyCode::Digit7, 6),
        (KeyCode::Digit8, 7),
        (KeyCode::Digit9, 8),
        (KeyCode::Digit0, 9),
    ];

    DIGITS
        .into_iter()
        .find_map(|(key, slot)| keys.contains(&key).then_some(slot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q_rotation_maps_to_positive_quarter_turn() {
        assert_eq!(camera_rotation_axis(true, false), 1);
    }

    #[test]
    fn e_rotation_maps_to_negative_quarter_turn() {
        assert_eq!(camera_rotation_axis(false, true), -1);
    }

    #[test]
    fn space_maps_to_jump() {
        assert!(jump_just_pressed(&HashSet::from([KeyCode::Space])));
    }
}
