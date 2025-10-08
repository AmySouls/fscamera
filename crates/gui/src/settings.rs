use std::{fs, path::PathBuf};

use directories::ProjectDirs;
use protocol::{Keybind, SettingsData};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    let Some(path) = ProjectDirs::from("nl", "vswarte", "FromSoftware Camera Tool")
        .map(|d| d.config_dir().join("settings.json"))
    else {
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
            apply_gamespeed_only_when_playback_mode_active: self
                .apply_gamespeed_only_when_playback_mode_active,
            enabling_playback_disables_freecam: self.enabling_playback_disables_freecam,
            playback_start_restarts_path: self.enabling_playback_disables_freecam,
            keybinds: self.keybinds,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsDataV1 {
    pub apply_gamespeed_only_when_playback_mode_active: bool,
    pub enabling_playback_disables_freecam: bool,
    pub playback_start_restarts_path: bool,
    pub keybinds: Vec<Keybind>,
}
