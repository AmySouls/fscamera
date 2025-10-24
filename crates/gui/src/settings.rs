use std::{fs, path::PathBuf};

use directories::ProjectDirs;
use eframe::egui::{Context, DragValue, Grid, Key, ScrollArea, Ui};
use protocol::{SettingsData, keybind::{KeybindInput, KeybindMapping}};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::controls::{labeled_control, panel_header};

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("Could not figure out where to save your settings file.")]
    NoConfigPath,
    #[error("JSON error")]
    Json,
    #[error("IO error {0}")]
    Io(#[from] std::io::Error),
}

fn get_settings_path() -> Option<PathBuf> {
    ProjectDirs::from("nl", "vswarte", "FromSoftware Camera Tool")
        .map(|d| d.config_dir().join("settings.json"))
}

pub(crate) fn get_settings() -> Result<SettingsData, SettingsError> {
    let Some(path) = get_settings_path() else {
        return Err(SettingsError::NoConfigPath);
    };

    // No saved data yet results in default being loaded
    if !fs::exists(&path)? {
        return Ok(SettingsData::default());
    }

    let data = fs::read_to_string(path)?;
    let Ok(deserialized) = serde_json::from_str::<SettingsFormat>(&data) else {
        return Err(SettingsError::Json);
    };

    Ok(deserialized.into())
}

