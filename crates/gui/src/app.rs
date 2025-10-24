use crate::freecam::FreeCamControl;
use crate::game::RemoteGame;
// use crate::keybind::SettingsKeybindControl;
use crate::process::{self, GameProcess};
use crate::program_title;
use crate::settings::{SettingsControl, get_settings, save_settings};
use crate::timeline::TimelineControl;
use crate::world::WorldControl;
use eframe::egui::{
    self, Align, CentralPanel, ComboBox, Context, Layout, TopBottomPanel, Visuals
};
use egui_notify::Toasts;
use protocol::{CameraMode, OutboundGameControlEvent, SettingsData};

pub(crate) struct CameraControlApp {
    notify: Toasts,

    process: Option<GameProcess>,
    remote: Option<RemoteGame>,

    settings_open: bool,
    // settings: SettingsData,

    camera_mode: CameraMode,
    hud_disabled: bool,
    debug_pause_enabled: bool,

    settings_control: SettingsControl,
    // settings_keybinds_control: SettingsKeybindControl,
    freecam_control: FreeCamControl,
    world_control: WorldControl,
    timeline_control: TimelineControl,

    // // Game control
    // time_of_day_control: u32,

    // // freecam_enabled: bool,
    // character_no_dead: bool,
    // character_no_move: bool,
    // gamespeed_multiplier: f32,
    // global_fov: f32,
    // fov_override_enabled: bool,

    // // Keyframe
    // keyframes: Vec<Keyframe>,
    // dragging_index: Option<usize>,
    // selected_index: Option<usize>,
    // timeline_duration: f32,
    // playing: bool,
    // // playback_mode_active: bool,
    // playback_time: f32,
    // last_update: Option<std::time::Instant>,
    // place_keyframes_at_playback_time: bool,
}

impl Default for CameraControlApp {
    fn default() -> Self {
        let settings = get_settings().expect("Could not load settings");

        Self {
            notify: Toasts::default(),
            settings_control: SettingsControl::new(settings),
            // settings_keybinds_control: SettingsKeybindControl::from_settings(&settings.keybinds),
            freecam_control: FreeCamControl::default(),
            world_control: WorldControl::default(),
            timeline_control: TimelineControl::default(),

            process: None,
            remote: None,

            settings_open: false,
            // settings,

            camera_mode: CameraMode::Game,
            hud_disabled: false,
            debug_pause_enabled: false,

            // // Debug pause + frame-by-frame
            // debug_pause_enabled: false,
            // // freecam_enabled: false,
            // global_fov: 48.0f32.to_radians(),
            // fov_override_enabled: false
            //
            // // Other garbage
            // keyframes: Vec::new(),
            // dragging_index: None,
            // selected_index: None,
            // timeline_duration: 90.0,
            // playing: false,
            // // playback_mode_active: false,
            // playback_time: 0.0,
            // last_update: None,
            //
            // place_keyframes_at_playback_time: false,
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

            OutboundGameControlEvent::IncreaseFov => {},
            OutboundGameControlEvent::DecreaseFov => {},
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
                    process::get_running_games().iter().for_each(|e| {
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

                    if remote.initialize(&self.settings_control.data()).is_err() {
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
            // self.settings_keybinds_control.update(ctx, ui);
        });

        if let Some(remote) = self.remote.as_ref() && flush_settings {
            if let Err(e) = save_settings(self.settings_control.data()) {
                self.notify.error(format!("Could not save settings: {e}"));
            }

            if let Err(e) = remote.set_settings(self.settings_control.data().clone()) {
                self.notify.error(format!("Could not pass settings to game: {e}"));
            }

            self.notify.info("Saved and applied new settings");
        }
    }

    fn update_main(&mut self, ctx: &egui::Context) {
        // Safety: should be fine since we check if there is a remote at all
        let remote = self.remote.as_ref().expect("remote was None");

        TopBottomPanel::top("top_controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(program_title());

                ui.separator();

                let prev_camera_mode = self.camera_mode.clone();
                ComboBox::from_label("Camera mode")
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

                ui.add_enabled_ui(
                    self.camera_mode == CameraMode::Playback,
                    |ui| {
                        if !self.timeline_control.playing() {
                            if ui.button("▶ Play").clicked() {
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
                        } else {
                            if ui.button("⏸ Pause").clicked() {
                                if let Err(e) = remote.set_playback_state(false, self.timeline_control.time()) {
                                    self.notify.error(format!("Could not pause path: {e}"));
                                    return;
                                }

                                self.camera_mode = CameraMode::Playback;
                                self.timeline_control.stop();
                            }
                        }
                    },
                );

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
            self.world_control.update(ui, remote, &mut self.notify);
        });

        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            self.timeline_control.update(ui, remote, &mut self.notify, self.settings_control.data());
        });
    }

