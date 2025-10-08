use std::{sync::atomic::Ordering, time::Duration};

use protocol::{KeybindAction, OutboundGameControlEvent};

use crate::{DISABLE_HUD, OUTBOUND_EVENT_QUEUE, input::Input};

#[derive(Default)]
pub struct Keybinds {
    pub keybinds: Vec<protocol::Keybind>,
}

impl Keybinds {
    pub fn execute(
        &self,
        input: &mut Input,
    ) {
        for binding in self.keybinds.iter().filter(|k| k.active) {
            let active = match binding.input {
                None => false,
                Some(protocol::KeybindInput::Keyboard(code)) => {
                    if binding.action.is_debounced() {
                        input.key_pressed_debounced(code, Duration::from_millis(500))
                    } else {
                        input.key_pressed(code)
                    }
                },
            };

            if !active {
                break;
            }

            OUTBOUND_EVENT_QUEUE
                .push(OutboundGameControlEvent::KeybindAction(binding.action.clone()));

            // match binding.action {
            //     // KeybindAction::TogglePlaybackMode => todo!(),
            //     KeybindAction::CreateKeyframe => {
            //     },
            //     KeybindAction::ToggleHUD => {
            //         DISABLE_HUD.fetch_not(Ordering::Relaxed);
            //     },
            //     _ => {},
            //     // KeybindAction::ToggleCharacterNoDead => todo!(),
            //     // KeybindAction::ToggleCharacterNoMove => todo!(),
            //     // KeybindAction::ToggleFovOverride => todo!(),
            //     // KeybindAction::SetFov(_) => todo!(),
            //     // KeybindAction::AdjustFov(_) => todo!(),
            //     // KeybindAction::SetGameSpeed(_) => todo!(),
            //     // KeybindAction::AdjustGamespeed(_) => todo!(),
            //     // KeybindAction::ToggleDebugPause => todo!(),
            //     // KeybindAction::ToggleFreecam => todo!(),
            // }
        }
    }
}
