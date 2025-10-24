use std::{sync::atomic::Ordering, time::Duration};

use camera::freecam::FreeCamInput;
use protocol::{
    OutboundGameControlEvent, SettingsData, keybind::{KeybindInput, KeybindMapping}
};

use crate::{input::Input, DISABLE_HUD, OUTBOUND_EVENT_QUEUE};

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

    pub fn execute_general_bindings(&self, input: &mut Input) {
        Self::execute_bindings(
            input,
            &[
                (&self.mapping.toggle_freecam, OutboundGameControlEvent::ToggleFreecam),
                (&self.mapping.toggle_debug_pause, OutboundGameControlEvent::ToggleDebugPause),
                (&self.mapping.toggle_game_speed, OutboundGameControlEvent::ToggleGameSpeed),
                (&self.mapping.toggle_hud, OutboundGameControlEvent::ToggleHud),
            ]
        );
    }

    pub fn execute_freecam_bindings(&self, input: &Input) {
        Self::execute_bindings(
            input,
            &[
                (&self.mapping.create_keyframe, OutboundGameControlEvent::CreateKeyframe),
                (&self.mapping.play_path, OutboundGameControlEvent::PlayPath),
                (&self.mapping.increase_fov, OutboundGameControlEvent::IncreaseFov),
                (&self.mapping.decrease_fov, OutboundGameControlEvent::DecreaseFov),
                (&self.mapping.toggle_freecam_lock, OutboundGameControlEvent::ToggleFreecamLock),
            ]
        );
    }

    fn execute_bindings(input: &Input, bindings: &[(&Option<KeybindInput>, OutboundGameControlEvent)]) {
        for (binding, event) in bindings {
            let active = match binding.as_ref() {
                None => false,
                Some(KeybindInput::Keyboard(code)) => input.key_down(*code),
                Some(KeybindInput::ScrollUp) => input.mousewheel_delta() > 0.0,
                Some(KeybindInput::ScrollDown) => input.mousewheel_delta() < 0.0,
            };

            if !active {
                continue;
            }

            OUTBOUND_EVENT_QUEUE.push(*event);
        }
    }

    pub fn make_freecam_input(&self, input: &Input) -> FreeCamInput {
        let mut freecam_input = FreeCamInput::default();

        let orientation_delta = input.orientation_delta();
        freecam_input.mouse_delta = glam::Vec2::new(
            orientation_delta.0,
            orientation_delta.1,
        );

        // TODO: make it bindable
        //
        // Movement
        input.key_pressed(0x57).then(|| freecam_input.forward += 1.0);
        input.key_pressed(0x53).then(|| freecam_input.forward -= 1.0);
        input.key_pressed(0x44).then(|| freecam_input.right += 1.0);
        input.key_pressed(0x41).then(|| freecam_input.right -= 1.0);
        input.key_pressed(0x45).then(|| freecam_input.up += 1.0);
        input.key_pressed(0x51).then(|| freecam_input.up -= 1.0);

        // Tilt
        input.key_pressed(0x4D).then(|| freecam_input.roll_delta += 1.0);
        input.key_pressed(0x4E).then(|| freecam_input.roll_delta -= 1.0);

        // Speed modifiers
        input.key_pressed(0xA4).then(|| freecam_input.speed_modifier = 4.0);
        input.key_pressed(0xA2).then(|| freecam_input.speed_modifier = 1.0 / 4.0);

        freecam_input
    }
}
