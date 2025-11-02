use crate::freecam::FreeCamControl;
use crate::game::RemoteGame;
use crate::process::{self, GameProcess};
use crate::program_title;
use crate::settings::{SettingsControl, get_settings, save_settings};
use crate::timeline::{TimelineControl, TimelineControlCommand};
use crate::world::WorldControl;
use eframe::egui::{
    self, Align, CentralPanel, ComboBox, Context, Layout, TopBottomPanel, Visuals
};
use egui_notify::Toasts;
use protocol::{CameraMode, OutboundGameControlEvent};

pub(crate) struct CameraControlApp {
    notify: Toasts,

    process: Option<GameProcess>,
    remote: Option<RemoteGame>,

    settings_open: bool,

    camera_mode: CameraMode,
    hud_disabled: bool,
    debug_pause_enabled: bool,

    settings_control: SettingsControl,
    freecam_control: FreeCamControl,
    world_control: WorldControl,
    timeline_control: TimelineControl,
}

impl Default for CameraControlApp {
    fn default() -> Self {
        let settings = get_settings()
            .expect("Could not load settings");

        Self {
            notify: Toasts::default(),
            freecam_control: FreeCamControl::default(),
            world_control: WorldControl::default(),
            timeline_control: TimelineControl::new(),
            settings_control: SettingsControl::new(settings),

            process: None,
            remote: None,
            settings_open: false,

            camera_mode: CameraMode::Game,
            hud_disabled: false,
            debug_pause_enabled: false,
        }
    }
}

impl eframe::App for CameraControlApp {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        ctx.set_visuals(Visuals::dark());

        if self.settings_open {
            self.update_settings(ctx, frame);
        } else if self.remote.is_some() {
            self.update_main(ctx);
        } else {
            self.update_game_selector(ctx, frame);
        }

        let mut events = vec![];
        if let Some(remote) = self.remote.as_mut() {
            while let Some(event) = remote.receive_event() {
                events.push(event);
            }
        }

        for event in events {
            self.handle_game_event(&event);
        }

        ctx.request_repaint();
        self.notify.show(ctx);
    }
}

impl CameraControlApp {
    fn handle_game_event(&mut self, event: &OutboundGameControlEvent) {
        let Some(remote) = self.remote.as_ref() else {
            return;
        };

        match event {
            OutboundGameControlEvent::CreateKeyframe => {
                if let Err(e) = self.timeline_control.create_keyframe(remote, self.settings_control.data()) {
                    self.notify.error(format!("Could not create keyframe: {e}"));
                }

                if let Err(e) = remote.set_keyframes(self.timeline_control.keyframes()) {
                    self.notify.error(format!("Could not push keyframes to game: {e}"));
                }
            },
            OutboundGameControlEvent::PlayPath => {
                self.camera_mode = CameraMode::Playback;
                self.timeline_control.play();

                if let Err(e) = remote.set_camera_mode(self.camera_mode) {
                    self.notify.error(format!("Could not toggle freecam: {e}"));
                }

                if let Err(e) = remote.set_playback_state(true, self.timeline_control.time()) {
                    self.notify.error(format!("Could not set remote playback state: {e}"));
                }
            },
            OutboundGameControlEvent::ToggleFreecam => {
                if self.camera_mode == CameraMode::Freecam {
                    self.camera_mode = CameraMode::Game;
                } else {
                    self.camera_mode = CameraMode::Freecam;
                }

                self.timeline_control.stop();

                if let Err(e) = remote.set_camera_mode(self.camera_mode) {
                    self.notify.error(format!("Could not toggle freecam: {e}"));
                }
            },
            OutboundGameControlEvent::ToggleFreecamLock => {
                self.freecam_control.toggle_locked();
                if let Err(e) = remote.set_freecam_locked(self.freecam_control.locked()) {
                    self.notify.error(format!("Could not toggle freecam lock: {e}"));
                }
            },
            OutboundGameControlEvent::ToggleHud => {
                self.hud_disabled = !self.hud_disabled;
                if let Err(e) = remote.set_hud_disabled(self.hud_disabled) {
                    self.notify.error(format!("Could not toggle hud: {e}"));
                }
            },
            OutboundGameControlEvent::ToggleDebugPause => {
                self.debug_pause_enabled = !self.debug_pause_enabled;
                if let Err(e) = remote.set_debug_pause_enabled(self.debug_pause_enabled) {
                    self.notify.error(format!("Could not toggle debug pause: {e}"));
                }
            },
            OutboundGameControlEvent::ToggleGameSpeed => {
                self.world_control.toggle_gamespeed_enabled();
                if let Err(e) = remote.set_gamespeed_multiplier_enabled(self.world_control.gamespeed_enabled()) {
                    self.notify.error(format!("Could not game speed multiplier: {e}"));
                }
            },
            OutboundGameControlEvent::SetCameraMode { mode } => {
                self.camera_mode = *mode;
            },
            OutboundGameControlEvent::UpdateFreecamFov { fov } => {
                self.freecam_control.set_fov(fov);
            },
        }
    }

