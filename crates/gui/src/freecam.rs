use eframe::egui::{Ui};
use egui_notify::Toasts;

use crate::{controls::{drag_percentage, labeled_control, panel_header}, game::RemoteGame};

pub struct FreeCamControl {
    locked: bool,
    movement_speed: f32,
    rotation_speed: f32,
}

impl Default for FreeCamControl {
    fn default() -> Self {
        Self {
            locked: false,
            movement_speed: 1.0,
            rotation_speed: 1.0,
        }
    }
}

impl FreeCamControl {
    pub fn update(&mut self, ui: &mut Ui, remote: &RemoteGame, notify: &mut Toasts) {
        panel_header(ui, "Freecam");

        ui.horizontal(|ui| {
            labeled_control(ui, "", |ui| {
                if ui.checkbox(&mut self.locked, "Locked").changed()
                    && let Err(e) = remote.set_freecam_locked(self.locked) {
                        notify.error(format!("Could not change freecam movement speed: {e}"));
                }
            });

            ui.horizontal(|ui| {
                labeled_control(ui, "Movement speed", |ui| {
                    if drag_percentage(
                        ui,
                        "",
                        &mut self.movement_speed,
                        true,
                    )
                        .changed()
                            && let Err(e) = remote.set_freecam_movement_speed(self.movement_speed) {
                                notify.error(format!("Could not change freecam movement speed: {e}"));
                    }

                });

                labeled_control(ui, "Rotation speed", |ui| {
                    if drag_percentage(
                        ui,
                        "",
                        &mut self.rotation_speed,
                        true,
                    )
                        .changed()
                            && let Err(e) = remote.set_freecam_rotation_speed(self.rotation_speed)
                    {
                        notify.error(format!("Could not change freecam rotation speed: {e}"));
                    }
                });
            });
        });
    }

    pub fn toggle_locked(&mut self) {
        self.locked = !self.locked;
    }

    pub fn locked(&self) -> bool {
        self.locked
    }
}
