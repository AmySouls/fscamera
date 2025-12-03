use eframe::egui::{Ui};
use egui_notify::Toasts;

use crate::{controls::{drag_fov, drag_percentage, drag_percentage_modifier, labeled_control, panel_header}, game::RemoteGame};

pub struct FreeCamControl {
    locked: bool,
    movement_speed: f32,
    rotation_speed: f32,
    slow_modifier: f32,
    fast_modifier: f32,
    fov: f32,
}

impl Default for FreeCamControl {
    fn default() -> Self {
        Self {
            locked: false,
            movement_speed: 1.0,
            rotation_speed: 1.0,
            slow_modifier: 0.25,
            fast_modifier: 4.0,
            fov: 48.0f32.to_radians(),
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
                labeled_control(ui, "Field of view", |ui| {
                    if drag_fov(
                        ui,
                        "",
                        &mut self.fov,
                    )
                        .changed()
                            && let Err(e) = remote.set_freecam_fov(self.fov) {
                                println!("Changed fov");
                            notify.error(format!("Could not change freecam fov: {e}"));
                    }

                });

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
                    if drag_percentage(ui, "", &mut self.rotation_speed, true).changed()
                        && let Err(e) = remote.set_freecam_rotation_speed(self.rotation_speed)
                    {
                        notify.error(format!("Could not change freecam rotation speed: {e}"));
                    }
                });
            });
        });

        ui.horizontal(|ui| {
            labeled_control(ui, "Slow modifier speed", |ui| {
                if drag_percentage_modifier(
                    ui,
                    "",
                    &mut self.slow_modifier,
                    0.01..=1.0
                )
                    .changed()
                        && let Err(e) = remote.set_freecam_speed_modifiers(self.slow_modifier, self.fast_modifier) {
                            notify.error(format!("Could not change freecam movement speed: {e}"));
                }

            });

            labeled_control(ui, "Fast modifier speed", |ui| {
                if drag_percentage_modifier(
                    ui,
                    "",
                    &mut self.fast_modifier,
                    1.00..=8.0
                )
                    .changed()
                        && let Err(e) = remote.set_freecam_speed_modifiers(self.slow_modifier, self.fast_modifier) {
                            notify.error(format!("Could not change freecam movement speed: {e}"));
                }

            });
        });
    }

    pub fn toggle_locked(&mut self) {
        self.locked = !self.locked;
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    pub fn set_fov(&mut self, fov: &f32) {
        self.fov = *fov;
    }
}