pub(crate) fn save_settings(data: &SettingsData) -> Result<(), SettingsError> {
    let Some(path) = get_settings_path() else {
        return Err(SettingsError::NoConfigPath);
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let wrapped = SettingsFormat::V2(data.clone());

    let Ok(data) = serde_json::to_string_pretty(&wrapped) else {
        return Err(SettingsError::Json);
    };

    fs::write(path, data)?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub enum SettingsFormat {
    V1(SettingsDataV1),
    V2(SettingsData),
}

impl Into<SettingsData> for SettingsFormat {
    fn into(self) -> SettingsData {
        match self {
            SettingsFormat::V1(v) => v.into(),
            SettingsFormat::V2(v) => v,
        }
    }
}

// Migrate from old settings format to new
impl Into<SettingsData> for SettingsDataV1 {
    fn into(self) -> SettingsData {
        SettingsData {
            path_duration: 90.0,
            time_between_created_keyframes: 10.0,
            keybinds: KeybindMapping::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsDataV1 {
    pub apply_gamespeed_only_when_playback_mode_active: bool,
    pub enabling_playback_disables_freecam: bool,
    pub playback_start_restarts_path: bool,
    pub keybinds: KeybindMapping,
}

pub struct SettingsControl {
    data: SettingsData,
    keybind_listening: Option<ListeningKeybind>,
}

impl SettingsControl {
    pub fn new(data: SettingsData) -> Self {
        Self {
            data,
            keybind_listening: None,
        }
    }

    pub fn data(&self) -> &SettingsData {
        &self.data
    }

    pub fn update(&mut self, ui: &mut Ui, ctx: &Context) {
        self.keyframe_controls(ui);
        self.keybind_controls(ui, ctx);
    }

    fn keyframe_controls(&mut self, ui: &mut Ui) {
        panel_header(ui, "Keyframes");

        ui.horizontal(|ui| {
            labeled_control(ui, "Time between created keyframes", |ui| {
                ui.add(
                    DragValue::new(&mut self.data.time_between_created_keyframes)
                        .speed(1.0)
                        .suffix("s")
                        .range(0.00..=30.0),
                );
            });

            labeled_control(ui, "Path duration", |ui| {
                ui.add(
                    DragValue::new(&mut self.data.path_duration)
                        .speed(1.0)
                        .suffix("s")
                        .range(30.00..=240.0),
                );
            });
        });
    }

    fn keybind_controls(&mut self, ui: &mut Ui, ctx: &Context) {
        panel_header(ui, "Keybinds");

        ScrollArea::vertical().show(ui, |ui| {
            Grid::new("settings_keybinds")
                .num_columns(3)
                .spacing([40.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    Self::keybind_column(
                        ui,
                        "Create keyframe",
                        &mut self.data.keybinds.create_keyframe,
                        &mut self.keybind_listening,
                        ListeningKeybind::CreateKeyframe,
                    );

                    Self::keybind_column(
                        ui,
                        "Play camera path",
                        &mut self.data.keybinds.play_path,
                        &mut self.keybind_listening,
                        ListeningKeybind::PlayPath,
                    );

                    Self::keybind_column(
                        ui,
                        "Toggle freecam",
                        &mut self.data.keybinds.toggle_freecam,
                        &mut self.keybind_listening,
                        ListeningKeybind::ToggleFreecam,
                    );

                    Self::keybind_column(
                        ui,
                        "Lock freecam position and rotation",
                        &mut self.data.keybinds.toggle_freecam_lock,
                        &mut self.keybind_listening,
                        ListeningKeybind::ToggleFreecamLock,
                    );

                    Self::keybind_column(
                        ui,
                        "Toggle HUD",
                        &mut self.data.keybinds.toggle_hud,
                        &mut self.keybind_listening,
                        ListeningKeybind::ToggleHud,
                    );

                    Self::keybind_column(
                        ui,
                        "Toggle game pause",
                        &mut self.data.keybinds.toggle_debug_pause,
                        &mut self.keybind_listening,
                        ListeningKeybind::ToggleDebugPause,
                    );

                    Self::keybind_column(
                        ui,
                        "Toggle game speed",
                        &mut self.data.keybinds.toggle_game_speed,
                        &mut self.keybind_listening,
                        ListeningKeybind::ToggleGameSpeed,
                    );

                    Self::keybind_column(
                        ui,
                        "Increase FoV",
                        &mut self.data.keybinds.increase_fov,
                        &mut self.keybind_listening,
                        ListeningKeybind::IncreaseFov,
                    );

                    Self::keybind_column(
                        ui,
                        "Decrease FoV",
                        &mut self.data.keybinds.decrease_fov,
                        &mut self.keybind_listening,
                        ListeningKeybind::DecreaseFov,
                    );
                });
        });


        if let Some(ref action) = self.keybind_listening.clone() {
            let mut caught_input = None;

            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.keybind_listening = None;
            }

            for key in Key::ALL {
                if ctx.input(|i| i.key_pressed(*key))
                    && let Some(key) = egui_key_to_vk(*key)
                {
                    caught_input = Some(KeybindInput::Keyboard(key));
                }
            }

            if caught_input.is_none() {
                let dy = ctx.input(|i| i.smooth_scroll_delta.y);
                if dy > 0.0 {
                    caught_input = Some(KeybindInput::ScrollUp);
                } else if dy < 0.0 {
                    caught_input = Some(KeybindInput::ScrollDown);
                }
            }

            if let Some(input) = caught_input {
                match action {
                    ListeningKeybind::CreateKeyframe => self.data.keybinds.create_keyframe = Some(input),
                    ListeningKeybind::PlayPath => self.data.keybinds.play_path = Some(input),
                    ListeningKeybind::ToggleFreecam => self.data.keybinds.toggle_freecam = Some(input),
                    ListeningKeybind::ToggleHud => self.data.keybinds.toggle_hud = Some(input),
                    ListeningKeybind::ToggleDebugPause => self.data.keybinds.toggle_debug_pause = Some(input),
                    ListeningKeybind::ToggleGameSpeed => self.data.keybinds.toggle_game_speed = Some(input),
                    ListeningKeybind::IncreaseFov => self.data.keybinds.increase_fov = Some(input),
                    ListeningKeybind::DecreaseFov => self.data.keybinds.decrease_fov = Some(input),
                    ListeningKeybind::ToggleFreecamLock => self.data.keybinds.toggle_freecam_lock = Some(input),
                }

                self.keybind_listening = None;
            }
        }
    }

    fn keybind_column(
        ui: &mut Ui,
        label: &str,
        v: &mut Option<KeybindInput>,
        listening: &mut Option<ListeningKeybind>,
        keybind: ListeningKeybind,
    ) {
        ui.label(label);

        match draw_keybind_control(ui, v, listening.as_ref() == Some(&keybind)) {
            None => {}
            Some(KeybindRequest::StartBinding) => *listening = Some(keybind),
            Some(KeybindRequest::Remove) => *v = None,
        }

        ui.end_row();
    }
}

fn draw_keybind_control(
    ui: &mut Ui,
    v: &mut Option<KeybindInput>,
    listening: bool,
) -> Option<KeybindRequest> {
    let label = match (listening, v) {
        (true, _) => "Listening for input",
        (false, None) => "Unbound",
        (false, Some(v)) => format_input(v),
    };

    if ui.button(label).clicked() {
        return Some(KeybindRequest::StartBinding);
    }

    if ui.button("❌").clicked() {
        return Some(KeybindRequest::Remove);
    }

    None
}

enum KeybindRequest {
    Remove,
    StartBinding,
}

#[derive(Clone, PartialEq)]
enum ListeningKeybind {
    CreateKeyframe,
    PlayPath,
    ToggleFreecam,
    ToggleHud,
    ToggleDebugPause,
    ToggleGameSpeed,
    IncreaseFov,
    DecreaseFov,
    ToggleFreecamLock,
}

fn format_input(input: &KeybindInput) -> &str {
    match input {
        KeybindInput::ScrollDown => "Scroll down",
        KeybindInput::ScrollUp => "Scroll up",
        KeybindInput::Keyboard(vk) => match vk {
            0x41 => "A",
            0x42 => "B",
            0x43 => "C",
            0x44 => "D",
            0x45 => "E",
            0x46 => "F",
            0x47 => "G",
            0x48 => "H",
            0x49 => "I",
            0x4A => "J",
            0x4B => "K",
            0x4C => "L",
            0x4D => "M",
            0x4E => "N",
            0x4F => "O",
            0x50 => "P",
            0x51 => "Q",
            0x52 => "R",
            0x53 => "S",
            0x54 => "T",
            0x55 => "U",
            0x56 => "V",
            0x57 => "W",
            0x58 => "X",
            0x59 => "Y",
            0x5A => "Z",

            0x60 => "Num0",
            0x61 => "Num1",
            0x62 => "Num2",
            0x63 => "Num3",
            0x64 => "Num4",
            0x65 => "Num5",
            0x66 => "Num6",
            0x67 => "Num7",
            0x68 => "Num8",
            0x69 => "Num9",

            0x1B => "Escape",
            0x0D => "Enter",
            0x09 => "Tab",
            0x08 => "Backspace",
            0x2D => "Insert",
            0x2E => "Delete",
            0x24 => "Home",
            0x23 => "End",
            0x21 => "PageUp",
            0x22 => "PageDown",
            0x25 => "ArrowLeft",
            0x26 => "ArrowUp",
            0x27 => "ArrowRight",
            0x28 => "ArrowDown",
            0x20 => "Space",

            0x70 => "F1",
            0x71 => "F2",
            0x72 => "F3",
            0x73 => "F4",
            0x74 => "F5",
            0x75 => "F6",
            0x76 => "F7",
            0x77 => "F8",
            0x78 => "F9",
            0x79 => "F10",
            0x7A => "F11",
            0x7B => "F12",
            0x7C => "F13",
            0x7D => "F14",
            0x7E => "F15",
            0x7F => "F16",
            0x80 => "F17",
            0x81 => "F18",
            0x82 => "F19",
            0x83 => "F20",
            0x84 => "F21",
            0x85 => "F22",
            0x86 => "F23",
            0x87 => "F24",

            _ => "Unknown",
        },
    }
}

fn egui_key_to_vk(key: eframe::egui::Key) -> Option<i32> {
    Some(match key {
        // Ordinary keys
        Key::A => 0x41,
        Key::B => 0x42,
        Key::C => 0x43,
        Key::D => 0x44,
        Key::E => 0x45,
        Key::F => 0x46,
        Key::G => 0x47,
        Key::H => 0x48,
        Key::I => 0x49,
        Key::J => 0x4A,
        Key::K => 0x4B,
        Key::L => 0x4C,
        Key::M => 0x4D,
        Key::N => 0x4E,
        Key::O => 0x4F,
        Key::P => 0x50,
        Key::Q => 0x51,
        Key::R => 0x52,
        Key::S => 0x53,
        Key::T => 0x54,
        Key::U => 0x55,
        Key::V => 0x56,
        Key::W => 0x57,
        Key::X => 0x58,
        Key::Y => 0x59,
        Key::Z => 0x5A,

        // Numpad shit
        Key::Num0 => 0x60,
        Key::Num1 => 0x61,
        Key::Num2 => 0x62,
        Key::Num3 => 0x63,
        Key::Num4 => 0x64,
        Key::Num5 => 0x65,
        Key::Num6 => 0x66,
        Key::Num7 => 0x67,
        Key::Num8 => 0x68,
        Key::Num9 => 0x69,

        Key::Enter => 0x0D,
        Key::Tab => 0x09,
        Key::Backspace => 0x08,
        Key::Insert => 0x2D,
        Key::Delete => 0x2E,
        Key::Home => 0x24,
        Key::End => 0x23,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::ArrowLeft => 0x25,
        Key::ArrowUp => 0x26,
        Key::ArrowRight => 0x27,
        Key::ArrowDown => 0x28,
        Key::Space => 0x20,

        // Function keys
        Key::F1 => 0x70,
        Key::F2 => 0x71,
        Key::F3 => 0x72,
        Key::F4 => 0x73,
        Key::F5 => 0x74,
        Key::F6 => 0x75,
        Key::F7 => 0x76,
        Key::F8 => 0x77,
        Key::F9 => 0x78,
        Key::F10 => 0x79,
        Key::F11 => 0x7A,
        Key::F12 => 0x7B,
        Key::F13 => 0x7C,
        Key::F14 => 0x7D,
        Key::F15 => 0x7E,
        Key::F16 => 0x7F,
        Key::F17 => 0x80,
        Key::F18 => 0x81,
        Key::F19 => 0x82,
        Key::F20 => 0x83,
        Key::F21 => 0x84,
        Key::F22 => 0x85,
        Key::F23 => 0x86,
        Key::F24 => 0x87,

        _ => return None,
    })
}
