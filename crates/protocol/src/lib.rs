use keyframe::{Keyframe, Quat, Vec3};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::keyframe::Orientation;

pub mod keyframe;

#[derive(Debug, Serialize, Deserialize)]
pub enum InboundGameControlEvent {
    Initialize { settings: SettingsData },
    Settings { settings: SettingsData },
    Keyframes { keyframes: Vec<Keyframe> },
    Play { time: f32, keyframes: Vec<Keyframe> },
    Pause { time: f32 },
    Scrub { time: f32 },
    PlaybackModeState { state: bool },
    TimeMultiplier { multiplier: f32 },
    GlobalFov { fov: f32, enabled: bool },
    RequestTimeOfDay { hours: u8, minutes: u8, seconds: u8 },
    HudState { hidden: bool },
    SetCharacterNoDead { value: bool },
    SetCharacterNoMove { value: bool },
    SetFreecamMovementSpeed { value: f32 },
    SetFreecamRotationSpeed { value: f32 },
    SetDebugPause { enabled: bool },
    SetFreecamEnabled { enabled: bool },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum OutboundGameControlEvent {
    KeybindAction(KeybindAction),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CameraState {
    pub map_id: i32,
    pub position: Vec3,
    pub orientation: Orientation,
    pub fov: f32,
}

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum RemoteError {
    #[error("Game or version of game is not supported.")]
    UnknownGame,
    #[error("Could not acquire a process handle")]
    AcquireProcess,
    #[error("Could not inject agent DLL into game")]
    AcquireAgent,
    #[error("Could not locate remote procudure \"{0}\"")]
    AcquireProcedure(String),
    #[error("Could not obtain an instance of CSCamera, are you loaded into a map?")]
    AcquireCSCamera,
    #[error("Could not obtain an instance of WorldAreaTime, are you loaded into a map?")]
    AcquireWorldAreaTime,
    #[error("Could not obtain an instance of FieldArea, are you loaded into a map?")]
    AcquireFieldArea,
    #[error("Could not obtain an instance of WorldBlockInfo, are you loaded into a map?")]
    AcquireWorldBlockInfo,
    #[error("Could not obtain an instance of WorldChrMan, are you loaded into a map?")]
    AcquireWorldChrMan,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsData {
    pub apply_gamespeed_only_when_playback_mode_active: bool,
    pub enabling_playback_disables_freecam: bool,
    pub playback_start_restarts_path: bool,
    pub keybinds: Vec<Keybind>,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            apply_gamespeed_only_when_playback_mode_active: Default::default(),
            enabling_playback_disables_freecam: Default::default(),
            playback_start_restarts_path: Default::default(),
            keybinds: vec![
                Keybind {
                    action: KeybindAction::TogglePlaybackMode,
                    active: true,
                    input: Some(KeybindInput::Keyboard(0x73)), // F4
                },
                Keybind {
                    action: KeybindAction::ToggleFreecam,
                    active: true,
                    input: Some(KeybindInput::Keyboard(0x78)), // F9
                },
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum KeybindInput {
    Keyboard(i32),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keybind {
    pub action: KeybindAction,
    pub active: bool,
    pub input: Option<KeybindInput>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum KeybindAction {
    TogglePlaybackMode,
    CreateKeyframe,
    ToggleHUD,
    ToggleCharacterNoDead,
    ToggleCharacterNoMove,
    ToggleFovOverride,
    SetFov(f32),
    AdjustFov(f32),
    SetGameSpeed(f32),
    AdjustGamespeed(f32),
    ToggleDebugPause,
    ToggleFreecam,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsDataKeybind {
    pub key: u32,
    pub action: KeybindAction,
}
