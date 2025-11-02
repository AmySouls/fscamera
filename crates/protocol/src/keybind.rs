use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeybindMapping {
    pub create_keyframe: Option<KeybindInput>,
    pub play_path: Option<KeybindInput>,
    pub toggle_freecam: Option<KeybindInput>,
    pub toggle_freecam_lock: Option<KeybindInput>,
    pub toggle_hud: Option<KeybindInput>,
    pub toggle_debug_pause: Option<KeybindInput>,
    pub toggle_game_speed: Option<KeybindInput>,
    pub increase_fov: Option<KeybindInput>,
    pub decrease_fov: Option<KeybindInput>,
}

impl Default for KeybindMapping {
    fn default() -> Self {
        Self {
            // F10
            create_keyframe: Some(KeybindInput::KeyDown(0x79)),
            // F4
            play_path: Some(KeybindInput::KeyDown(0x73)),
            // F9
            toggle_freecam: Some(KeybindInput::KeyDown(0x78)),
            // F8
            toggle_freecam_lock: Some(KeybindInput::KeyDown(0x77)),
            // DEL
            toggle_hud: Some(KeybindInput::KeyDown(0x2e)),
            // P
            toggle_debug_pause: Some(KeybindInput::KeyDown(0x50)),
            // O
            toggle_game_speed: Some(KeybindInput::KeyDown(0x4f)),

            increase_fov: Some(KeybindInput::ScrollDown),
            decrease_fov: Some(KeybindInput::ScrollUp),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum KeybindInput {
    KeyPressed(i32),
    KeyDown(i32),
    ScrollUp,
    ScrollDown,
}
