use eframe::egui::{Slider, Ui};
use egui_notify::Toasts;

use crate::game::RemoteGame;
use crate::controls::{drag_percentage, labeled_control, panel_header};

pub struct WorldControl {
    character_no_dead: bool,
    character_no_move: bool,
    gamespeed_enabled: bool,
    gamespeed_multiplier: f32,
    time_of_day: u32,
}

impl Default for WorldControl {
    fn default() -> Self {
        Self {
            character_no_dead: false,
            character_no_move: false,
            gamespeed_enabled: false,
            gamespeed_multiplier: 1.0f32,
            time_of_day: 6 * 60,
        }
    }
}

impl WorldControl {
    pub fn update(&mut self, ui: &mut Ui, remote: &RemoteGame, notify: &mut Toasts) {
        panel_header(ui, "World control");

        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut self.gamespeed_enabled, "Gamespeed enabled")
                    .changed()
                    && let Err(e) = remote.set_gamespeed_multiplier_enabled(self.gamespeed_enabled)
            {
                notify.error(format!("Could not enable gamespeed override: {e}"));
            }
        });

        ui.horizontal(|ui| {
            labeled_control(ui, "Game speed", |ui| {
                if drag_percentage(ui, "", &mut self.gamespeed_multiplier, true).changed()
                    && let Err(e) = remote.set_gamespeed_multiplier(self.gamespeed_multiplier) {
                    notify.error(format!("Could not change game speed: {e}"));
                }
            });

            labeled_control(ui, "Time of day", |ui| {
                // Convert minutes to hh:mm string
                let hours = self.time_of_day / 60;
                let mins = self.time_of_day % 60;
                let label = format!("{:02}:{:02}", hours, mins);

                if ui.add(
                    Slider::new(&mut self.time_of_day, 0..=(23 * 60 + 59))
                        .show_value(false)
                        .text(label)
                        .step_by(1.0),
                ).changed() && let Err(e) = remote.set_time_of_day(
                    hours as u8,
                    mins as u8,
                    0
                ) {
                    notify.error(format!("Could not set time of day: {e}"));
                }
            });
        });

        ui.spacing();

        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut self.character_no_dead, "Disable character death")
                    .changed()
                    && let Err(e) = remote.set_character_no_dead(self.character_no_dead)
            {
                notify.error(format!("Could not enable no dead: {e}"));
            }

            if ui
                .checkbox(&mut self.character_no_move, "Disable character movement")
                    .changed()
                    && let Err(e) = remote.set_character_no_move(self.character_no_move)
            {
                notify.error(format!("Could not enable no move: {e}"));
            }
        });
    }

    pub fn set_gamespeed_multiplier(&mut self, value: f32) {
        self.gamespeed_multiplier = value;
    }

    pub fn gamespeed_enabled(&self) -> bool {
        self.gamespeed_enabled
    }

    pub fn toggle_gamespeed_enabled(&mut self) {
        self.gamespeed_enabled = !self.gamespeed_enabled;
    }
}