    // fn update_main(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    //     let now = std::time::Instant::now();
    //     let delta_seconds = if let Some(last) = self.last_update {
    //         (now - last).as_secs_f32()
    //     } else {
    //         0.0
    //     };
    //
    //     self.last_update = Some(now);
    //     if self.playing {
    //         self.playback_time += delta_seconds;
    //
    //         if self.playback_time > self.timeline_duration {
    //             self.playing = false;
    //             self.playback_time = 0.0;
    //         }
    //     }
    //
    //     // Track if we need to flush changes to agent
    //     let mut changed = false;
    //     TopBottomPanel::top("top_controls").show(ctx, |ui| {
    //         ui.horizontal(|ui| {
    //             ui.label(program_title());
    //
    //             ui.separator();
    //
    //             if ui.button("💾 Save").clicked()
    //                 && let Err(err) = prompt_and_save_keyframes(&self.keyframes)
    //             {
    //                 self.notify.error(format!("Could not save file: {err}"));
    //             }
    //
    //             if ui.button("📂 Load").clicked()
    //                 && let Some(loaded) = prompt_and_load_keyframes()
    //             {
    //                 self.keyframes = loaded;
    //                 self.selected_index = None;
    //                 return;
    //             }
    //
    //             ui.separator();
    //
    //             if self.playing {
    //                 if ui.button("⏸ Pause").clicked() {
    //                     if let Err(e) = self.remote.as_ref().unwrap().post_event(
    //                         InboundGameControlEvent::Pause {
    //                             time: self.playback_time,
    //                         },
    //                     ) {
    //                         self.notify
    //                             .error(format!("Could not pause playback remotely: {e}"));
    //                     } else {
    //                         self.playing = false;
    //                     }
    //                 }
    //             } else if ui.button("▶ Play").clicked() {
    //                 if let Err(e) =
    //                     self.remote
    //                         .as_ref()
    //                         .unwrap()
    //                         .post_event(InboundGameControlEvent::Play {
    //                             time: self.playback_time,
    //                             keyframes: self.keyframes.clone(),
    //                         })
    //                 {
    //                     self.notify
    //                         .error(format!("Could not start playback remotely: {e}"));
    //                 } else {
    //                     self.playing = true;
    //                     // self.playback_mode_active = true;
    //                 }
    //             }
    //
    //             // if ui
    //             //     .checkbox(&mut self.playback_mode_active, "Playback")
    //             //     .changed()
    //             // {
    //             //     if !self.playback_mode_active {
    //             //         self.playing = false;
    //             //         if let Err(e) = self.remote.as_ref().unwrap().post_event(
    //             //             InboundGameControlEvent::Pause {
    //             //                 time: self.playback_time,
    //             //             },
    //             //         ) {
    //             //             self.notify
    //             //                 .error(format!("Could not pause playback remotely: {e}"));
    //             //         } else {
    //             //             self.playing = false;
    //             //         }
    //             //     }
    //             //
    //             //     if let Err(e) = self.remote.as_ref().unwrap().post_event(
    //             //         InboundGameControlEvent::PlaybackModeState {
    //             //             state: self.playback_mode_active,
    //             //         },
    //             //     ) {
    //             //         self.notify
    //             //             .error(format!("Could not start disengage camera remotely: {e}"));
    //             //     }
    //             // }
    //
    //             // if ui
    //             //     .checkbox(&mut self.freecam_enabled, "Freecam")
    //             //         .changed() &&
    //             //         let Err(e) = self.remote.as_ref().unwrap().post_event(
    //             //             InboundGameControlEvent::SetFreecamEnabled {
    //             //                 enabled: self.freecam_enabled,
    //             //             },
    //             //         ) {
    //             //             self.notify
    //             //                 .error(format!("Could not toggle freecam remotely: {e}"));
    //             // }
    //
    //             ui.separator();
    //
    //             if ui.button("⚙ Settings").clicked() {
    //                 self.settings_open = true;
    //             }
    //         });
    //     });
    //
    //     CentralPanel::default().show(ctx, |ui| {
    //         if ui.input(|i| i.pointer.any_released()) {
    //             self.dragging_index = None;
    //         }
    //
    //         ui.horizontal(|ui| {
    //             if drag_percentage(ui, "Game speed", &mut self.gamespeed_multiplier, true).changed()
    //                 && let Err(e) = self.remote.as_ref().unwrap().post_event(
    //                     InboundGameControlEvent::TimeMultiplier {
    //                         multiplier: self.gamespeed_multiplier,
    //                     },
    //                 )
    //             {
    //                 self.notify
    //                     .error(format!("Could not change game speed: {e}"));
    //             }
    //
    //
    //             ui.separator();
    //
    //             if ui.checkbox(&mut self.hud_disabled, "Disable HUD").changed()
    //                 && let Err(e) = self.remote.as_ref().unwrap().post_event(
    //                     InboundGameControlEvent::SetHudDisabled {
    //                         disabled: self.hud_disabled,
    //                     },
    //                 )
    //             {
    //                 self.notify.error(format!("Could not disable HUD: {e}"));
    //             }
    //         });
    //
    //         ui.separator();
    //
    //         ui.horizontal(|ui| {
    //             if ui
    //                 .checkbox(&mut self.character_no_dead, "No Dead")
    //                 .changed()
    //                 && let Err(e) = self.remote.as_ref().unwrap().post_event(
    //                     InboundGameControlEvent::SetCharacterNoDead {
    //                         value: self.character_no_dead,
    //                     },
    //                 )
    //             {
    //                 self.notify.error(format!("Could not enable no dead: {e}"));
    //             }
    //
    //             if ui
    //                 .checkbox(&mut self.character_no_move, "Disable character movement")
    //                 .changed()
    //                 && let Err(e) = self.remote.as_ref().unwrap().post_event(
    //                     InboundGameControlEvent::SetCharacterNoMove {
    //                         value: self.character_no_move,
    //                     },
    //                 )
    //             {
    //                 self.notify.error(format!("Could not enable no move: {e}"));
    //             }
    //         });
    //
    //         ui.separator();
    //
    //         ui.horizontal(|ui| {
    //             if drag_percentage(
    //                 ui,
    //                 "FC movement speed",
    //                 &mut self.freecam_movement_speed,
    //                 false,
    //             )
    //             .changed()
    //                 && let Err(e) = self.remote.as_ref().unwrap().post_event(
    //                     InboundGameControlEvent::SetFreecamMovementSpeed {
    //                         value: self.freecam_movement_speed,
    //                     },
    //                 )
    //             {
    //                 self.notify
    //                     .error(format!("Could not change freecam movement speed: {e}"));
    //             }
    //
    //             if drag_percentage(
    //                 ui,
    //                 "FC rotation speed",
    //                 &mut self.freecam_rotation_speed,
    //                 false,
    //             )
    //             .changed()
    //                 && let Err(e) = self.remote.as_ref().unwrap().post_event(
    //                     InboundGameControlEvent::SetFreecamRotationSpeed {
    //                         value: self.freecam_rotation_speed,
    //                     },
    //                 )
    //             {
    //                 self.notify
    //                     .error(format!("Could not change freecam rotation speed: {e}"));
    //             }
    //
    //             ui.separator();
    //             if ui.checkbox(&mut self.fov_override_enabled, "").changed()
    //                 && let Err(e) = self.remote.as_ref().unwrap().post_event(
    //                     InboundGameControlEvent::GlobalFov {
    //                         fov: self.global_fov,
    //                         enabled: self.fov_override_enabled,
    //                     },
    //                 )
    //             {
    //                 self.notify
    //                     .error(format!("Could not change global FOV: {e}"));
    //             }
    //
    //             if drag_angle(ui, "", &mut self.global_fov).changed()
    //                 && let Err(e) = self.remote.as_ref().unwrap().post_event(
    //                     InboundGameControlEvent::GlobalFov {
    //                         fov: self.global_fov,
    //                         enabled: self.fov_override_enabled,
    //                     },
    //                 )
    //             {
    //                 self.notify
    //                     .error(format!("Could not change global FOV: {e}"));
    //             }
    //             ui.label("FC FoV");
    //         });
    //
    //         ui.separator();
    //
    //         ui.horizontal(|ui| {
    //             // Convert minutes to hh:mm string
    //             let hours = self.time_of_day_control / 60;
    //             let mins = self.time_of_day_control % 60;
    //             let label = format!("{:02}:{:02}", hours, mins);
    //
    //             if ui.add(
    //                 Slider::new(&mut self.time_of_day_control, 0..=(23 * 60 + 59))
    //                     .show_value(false)
    //                     .text(label)
    //                     .step_by(1.0),
    //             ).changed() && let Err(e) = self.remote.as_ref().unwrap().post_event(
    //                 InboundGameControlEvent::SetTimeOfDay {
    //                     hours: hours as u8,
    //                     minutes: mins as u8,
    //                     seconds: 0,
    //                 },
    //             ) {
    //                 self.notify
    //                     .error(format!("Could not dispatch scrub event to game: {e}"));
    //             }
    //         });
    //
    //         ui.separator();
    //
    //         if let Some(i) = self.selected_index {
    //             let pre_edit = self.keyframes[i].clone();
    //             let mut kf = pre_edit.clone();
    //
    //             ui.vertical(|ui| {
    //                 ui.label("Keyframe details");
    //                 ui.horizontal(|ui| {
    //                     ui.label("Time (s):");
    //                     changed |= ui.add(DragValue::new(&mut kf.time).speed(0.01)).changed();
    //                 });
    //
    //                 if ui.button("Read from Game").clicked() {
    //                     match self.remote.as_ref().unwrap().snapshot_camera_state() {
    //                         Ok(c) => {
    //                             kf.map_id = c.map_id;
    //                             kf.position = c.position;
    //                             kf.orientation = c.orientation;
    //                             kf.fov = c.fov;
    //                         }
    //                         Err(e) => {
    //                             self.notify.error(format!("Game process error: {e}"));
    //                         }
    //                     }
    //                 }
    //
    //                 ui.horizontal(|ui| {
    //                     ui.label("Position:");
    //                     ui.label(format!("Map ID: {:X}", kf.map_id));
    //
    //                     changed |= ui
    //                         .add(DragValue::new(&mut kf.position.x).speed(0.1).prefix("x: "))
    //                         .changed();
    //                     changed |= ui
    //                         .add(DragValue::new(&mut kf.position.y).speed(0.1).prefix("y: "))
    //                         .changed();
    //                     changed |= ui
    //                         .add(DragValue::new(&mut kf.position.z).speed(0.1).prefix("z: "))
    //                         .changed();
    //                 });
    //
    //                 ui.horizontal(|ui| {
    //                     ui.label(format!("Rotation: {:?}", kf.orientation));
    //
    //                     // changed |= drag_pitch(ui, "pitch", &mut kf.orientation.0).changed();
    //                     // changed |= drag_angle_signed(ui, "yaw", &mut kf.orientation.1).changed();
    //                     // changed |= drag_angle_signed(ui, "roll", &mut kf.orientation.2).changed();
    //                 });
    //
    //                 ui.horizontal(|ui| {
    //                     ui.label("FOV:");
    //                     changed |= drag_angle(ui, "", &mut kf.fov).changed();
    //                 });
    //
    //                 ui.separator();
    //             });
    //
    //             let post_edit = kf.clone();
    //             if pre_edit != post_edit {
    //                 self.keyframes[i] = post_edit;
    //             }
    //         }
    //     });
    //
    //     TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
    //         ui.horizontal(|ui| {
    //             if ui.button("➕ Add Keyframe").clicked() {
    //                 let time = self.playback_time;
    //                 let mut kf = Keyframe {
    //                     map_id: -1,
    //                     time,
    //                     position: Vec3::new(0.0, 0.0, 0.0),
    //                     orientation: Quat::IDENTITY,
    //                     fov: 0.89,
    //                 };
    //
    //                 match self.remote.as_ref().unwrap().snapshot_camera_state() {
    //                     Ok(c) => {
    //                         kf.map_id = c.map_id;
    //                         kf.position = c.position;
    //                         kf.orientation = c.orientation;
    //                         kf.fov = c.fov;
    //                     }
    //                     Err(e) => {
    //                         self.notify
    //                             .error(format!("Could not retrieve current camera state: {e}"));
    //                     }
    //                 }
    //
    //                 self.keyframes.push(kf);
    //                 self.selected_index = Some(self.keyframes.len() - 1);
    //                 changed = true;
    //             }
    //
    //             ui.add_enabled_ui(self.selected_index.is_some(), |ui| {
    //                 if ui.button("📋 Duplicate Keyframe").clicked() {
    //                     let mut kf = self.keyframes[*self.selected_index.as_ref().unwrap()].clone();
    //                     kf.time += 1.0;
    //
    //                     self.keyframes.push(kf);
    //                     self.selected_index = Some(self.keyframes.len() - 1);
    //                     changed = true;
    //                 }
    //             });
    //
    //             ui.add_enabled_ui(self.selected_index.is_some(), |ui| {
    //                 if ui.button("❌ Delete Keyframe").clicked() {
    //                     self.keyframes.remove(self.selected_index.take().unwrap());
    //
    //                     // Select prev keyframe instead
    //                     if !self.keyframes.is_empty() {
    //                         self.selected_index = Some(self.keyframes.len() - 1);
    //                     }
    //                     changed = true;
    //                 }
    //             });
    //
    //             ui.checkbox(
    //                 &mut self.place_keyframes_at_playback_time,
    //                 "Place keyframes at current playback time",
    //             );
    //         });
    //
    //         let pixels_per_second = ui.available_width() / self.timeline_duration;
    //
    //         let timeline_height = 50.0;
    //         let (timeline_rect, timeline_response) = ui.allocate_exact_size(
    //             Vec2::new(ui.available_width(), timeline_height),
    //             Sense::click_and_drag(),
    //         );
    //         let painter = ui.painter_at(timeline_rect);
    //         let timeline_top = timeline_rect.top();
    //         let timeline_left = timeline_rect.left();
    //
    //         // Draw background
    //         painter.rect_filled(timeline_rect, 0.0, Color32::DARK_GRAY);
    //
    //         // Second markers
    //         let total_seconds = self.timeline_duration.ceil() as usize;
    //         for second in 0..=total_seconds {
    //             let x = timeline_left + second as f32 * pixels_per_second;
    //             painter.line_segment(
    //                 [
    //                     Pos2::new(x, timeline_top),
    //                     Pos2::new(x, timeline_top + 10.0),
    //                 ],
    //                 (1.0, Color32::WHITE),
    //             );
    //
    //             if second % 5 == 0 {
    //                 let minute = second / 60;
    //                 let second = second % 60;
    //
    //                 painter.text(
    //                     Pos2::new(x + 2.0, timeline_top + 12.0),
    //                     Align2::LEFT_TOP,
    //                     format!("{minute:#01}:{second:#02}"),
    //                     FontId::monospace(10.0),
    //                     Color32::WHITE,
    //                 );
    //             }
    //         }
    //
    //         // Draw keyframes as vertical markers
    //         for (i, keyframe) in self.keyframes.iter_mut().enumerate() {
    //             let x = timeline_left + keyframe.time * pixels_per_second;
    //             let kf_top = timeline_top + 20.0;
    //             let kf_bottom = timeline_rect.bottom() - 5.0;
    //             let color = if self.selected_index == Some(i) {
    //                 Color32::YELLOW
    //             } else {
    //                 Color32::LIGHT_BLUE
    //             };
    //
    //             let line_rect =
    //                 Rect::from_min_max(Pos2::new(x - 4.0, kf_top), Pos2::new(x + 4.0, kf_bottom));
    //             let response = ui.allocate_rect(line_rect, Sense::click_and_drag());
    //             painter.rect_filled(line_rect, 2.0, color);
    //
    //             if response.clicked() {
    //                 self.selected_index = Some(i);
    //             }
    //
    //             if response.drag_started() {
    //                 self.dragging_index = Some(i);
    //             }
    //
    //             if response.dragged() && self.dragging_index == Some(i) {
    //                 keyframe.time += response.drag_delta().x / pixels_per_second;
    //                 keyframe.time = keyframe.time.clamp(0.0, self.timeline_duration);
    //                 changed = true;
    //             }
    //         }
    //
    //         // Draw playhead
    //         let playhead_x = timeline_left + self.playback_time * pixels_per_second;
    //         let playhead_top = timeline_rect.top();
    //         let playhead_bottom = timeline_rect.bottom();
    //
    //         painter.line_segment(
    //             [
    //                 Pos2::new(playhead_x, playhead_top),
    //                 Pos2::new(playhead_x, playhead_bottom),
    //             ],
    //             (2.0, Color32::RED),
    //         );
    //
    //         painter.text(
    //             Pos2::new(playhead_x + 4.0, playhead_top),
    //             Align2::LEFT_TOP,
    //             format!("{:2.2}", self.playback_time),
    //             FontId::monospace(12.0),
    //             Color32::RED,
    //         );
    //
    //         // Handle scrubbing motions
    //         if (timeline_response.clicked() || timeline_response.dragged())
    //             && let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos())
    //         {
    //             let relative_x = pointer_pos.x - timeline_rect.left();
    //             let new_time = (relative_x / pixels_per_second).clamp(0.0, self.timeline_duration);
    //             self.playback_time = new_time;
    //             self.last_update = Some(std::time::Instant::now());
    //
    //             // if self.playback_mode_active
    //             //     && let Err(e) = self
    //             //         .remote
    //             //         .as_ref()
    //             //         .unwrap()
    //             //         .post_event(InboundGameControlEvent::Scrub { time: new_time })
    //             // {
    //             //     self.notify
    //             //         .error(format!("Could not dispatch scrub event to game: {e}"));
    //             // }
    //         }
    //
    //         // Pause when scrubbing
    //         if timeline_response.dragged() {
    //             self.playing = false;
    //         }
    //
    //         let input = ui.input(|i| i.clone());
    //
    //         // Toggle playback
    //         if input.key_pressed(egui::Key::Space) {
    //             self.toggle_playback();
    //         }
    //
    //         if input.key_pressed(egui::Key::Delete) {
    //             self.keyframes.remove(self.selected_index.take().unwrap());
    //
    //             // Select prev keyframe instead
    //             if !self.keyframes.is_empty() {
    //                 self.selected_index = Some(self.keyframes.len() - 1);
    //             }
    //             changed = true;
    //         }
    //     });
    //
    //     if changed
    //         && let Err(e) =
    //             self.remote
    //                 .as_ref()
    //                 .unwrap()
    //                 .post_event(InboundGameControlEvent::Keyframes {
    //                     keyframes: self.keyframes.clone(),
    //                 })
    //     {
    //         self.notify
    //             .error(format!("Could not send keyframes to game: {e}"));
    //     }
    // }
    //
    // fn toggle_playback(&mut self) {
    //     self.playing = !self.playing;
    //
    //     if self.playing && self.settings.playback_start_restarts_path {
    //         self.playback_time = 0.0;
    //     }
    //
    //     // self.playback_mode_active = true;
    //     if self.playing {
    //         if let Err(e) =
    //             self.remote
    //                 .as_ref()
    //                 .unwrap()
    //                 .post_event(InboundGameControlEvent::Play {
    //                     time: self.playback_time,
    //                     keyframes: self.keyframes.clone(),
    //                 })
    //         {
    //             self.notify
    //                 .error(format!("Could not start playback remotely: {e}"));
    //         }
    //     } else if let Err(e) =
    //         self.remote
    //             .as_ref()
    //             .unwrap()
    //             .post_event(InboundGameControlEvent::Pause {
    //                 time: self.playback_time,
    //             })
    //     {
    //         self.notify
    //             .error(format!("Could not pause playback remotely: {e}"));
    //     }
    // }
    //
    // fn create_keyframe(&mut self) {
    //     let time = if self.place_keyframes_at_playback_time {
    //         self.playback_time
    //     } else if self.keyframes.is_empty() {
    //         0.0
    //     } else {
    //         self.keyframes[*self.selected_index.as_ref().unwrap()]
    //             .clone()
    //             .time
    //             + 1.0
    //     };
    //
    //     let mut kf = Keyframe {
    //         map_id: -1,
    //         time,
    //         position: Vec3::new(0.0, 0.0, 0.0),
    //         orientation: Quat::IDENTITY,
    //         fov: 0.89,
    //     };
    //
    //     match self.remote.as_ref().unwrap().snapshot_camera_state() {
    //         Ok(c) => {
    //             kf.map_id = c.map_id;
    //             kf.position = c.position;
    //             kf.orientation = c.orientation;
    //             kf.fov = c.fov;
    //         }
    //         Err(e) => {
    //             self.notify
    //                 .error(format!("Could not retrieve current camera state: {e}"));
    //         }
    //     }
    //
    //     self.keyframes.push(kf);
    //     self.selected_index = Some(self.keyframes.len() - 1);
    //
    //     if let Err(e) =
    //         self.remote
    //             .as_ref()
    //             .unwrap()
    //             .post_event(InboundGameControlEvent::Keyframes {
    //                 keyframes: self.keyframes.clone(),
    //             })
    //     {
    //         self.notify
    //             .error(format!("Could not send keyframes to game: {e}"));
    //     }
    // }
    //
    // fn toggle_hud(&mut self) {
    //     self.hud_disabled = !self.hud_disabled;
    //
    //     if let Err(e) =
    //         self.remote
    //             .as_ref()
    //             .unwrap()
    //             .post_event(InboundGameControlEvent::SetHudDisabled {
    //                 disabled: self.hud_disabled,
    //             })
    //     {
    //         self.notify.error(format!("Could not disable HUD: {e}"));
    //     }
    // }
    //
    // fn toggle_character_no_dead(&mut self) {
    //     self.character_no_dead = !self.character_no_dead;
    //
    //     if let Err(e) =
    //         self.remote
    //             .as_ref()
    //             .unwrap()
    //             .post_event(InboundGameControlEvent::SetCharacterNoDead {
    //                 value: self.character_no_dead,
    //             })
    //     {
    //         self.notify.error(format!("Could not enable no dead: {e}"));
    //     }
    // }
    //
    // fn toggle_character_no_move(&mut self) {
    //     self.character_no_move = !self.character_no_move;
    //
    //     if let Err(e) =
    //         self.remote
    //             .as_ref()
    //             .unwrap()
    //             .post_event(InboundGameControlEvent::SetCharacterNoMove {
    //                 value: self.character_no_move,
    //             })
    //     {
    //         self.notify.error(format!("Could not enable no move: {e}"));
    //     }
    // }
    //
    // fn toggle_fov_override(&mut self) {
    //     self.fov_override_enabled = !self.fov_override_enabled;
    //
    //     if let Err(e) =
    //         self.remote
    //             .as_ref()
    //             .unwrap()
    //             .post_event(InboundGameControlEvent::GlobalFov {
    //                 fov: self.global_fov,
    //                 enabled: self.fov_override_enabled,
    //             })
    //     {
    //         self.notify
    //             .error(format!("Could not change global FOV: {e}"));
    //     }
    // }
    //
    // fn set_fov(&mut self, fov: &f32) {
    //     self.global_fov = (*fov).clamp(2.0f32.to_radians(), 360.0f32.to_radians());
    //
    //     if let Err(e) =
    //         self.remote
    //             .as_ref()
    //             .unwrap()
    //             .post_event(InboundGameControlEvent::GlobalFov {
    //                 fov: self.global_fov,
    //                 enabled: self.fov_override_enabled,
    //             })
    //     {
    //         self.notify
    //             .error(format!("Could not change global FOV: {e}"));
    //     }
    // }
    //
    // fn set_gamespeed(&mut self, multiplier: &f32) {
    //     self.gamespeed_multiplier = (*multiplier).clamp(0.001, 100.0);
    //
    //     if let Err(e) =
    //         self.remote
    //             .as_ref()
    //             .unwrap()
    //             .post_event(InboundGameControlEvent::TimeMultiplier {
    //                 multiplier: self.gamespeed_multiplier,
    //             })
    //     {
    //         self.notify
    //             .error(format!("Could not change game speed: {e}"));
    //     }
    // }
    //
    // fn toggle_debug_pause_enabled(&mut self) {
    //     self.debug_pause_enabled = !self.debug_pause_enabled;
    //
    //     if let Err(e) =
    //         self.remote
    //             .as_ref()
    //             .unwrap()
    //             .post_event(InboundGameControlEvent::SetDebugPause {
    //                 enabled: self.debug_pause_enabled,
    //             })
    //     {
    //         self.notify
    //             .error(format!("Could not toggle debug pause: {e}"));
    //     }
    // }
}
