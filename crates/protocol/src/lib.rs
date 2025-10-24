use keyframe::{Keyframe, Quat, Vec3};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::keybind::KeybindMapping;

pub mod keyframe;
pub mod keybind;

#[derive(PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CameraMode {
    /// Game is in full control of the camera.
    Game,
    /// Freecam mode is active.
    Freecam,
    /// Camera is in playback mode on a specific path.
    Playback,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum InboundGameControlEvent {
    Initialize { settings: SettingsData },
    Settings { settings: SettingsData },
    SetFreecamMovementSpeed { value: f32 },
    SetFreecamRotationSpeed { value: f32 },

    SetCameraMode { mode: CameraMode },
    SetFreecamLocked { locked: bool },

    SetFreecamFov { fov: f32 },
    SetHudDisabled { disabled: bool },
    SetDebugPauseEnabled { enabled: bool },

    SetTimeOfDay { hours: u8, minutes: u8, seconds: u8 },
    SetCharacterNoDead { enabled: bool },
    SetCharacterNoMove { enabled: bool },
    SetGameSpeedMultiplier { value: f32 },
    SetGameSpeedMultiplierEnabled { enabled: bool },

    SetKeyframes { keyframes: Vec<Keyframe> },
    SetPlaybackState { playing: bool, time: f32 },

    // Play { time: f32, keyframes: Vec<Keyframe> },
    // Pause { time: f32 },
    // Scrub { time: f32 },
    //
    // PlaybackModeState { state: bool },
    // TimeMultiplier { multiplier: f32 },
    // GlobalFov { fov: f32, enabled: bool },
    // SetHudDisabled { disabled: bool },
    // SetDebugPause { enabled: bool },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OutboundGameControlEvent {
    CreateKeyframe,
    PlayPath,
    ToggleFreecam,
    ToggleFreecamLock,
    ToggleHud,
    ToggleDebugPause,
    ToggleGameSpeed,
    IncreaseFov,
    DecreaseFov,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CameraState {
    pub map_id: i32,
    pub position: Vec3,
    pub orientation: Quat,
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
    // pub apply_gamespeed_only_when_playback_mode_active: bool,
    // pub enabling_playback_disables_freecam: bool,
    // pub playback_start_restarts_path: bool,

    pub path_duration: f32,
    pub time_between_created_keyframes: f32,
    pub keybinds: KeybindMapping,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            path_duration: 90.0,
            time_between_created_keyframes: 10.0,
            keybinds: KeybindMapping::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsDataKeybind {
    pub key: u32,
}
