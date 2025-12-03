use camera::freecam::{FreeCamInput, SpeedModifier};
use protocol::{
    keybind::{KeybindInput, KeybindMapping},
    OutboundGameControlEvent,
};

use crate::{OUTBOUND_EVENT_QUEUE, input::Input};

#[derive(Clone, Debug, PartialEq)]
pub struct Keybinds {
    mapping: KeybindMapping,
}

impl Keybinds {
    pub fn from_mapping(mapping: KeybindMapping) -> Self {
        Self { mapping }
    }

    pub fn set_mapping(&mut self, mapping: KeybindMapping) {
        self.mapping = mapping;
    }

    pub fn execute_general_bindings(
        &self,
        input: &mut Input,
    ) {
        Self::execute_bindings(
            input,
            &[
                (&self.mapping.play_path, OutboundGameControlEvent::PlayPath),
                (
                    &self.mapping.toggle_freecam,
                    OutboundGameControlEvent::ToggleFreecam,
                ),
                (
                    &self.mapping.toggle_debug_pause,
                    OutboundGameControlEvent::ToggleDebugPause,
                ),
                (
                    &self.mapping.toggle_game_speed,
                    OutboundGameControlEvent::ToggleGameSpeed,
                ),
                (
                    &self.mapping.toggle_hud,
                    OutboundGameControlEvent::ToggleHud,
                ),
            ],
        );
    }

    pub fn execute_freecam_bindings(&self, input: &Input) {
        Self::execute_bindings(
            input,
            &[
                (
                    &self.mapping.create_keyframe,
                    OutboundGameControlEvent::CreateKeyframe,
                ),
                (
                    &self.mapping.toggle_freecam_lock,
                    OutboundGameControlEvent::ToggleFreecamLock,
                ),
            ],
        );
    }

    fn execute_bindings(
        input: &Input,
        bindings: &[(&Option<KeybindInput>, OutboundGameControlEvent)],
    ) {
        for (binding, event) in bindings {
            if binding
                .as_ref()
                .is_none_or(|b| !Self::input_active(input, b))
            {
                continue;
            }

            // Safety: we should have skipped loop iter if binding was none so no panic should
            // ever happen.
            let binding = binding.as_ref().unwrap();

            // Dispatch multiple events for scroll events to deal with multiple ticks in a single
            // frame.
            if *binding == KeybindInput::ScrollUp || *binding == KeybindInput::ScrollDown {
                let ticks = input.mousewheel_delta().abs() / 120;
                for _ in 0..ticks {
                    OUTBOUND_EVENT_QUEUE.push(*event);
                }
            } else {
                OUTBOUND_EVENT_QUEUE.push(*event);
            }
        }
    }

    pub fn make_freecam_input(&self, input: &Input) -> FreeCamInput {
        let mut freecam_input = FreeCamInput::default();

        let orientation_delta = input.orientation_delta();
        freecam_input.mouse_delta = glam::Vec2::new(orientation_delta.0, orientation_delta.1);

        // TODO: make this bindable
        Self::input_active(input, &KeybindInput::KeyPressed(0x57))
            .then(|| freecam_input.forward += 1.0);
        Self::input_active(input, &KeybindInput::KeyPressed(0x53))
            .then(|| freecam_input.forward -= 1.0);
        Self::input_active(input, &KeybindInput::KeyPressed(0x44))
            .then(|| freecam_input.right += 1.0);
        Self::input_active(input, &KeybindInput::KeyPressed(0x41))
            .then(|| freecam_input.right -= 1.0);
        Self::input_active(input, &KeybindInput::KeyPressed(0x45)).then(|| freecam_input.up += 1.0);
        Self::input_active(input, &KeybindInput::KeyPressed(0x51)).then(|| freecam_input.up -= 1.0);

        // Tilt
        Self::input_active(input, &KeybindInput::KeyPressed(0x4D))
            .then(|| freecam_input.roll_delta += 1.0);
        Self::input_active(input, &KeybindInput::KeyPressed(0x4E))
            .then(|| freecam_input.roll_delta -= 1.0);

        // Fov control
        self.mapping
            .increase_fov
            .as_ref()
            .is_some_and(|b| Self::input_active(input, b))
            .then(|| freecam_input.fov_delta += 1.0f32.to_radians());
        self.mapping
            .decrease_fov
            .as_ref()
            .is_some_and(|b| Self::input_active(input, b))
            .then(|| freecam_input.fov_delta -= 1.0f32.to_radians());

        // Speed modifiers
        Self::input_active(input, &KeybindInput::KeyPressed(0xA4))
            .then(|| freecam_input.speed_modifier = SpeedModifier::Fast);
        Self::input_active(input, &KeybindInput::KeyPressed(0xA2))
            .then(|| freecam_input.speed_modifier = SpeedModifier::Slow);

        freecam_input
    }

    fn input_active(input: &Input, binding: &KeybindInput) -> bool {
        match binding {
            KeybindInput::KeyPressed(code) => input.key_pressed(*code),
            KeybindInput::KeyDown(code) => input.key_down(*code),
            KeybindInput::ScrollUp => input.mousewheel_delta() > 0,
            KeybindInput::ScrollDown => input.mousewheel_delta() < 0,
        }
    }
}
