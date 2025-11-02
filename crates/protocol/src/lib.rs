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
    SetCameraMode { mode: CameraMode },
    UpdateFreecamFov { fov: f32 },
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

// TODO: alternative gamespeed for playback mode
// TODO: turn off debug pause when playback mode starts
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsData {
    pub time_between_created_keyframes: f32,
    pub keybinds: KeybindMapping,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            time_between_created_keyframes: 10.0,
            keybinds: KeybindMapping::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsDataKeybind {
    pub key: u32,
}
