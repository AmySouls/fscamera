use eframe::egui::Ui;
use egui_notify::Toasts;

use protocol::PlaybackSettingsData;
use crate::game::RemoteGame;
use crate::controls::{drag_percentage, labeled_control, panel_header};

pub struct PlaybackControl {
    gamespeed_enabled_on_playback: bool,
    gamespeed_multiplier: f32,
    unpause_on_playback: bool,
}

impl Default for PlaybackControl {
    fn default() -> Self {
        Self {
            gamespeed_enabled_on_playback: false,
            gamespeed_multiplier: 1.0f32,
            unpause_on_playback: false,
        }
    }
}

impl PlaybackControl {
    pub fn update(&mut self, ui: &mut Ui, remote: &RemoteGame, notify: &mut Toasts) {
        let mut changed = false;

        panel_header(ui, "Playback Control");

        ui.horizontal(|ui| {
            changed |= ui.checkbox(&mut self.gamespeed_enabled_on_playback, "Override gamespeed during playback").changed();
            changed |= ui.checkbox(&mut self.unpause_on_playback, "Unpause on play").changed();
        });

        ui.horizontal(|ui| {
            labeled_control(ui, "Playback gamespeed", |ui| {
                changed |= drag_percentage(ui, "", &mut self.gamespeed_multiplier, true).changed();
            });
        });

        if changed && let Err(e) = remote.set_playback_settings(PlaybackSettingsData {
            gamespeed_enabled_on_playback: self.gamespeed_enabled_on_playback,
            gamespeed_multiplier: self.gamespeed_multiplier,
            unpause_on_playback: self.unpause_on_playback,
        }) {
            notify.error(format!("Could not set playback settings: {e}"));
        }
    }
}
