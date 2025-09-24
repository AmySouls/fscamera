use std::{fs, path::PathBuf};

use directories::ProjectDirs;
use protocol::SettingsData;
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

    Ok(deserialized.to_current())
}

pub(crate) fn save_settings(data: &SettingsData) -> Result<(), SettingsError> {
    let Some(path) = ProjectDirs::from("nl", "vswarte", "FromSoftware Camera Tool")
        .map(|d| d.config_dir().join("settings.json")) else {
        return Err(SettingsError::NoConfigPath);
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let wrapped = SettingsFormat::V1(data.clone());

    let Ok(data) = serde_json::to_string_pretty(&wrapped) else {
        return Err(SettingsError::Json);
    };

    fs::write(path, data)?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub enum SettingsFormat {
    V1(SettingsData),
}

impl SettingsFormat {
    fn to_current(&self) -> SettingsData {
        match self {
            SettingsFormat::V1(data) => data.clone(),
        }
    }
}