    fn update_game_selector(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ui.heading(program_title());

            ComboBox::from_label("Game process")
                .selected_text(
                    self.process
                        .as_ref()
                        .map(|p| p.to_string())
                        .unwrap_or(String::from("No process selected")),
                )
                .show_ui(ui, |ui| {
                    process::get_running_games()
                        .iter()
                        .for_each(|e| {
                            ui.selectable_value(&mut self.process, Some(e.clone()), e.to_string());
                        })
                });

            ui.add_enabled_ui(self.process.is_some(), |ui| {
                if ui.button("Attach to game").clicked()
                    && let Some(process) = self.process.as_ref() {

                    let Ok(remote) = RemoteGame::connect(process) else {
                        self.notify.error("Failed attaching to game.");
                        return;
                    };

                    if remote.initialize(self.settings_control.data()).is_err() {
                        self.notify.error("Failed initializing game agent.");
                        return;
                    };

                    self.remote = Some(remote);
                    self.notify.success("Succesfully attached to game");
                }
            });

            if ui.button("⚙ Settings").clicked() {
                self.settings_open = true;
            }
        });
    }

    fn update_settings(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut flush_settings = false;
        TopBottomPanel::top("top_controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(program_title());

                ui.separator();

                if ui.button("💾 Save and apply").clicked() {
                    flush_settings = true;
                }

                ui.with_layout(
                    Layout::default().with_cross_align(Align::RIGHT),
                    |ui| {
                        if ui.button("⬅ Back").clicked() {
                            self.settings_open = false;
                        }
                    },
                );
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            self.settings_control.update(ui, ctx);
        });

        if let Some(remote) = self.remote.as_ref() && flush_settings {
            if let Err(e) = save_settings(self.settings_control.data()) {
                self.notify.error(format!("Could not save settings: {e}"));
            }

            if let Err(e) = remote.set_settings(self.settings_control.data().clone()) {
                self.notify.error(format!("Could not pass settings to game: {e}"));
            }
        }
    }

    fn update_main(&mut self, ctx: &egui::Context) {
        // Safety: should be fine since we check if there is a remote at all
        let remote = self.remote.as_ref().expect("remote was None");

        TopBottomPanel::top("top_controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(program_title());

                ui.separator();

                let prev_camera_mode = self.camera_mode;
                ComboBox::from_label("")
                    .selected_text(match self.camera_mode {
                        CameraMode::Game => "Disabled",
                        CameraMode::Freecam => "Freecam",
                        CameraMode::Playback => "Playback",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.camera_mode,
                            CameraMode::Game,
                            "Disabled",
                        );
                        ui.selectable_value(
                            &mut self.camera_mode,
                            CameraMode::Freecam,
                            "Freecam",
                        );
                        ui.selectable_value(
                            &mut self.camera_mode,
                            CameraMode::Playback,
                            "Playback",
                        );
                    });

                if prev_camera_mode != self.camera_mode 
                    && let Err(e) = remote.set_camera_mode(self.camera_mode) {
                    self.notify.error(format!("Could not set camera mode: {e}"));
                }

                if !self.timeline_control.playing() {
                    if ui.button("Play ▶").clicked() {
                        if let Err(e) = remote.set_camera_mode(self.camera_mode) {
                            self.notify.error(format!("Could not start playing path: {e}"));
                            return;
                        }

                        if let Err(e) = remote.set_playback_state(true, 0.0) {
                            self.notify.error(format!("Could not start playing path: {e}"));
                            return;
                        }

                        self.camera_mode = CameraMode::Playback;
                        self.timeline_control.play();
                    }
                } else if ui.button("Stop ⏹️").clicked() {
                    if let Err(e) = remote.set_playback_state(
                        false,
                        self.timeline_control.time(),
                    ) {
                        self.notify.error(format!("Could not pause path: {e}"));
                        return;
                    }

                    self.camera_mode = CameraMode::Playback;
                    self.timeline_control.stop();
                }

                ui.with_layout(
                    Layout::default().with_cross_align(Align::RIGHT),
                    |ui| {
                        if ui.button("⚙ Settings").clicked() {
                            self.settings_open = true;
                        }
                    },
                );
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            self.freecam_control.update(ui, remote, &mut self.notify);

            ui.separator();

            self.world_control.update(ui, remote, &mut self.notify);
        });

        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            match self.timeline_control.update(
                ui,
                remote,
                &mut self.notify,
                self.settings_control.data(),
            ) {
                Some(TimelineControlCommand::PlaybackDone) => {
                    self.camera_mode = CameraMode::Freecam;
                    if let Err(e) = remote.set_camera_mode(self.camera_mode) {
                        self.notify.error(format!("Could not switch camera mode to freecam: {e}"));
                    }
                },
                Some(TimelineControlCommand::Scrub) => {
                    self.camera_mode = CameraMode::Playback;
                    if let Err(e) = remote.set_camera_mode(self.camera_mode) {
                        self.notify.error(format!("Could not switch camera mode to playback: {e}"));
                    }
                },
                None => {},
            }
        });
    }
}
