use crate::controls::{drag_angle, drag_angle_signed, drag_percentage, drag_percentage_delta, drag_pitch};
use crate::process::{self, GameProcess, RemoteGame};
use crate::program_title;
use crate::save::{prompt_and_load_keyframes, prompt_and_save_keyframes};
use crate::settings::{get_settings, save_settings};
use eframe::egui::{self, Align2, CentralPanel, ComboBox, DragValue, FontId, Key, Slider, TopBottomPanel};
use egui::{Color32, Pos2, Rect, Sense, Vec2};
use egui_notify::Toasts;
use protocol::keyframe::{Keyframe, Quat, Vec3};
use protocol::{InboundGameControlEvent, Keybind, KeybindAction, KeybindInput, OutboundGameControlEvent, SettingsData};

pub(crate) struct CameraControlApp {
    process: Option<GameProcess>,
    remote: Option<RemoteGame>,
    notify: Toasts,

    // Game control
    time_of_day_control: u32,
    debug_pause_enabled: bool,
    freecam_enabled: bool,
    character_no_dead: bool,
    character_no_move: bool,
    gamespeed_multiplier: f32,
    global_fov: f32,
    fov_override_enabled: bool,
    hud_disabled: bool,

    // Settings
    settings_open: bool,
    settings: SettingsData,
    waiting_for_keybind_input: Option<usize>,

    // Keyframe
    keyframes: Vec<Keyframe>,
    dragging_index: Option<usize>,
    selected_index: Option<usize>,
    timeline_duration: f32,
    playing: bool,
    playback_mode_active: bool,
    playback_time: f32,
    last_update: Option<std::time::Instant>,
    freecam_movement_speed: f32,
    freecam_rotation_speed: f32,
    place_keyframes_at_playback_time: bool,
}

impl Default for CameraControlApp {
    fn default() -> Self {
        let settings = get_settings().expect("Could not load settings");

        Self {
            process: None,
            remote: None,
            notify: Toasts::default(),

            // Time request control, default 06:00
            time_of_day_control: 6 * 60,
            // Debug pause + frame-by-frame
            debug_pause_enabled: false,
            freecam_enabled: false,
            character_no_dead: false,
            character_no_move: false,
            gamespeed_multiplier: 1.0,
            global_fov: 48.0f32.to_radians(),
            fov_override_enabled: false,
            hud_disabled: false,

            freecam_movement_speed: 1.0,
            freecam_rotation_speed: 1.0,

            // Other garbage
            keyframes: Vec::new(),
            dragging_index: None,
            selected_index: None,
            timeline_duration: 60.0,
            playing: false,
            playback_mode_active: false,
            playback_time: 0.0,
            last_update: None,

            place_keyframes_at_playback_time: false,
            settings_open: false,
            settings,
            waiting_for_keybind_input: None,
        }
    }
}

impl eframe::App for CameraControlApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.remote.is_none() {
            self.update_game_selector(ctx, frame);
        } else if self.settings_open {
            self.update_settings(ctx, frame);
        } else {
            self.update_main(ctx, frame);
        }

        if let Some(remote) = self.remote.as_ref() {
            // Need this here to re-request for poll_events as well as timeline visual updates.
            ctx.request_repaint();

            if let Ok(events) = remote.poll_events() {
                for event in events {
                    match event {
                        OutboundGameControlEvent::KeybindAction(action) => {
                            self.handle_keybind_action(&action)
                        }
                    }
                }
            } else {
                self.notify.error(
                    "Could not poll events from remote. Reattach to game please.".to_string(),
                );
                self.remote = None;
                self.playing = false;
            }
        }

        self.notify.show(ctx);
    }
}

impl CameraControlApp {
    fn handle_keybind_action(&mut self, action: &KeybindAction) {
        match action {
            KeybindAction::TogglePlaybackMode => self.toggle_playback(),
            KeybindAction::CreateKeyframe => self.create_keyframe(),
            KeybindAction::ToggleHUD => self.toggle_hud(),
            KeybindAction::ToggleCharacterNoDead => self.toggle_character_no_dead(),
            KeybindAction::ToggleCharacterNoMove => self.toggle_character_no_move(),
            KeybindAction::ToggleFovOverride => self.toggle_fov_override(),
            KeybindAction::SetFov(fov) => self.set_fov(fov),
            KeybindAction::AdjustFov(fov) => self.set_fov(&(self.global_fov + fov)),
            KeybindAction::SetGameSpeed(multiplier) => self.set_gamespeed(multiplier),
            KeybindAction::AdjustGamespeed(multiplier) => {
                self.set_gamespeed(&(self.gamespeed_multiplier + multiplier))
            }
            KeybindAction::ToggleDebugPause => self.toggle_debug_pause_enabled(),
            KeybindAction::ToggleFreecam => self.toggle_freecam(),
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
                if ui.button("Inject").clicked() {
                    if let Err(e) = self.inject() {
                        self.notify.error(format!("Inject error: {e}"));
                    } else {
                        self.notify.success("Succesfully attached to game");
                    }
                }
            });
        });
    }

    fn inject(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let remote = RemoteGame::new(self.process.as_ref().unwrap())?;
        remote.initialize(&self.settings)?;
        self.remote = Some(remote);
        Ok(())
    }

    fn update_settings(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut flush_settings = false;
        TopBottomPanel::top("top_controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(program_title());

                ui.separator();
                if ui.button("⬅ Back").clicked() {
                    self.settings_open = false;
                }

                if ui.button("💾 Save and apply").clicked() {
                    flush_settings = true;
                }
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            ui.heading("General");
            ui.separator();

            ui.checkbox(
                &mut self.settings.apply_gamespeed_only_when_playback_mode_active,
                "Apply Game speed only when playback mode is active",
            );

            ui.checkbox(
                &mut self.settings.enabling_playback_disables_freecam,
                "Starting playback disables freecam",
            );

            ui.checkbox(
                &mut self.settings.playback_start_restarts_path,
                "Start camera path from start when starting playback",
            );

            ui.spacing();
            ui.spacing();
            ui.spacing();

            ui.heading("Keybinds");
            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Create keybind").clicked() {
                    self.settings.keybinds.push(Keybind {
                        active: true,
                        action: KeybindAction::TogglePlaybackMode,
                        input: None,
                    });
                }
            });

            let mut remove = None;
            for (i, bind) in self.settings.keybinds.iter_mut().enumerate() {
                match keybind_controls(ui, i, bind) {
                    Some(KeybindRequest::Remove(index)) => remove = Some(index),
                    Some(KeybindRequest::StartBinding(index)) => {
                        self.waiting_for_keybind_input = Some(index)
                    }
                    None => {}
                }
            }

            if let Some(index) = remove {
                self.settings.keybinds.remove(index);
                if let Some(waiting) = self.waiting_for_keybind_input {
                    if waiting == index {
                        self.waiting_for_keybind_input = None;
                    } else if waiting > index {
                        self.waiting_for_keybind_input = Some(waiting - 1);
                    }
                }
            }

            // Sense inputs when necessary
            let mut caught_input = None;
            if let Some(ref action) = self.waiting_for_keybind_input {
                for key in Key::ALL {
                    if ctx.input(|i| i.key_pressed(*key))
                        && let Some(key) = egui_key_to_vk(*key)
                    {
                        caught_input = Some(KeybindInput::Keyboard(key));
                    }
                }

                if let Some(input) = caught_input {
                    self.settings.keybinds[*action].input = Some(input);
                    self.waiting_for_keybind_input = None;
                }
            }
        });

        if flush_settings {
            if let Err(e) = save_settings(&self.settings) {
                self.notify.error(format!("Could not save settings: {e}"));
            } else {
                self.playing = false;
            }

            if let Err(e) =
                self.remote
                    .as_ref()
                    .unwrap()
                    .post_event(InboundGameControlEvent::Settings {
                        settings: self.settings.clone(),
                    })
            {
                self.notify
                    .error(format!("Could not pass settings to game: {e}"));
            } else {
                self.playing = false;
            }

            self.notify.info("Applied new settings");
        }
    }

    fn update_main(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = std::time::Instant::now();
        let delta_seconds = if let Some(last) = self.last_update {
            (now - last).as_secs_f32()
        } else {
            0.0
        };

        self.last_update = Some(now);
        if self.playing {
            self.playback_time += delta_seconds;

            if self.playback_time > self.timeline_duration {
                self.playing = false;
                self.playback_time = 0.0;
            }
        }

        // Track if we need to flush changes to agent
        let mut changed = false;
        TopBottomPanel::top("top_controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(program_title());

                ui.separator();

                if ui.button("💾 Save").clicked()
                    && let Err(err) = prompt_and_save_keyframes(&self.keyframes)
                {
                    self.notify.error(format!("Could not save file: {err}"));
                }

                if ui.button("📂 Load").clicked()
                    && let Some(loaded) = prompt_and_load_keyframes()
                {
                    self.keyframes = loaded;
                    self.selected_index = None;
                    return;
                }

                ui.separator();

                if self.playing {
                    if ui.button("⏸ Pause").clicked() {
                        if let Err(e) = self.remote.as_ref().unwrap().post_event(
                            InboundGameControlEvent::Pause {
                                time: self.playback_time,
                            },
                        ) {
                            self.notify
                                .error(format!("Could not pause playback remotely: {e}"));
                        } else {
                            self.playing = false;
                        }
                    }
                } else if ui.button("▶ Play").clicked() {
                    if let Err(e) =
                        self.remote
                            .as_ref()
                            .unwrap()
                            .post_event(InboundGameControlEvent::Play {
                                time: self.playback_time,
                                keyframes: self.keyframes.clone(),
                            })
                    {
                        self.notify
                            .error(format!("Could not start playback remotely: {e}"));
                    } else {
                        self.playing = true;
                        self.playback_mode_active = true;
                    }
                }

                if ui
                    .checkbox(&mut self.playback_mode_active, "Playback")
                    .changed()
                {
                    if !self.playback_mode_active {
                        self.playing = false;
                        if let Err(e) = self.remote.as_ref().unwrap().post_event(
                            InboundGameControlEvent::Pause {
                                time: self.playback_time,
                            },
                        ) {
                            self.notify
                                .error(format!("Could not pause playback remotely: {e}"));
                        } else {
                            self.playing = false;
                        }
                    }

                    if let Err(e) = self.remote.as_ref().unwrap().post_event(
                        InboundGameControlEvent::PlaybackModeState {
                            state: self.playback_mode_active,
                        },
                    ) {
                        self.notify
                            .error(format!("Could not start disengage camera remotely: {e}"));
                    }
                }

                if ui
                    .checkbox(&mut self.freecam_enabled, "Freecam")
                        .changed() &&
                        let Err(e) = self.remote.as_ref().unwrap().post_event(
                            InboundGameControlEvent::SetFreecamEnabled {
                                enabled: self.freecam_enabled,
                            },
                        ) {
                            self.notify
                                .error(format!("Could not toggle freecam remotely: {e}"));
                }


                ui.separator();

                if ui.button("⚙ Settings").clicked() {
                    self.settings_open = true;
                }
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            if ui.input(|i| i.pointer.any_released()) {
                self.dragging_index = None;
            }

            ui.horizontal(|ui| {
                if drag_percentage(ui, "Game speed", &mut self.gamespeed_multiplier, true).changed()
                    && let Err(e) = self.remote.as_ref().unwrap().post_event(
                        InboundGameControlEvent::TimeMultiplier {
                            multiplier: self.gamespeed_multiplier,
                        },
                    )
                {
                    self.notify
                        .error(format!("Could not change game speed: {e}"));
                }


                ui.separator();

                if ui.checkbox(&mut self.hud_disabled, "Disable HUD").changed()
                    && let Err(e) = self.remote.as_ref().unwrap().post_event(
                        InboundGameControlEvent::SetHudDisabled {
                            disabled: self.hud_disabled,
                        },
                    )
                {
                    self.notify.error(format!("Could not disable HUD: {e}"));
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut self.character_no_dead, "No Dead")
                    .changed()
                    && let Err(e) = self.remote.as_ref().unwrap().post_event(
                        InboundGameControlEvent::SetCharacterNoDead {
                            value: self.character_no_dead,
                        },
                    )
                {
                    self.notify.error(format!("Could not enable no dead: {e}"));
                }

                if ui
                    .checkbox(&mut self.character_no_move, "Disable character movement")
                    .changed()
                    && let Err(e) = self.remote.as_ref().unwrap().post_event(
                        InboundGameControlEvent::SetCharacterNoMove {
                            value: self.character_no_move,
                        },
                    )
                {
                    self.notify.error(format!("Could not enable no move: {e}"));
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                if drag_percentage(
                    ui,
                    "FC movement speed",
                    &mut self.freecam_movement_speed,
                    false,
                )
                .changed()
                    && let Err(e) = self.remote.as_ref().unwrap().post_event(
                        InboundGameControlEvent::SetFreecamMovementSpeed {
                            value: self.freecam_movement_speed,
                        },
                    )
                {
                    self.notify
                        .error(format!("Could not change freecam movement speed: {e}"));
                }

                if drag_percentage(
                    ui,
                    "FC rotation speed",
                    &mut self.freecam_rotation_speed,
                    false,
                )
                .changed()
                    && let Err(e) = self.remote.as_ref().unwrap().post_event(
                        InboundGameControlEvent::SetFreecamRotationSpeed {
                            value: self.freecam_rotation_speed,
                        },
                    )
                {
                    self.notify
                        .error(format!("Could not change freecam rotation speed: {e}"));
                }

                ui.separator();
                if ui.checkbox(&mut self.fov_override_enabled, "").changed()
                    && let Err(e) = self.remote.as_ref().unwrap().post_event(
                        InboundGameControlEvent::GlobalFov {
                            fov: self.global_fov,
                            enabled: self.fov_override_enabled,
                        },
                    )
                {
                    self.notify
                        .error(format!("Could not change global FOV: {e}"));
                }

                if drag_angle(ui, "", &mut self.global_fov).changed()
                    && let Err(e) = self.remote.as_ref().unwrap().post_event(
                        InboundGameControlEvent::GlobalFov {
                            fov: self.global_fov,
                            enabled: self.fov_override_enabled,
                        },
                    )
                {
                    self.notify
                        .error(format!("Could not change global FOV: {e}"));
                }
                ui.label("FC FoV");
            });

            ui.separator();

            ui.horizontal(|ui| {
                // Convert minutes to hh:mm string
                let hours = self.time_of_day_control / 60;
                let mins = self.time_of_day_control % 60;
                let label = format!("{:02}:{:02}", hours, mins);

                if ui.add(
                    Slider::new(&mut self.time_of_day_control, 0..=(23 * 60 + 59))
                        .show_value(false)
                        .text(label)
                        .step_by(1.0),
                ).changed() && let Err(e) = self.remote.as_ref().unwrap().post_event(
                    InboundGameControlEvent::SetTimeOfDay {
                        hours: hours as u8,
                        minutes: mins as u8,
                        seconds: 0,
                    },
                ) {
                    self.notify
                        .error(format!("Could not dispatch scrub event to game: {e}"));
                }
            });

            ui.separator();

            if let Some(i) = self.selected_index {
                let pre_edit = self.keyframes[i].clone();
                let mut kf = pre_edit.clone();

                ui.vertical(|ui| {
                    ui.label("Keyframe details");
                    ui.horizontal(|ui| {
                        ui.label("Time (s):");
                        changed |= ui.add(DragValue::new(&mut kf.time).speed(0.01)).changed();
                    });

                    if ui.button("Read from Game").clicked() {
                        match self.remote.as_ref().unwrap().snapshot_camera_state() {
                            Ok(c) => {
                                kf.map_id = c.map_id;
                                kf.position = c.position;
                                kf.orientation = c.orientation;
                                kf.fov = c.fov;
                            }
                            Err(e) => {
                                self.notify.error(format!("Game process error: {e}"));
                            }
                        }
                    }

                    ui.horizontal(|ui| {
                        ui.label("Position:");
                        ui.label(format!("Map ID: {:X}", kf.map_id));

                        changed |= ui
                            .add(DragValue::new(&mut kf.position.x).speed(0.1).prefix("x: "))
                            .changed();
                        changed |= ui
                            .add(DragValue::new(&mut kf.position.y).speed(0.1).prefix("y: "))
                            .changed();
                        changed |= ui
                            .add(DragValue::new(&mut kf.position.z).speed(0.1).prefix("z: "))
                            .changed();
                    });

                    ui.horizontal(|ui| {
                        ui.label(format!("Rotation: {:?}", kf.orientation));
                        // changed |= drag_pitch(ui, "pitch", &mut kf.orientation.0).changed();
                        // changed |= drag_angle_signed(ui, "yaw", &mut kf.orientation.1).changed();
                        // changed |= drag_angle_signed(ui, "roll", &mut kf.orientation.2).changed();
                    });

                    ui.horizontal(|ui| {
                        ui.label("FOV:");
                        changed |= drag_angle(ui, "", &mut kf.fov).changed();
                    });

                    ui.separator();
                });

                let post_edit = kf.clone();
                if pre_edit != post_edit {
                    self.keyframes[i] = post_edit;
                }
            }
        });

        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("➕ Add Keyframe").clicked() {
                    let time = self.playback_time;
                    let mut kf = Keyframe {
                        map_id: -1,
                        time,
                        position: Vec3::new(0.0, 0.0, 0.0),
                        orientation: Quat::IDENTITY,
                        fov: 0.89,
                    };

                    match self.remote.as_ref().unwrap().snapshot_camera_state() {
                        Ok(c) => {
                            kf.map_id = c.map_id;
                            kf.position = c.position;
                            kf.orientation = c.orientation;
                            kf.fov = c.fov;
                        }
                        Err(e) => {
                            self.notify
                                .error(format!("Could not retrieve current camera state: {e}"));
                        }
                    }

                    self.keyframes.push(kf);
                    self.selected_index = Some(self.keyframes.len() - 1);
                    changed = true;
                }

                ui.add_enabled_ui(self.selected_index.is_some(), |ui| {
                    if ui.button("📋 Duplicate Keyframe").clicked() {
                        let mut kf = self.keyframes[*self.selected_index.as_ref().unwrap()].clone();
                        kf.time += 1.0;

                        self.keyframes.push(kf);
                        self.selected_index = Some(self.keyframes.len() - 1);
                        changed = true;
                    }
                });

                ui.add_enabled_ui(self.selected_index.is_some(), |ui| {
                    if ui.button("❌ Delete Keyframe").clicked() {
                        self.keyframes.remove(self.selected_index.take().unwrap());

                        // Select prev keyframe instead
                        if !self.keyframes.is_empty() {
                            self.selected_index = Some(self.keyframes.len() - 1);
                        }
                        changed = true;
                    }
                });

                ui.checkbox(
                    &mut self.place_keyframes_at_playback_time,
                    "Place keyframes at current playback time",
                );
            });

            let pixels_per_second = ui.available_width() / self.timeline_duration;

            let timeline_height = 50.0;
            let (timeline_rect, timeline_response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), timeline_height),
                Sense::click_and_drag(),
            );
            let painter = ui.painter_at(timeline_rect);
            let timeline_top = timeline_rect.top();
            let timeline_left = timeline_rect.left();

            // Draw background
            painter.rect_filled(timeline_rect, 0.0, Color32::DARK_GRAY);

            // Second markers
            let total_seconds = self.timeline_duration.ceil() as usize;
            for second in 0..=total_seconds {
                let x = timeline_left + second as f32 * pixels_per_second;
                painter.line_segment(
                    [
                        Pos2::new(x, timeline_top),
                        Pos2::new(x, timeline_top + 10.0),
                    ],
                    (1.0, Color32::WHITE),
                );

                if second % 5 == 0 {
                    painter.text(
                        Pos2::new(x + 2.0, timeline_top + 12.0),
                        Align2::LEFT_TOP,
                        format!("0:{second:#02}"),
                        FontId::monospace(10.0),
                        Color32::WHITE,
                    );
                }
            }

            // Draw keyframes as vertical markers
            for (i, keyframe) in self.keyframes.iter_mut().enumerate() {
                let x = timeline_left + keyframe.time * pixels_per_second;
                let kf_top = timeline_top + 20.0;
                let kf_bottom = timeline_rect.bottom() - 5.0;
                let color = if self.selected_index == Some(i) {
                    Color32::YELLOW
                } else {
                    Color32::LIGHT_BLUE
                };

                let line_rect =
                    Rect::from_min_max(Pos2::new(x - 4.0, kf_top), Pos2::new(x + 4.0, kf_bottom));
                let response = ui.allocate_rect(line_rect, Sense::click_and_drag());
                painter.rect_filled(line_rect, 2.0, color);

                if response.clicked() {
                    self.selected_index = Some(i);
                }

                if response.drag_started() {
                    self.dragging_index = Some(i);
                }

                if response.dragged() && self.dragging_index == Some(i) {
                    keyframe.time += response.drag_delta().x / pixels_per_second;
                    keyframe.time = keyframe.time.clamp(0.0, self.timeline_duration);
                    changed = true;
                }
            }

            // Draw playhead
            let playhead_x = timeline_left + self.playback_time * pixels_per_second;
            let playhead_top = timeline_rect.top();
            let playhead_bottom = timeline_rect.bottom();

            painter.line_segment(
                [
                    Pos2::new(playhead_x, playhead_top),
                    Pos2::new(playhead_x, playhead_bottom),
                ],
                (2.0, Color32::RED),
            );

            painter.text(
                Pos2::new(playhead_x + 4.0, playhead_top),
                Align2::LEFT_TOP,
                format!("{:2.2}", self.playback_time),
                FontId::monospace(12.0),
                Color32::RED,
            );

            // Handle scrubbing motions
            if (timeline_response.clicked() || timeline_response.dragged())
                && let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos())
            {
                let relative_x = pointer_pos.x - timeline_rect.left();
                let new_time = (relative_x / pixels_per_second).clamp(0.0, self.timeline_duration);
                self.playback_time = new_time;
                self.last_update = Some(std::time::Instant::now());

                if self.playback_mode_active
                    && let Err(e) = self
                        .remote
                        .as_ref()
                        .unwrap()
                        .post_event(InboundGameControlEvent::Scrub { time: new_time })
                {
                    self.notify
                        .error(format!("Could not dispatch scrub event to game: {e}"));
                }
            }

            // Pause when scrubbing
            if timeline_response.dragged() {
                self.playing = false;
            }

            let input = ui.input(|i| i.clone());

            // Toggle playback
            if input.key_pressed(egui::Key::Space) {
                self.toggle_playback();
            }

            if input.key_pressed(egui::Key::Delete) {
                self.keyframes.remove(self.selected_index.take().unwrap());

                // Select prev keyframe instead
                if !self.keyframes.is_empty() {
                    self.selected_index = Some(self.keyframes.len() - 1);
                }
                changed = true;
            }
        });

        if changed
            && let Err(e) =
                self.remote
                    .as_ref()
                    .unwrap()
                    .post_event(InboundGameControlEvent::Keyframes {
                        keyframes: self.keyframes.clone(),
                    })
        {
            self.notify
                .error(format!("Could not send keyframes to game: {e}"));
        }
    }

    fn toggle_playback(&mut self) {
        self.playing = !self.playing;

        if self.playing && self.settings.playback_start_restarts_path {
            self.playback_time = 0.0;
        }

        self.playback_mode_active = true;
        if self.playing {
            if let Err(e) =
                self.remote
                    .as_ref()
                    .unwrap()
                    .post_event(InboundGameControlEvent::Play {
                        time: self.playback_time,
                        keyframes: self.keyframes.clone(),
                    })
            {
                self.notify
                    .error(format!("Could not start playback remotely: {e}"));
            }
        } else if let Err(e) =
            self.remote
                .as_ref()
                .unwrap()
                .post_event(InboundGameControlEvent::Pause {
                    time: self.playback_time,
                })
        {
            self.notify
                .error(format!("Could not pause playback remotely: {e}"));
        }
    }

    fn create_keyframe(&mut self) {
        let time = if self.place_keyframes_at_playback_time {
            self.playback_time
        } else if self.keyframes.is_empty() {
            0.0
        } else {
            self.keyframes[*self.selected_index.as_ref().unwrap()]
                .clone()
                .time
                + 1.0
        };

        let mut kf = Keyframe {
            map_id: -1,
            time,
            position: Vec3::new(0.0, 0.0, 0.0),
            orientation: Quat::IDENTITY,
            fov: 0.89,
        };

        match self.remote.as_ref().unwrap().snapshot_camera_state() {
            Ok(c) => {
                kf.map_id = c.map_id;
                kf.position = c.position;
                kf.orientation = c.orientation;
                kf.fov = c.fov;
            }
            Err(e) => {
                self.notify
                    .error(format!("Could not retrieve current camera state: {e}"));
            }
        }

        self.keyframes.push(kf);
        self.selected_index = Some(self.keyframes.len() - 1);

        if let Err(e) =
            self.remote
                .as_ref()
                .unwrap()
                .post_event(InboundGameControlEvent::Keyframes {
                    keyframes: self.keyframes.clone(),
                })
        {
            self.notify
                .error(format!("Could not send keyframes to game: {e}"));
        }
    }

    fn toggle_hud(&mut self) {
        self.hud_disabled = !self.hud_disabled;

        if let Err(e) =
            self.remote
                .as_ref()
                .unwrap()
                .post_event(InboundGameControlEvent::SetHudDisabled {
                    disabled: self.hud_disabled,
                })
        {
            self.notify.error(format!("Could not disable HUD: {e}"));
        }
    }

    fn toggle_character_no_dead(&mut self) {
        self.character_no_dead = !self.character_no_dead;

        if let Err(e) =
            self.remote
                .as_ref()
                .unwrap()
                .post_event(InboundGameControlEvent::SetCharacterNoDead {
                    value: self.character_no_dead,
                })
        {
            self.notify.error(format!("Could not enable no dead: {e}"));
        }
    }

    fn toggle_character_no_move(&mut self) {
        self.character_no_move = !self.character_no_move;

        if let Err(e) =
            self.remote
                .as_ref()
                .unwrap()
                .post_event(InboundGameControlEvent::SetCharacterNoMove {
                    value: self.character_no_move,
                })
        {
            self.notify.error(format!("Could not enable no move: {e}"));
        }
    }

    fn toggle_fov_override(&mut self) {
        self.fov_override_enabled = !self.fov_override_enabled;

        if let Err(e) =
            self.remote
                .as_ref()
                .unwrap()
                .post_event(InboundGameControlEvent::GlobalFov {
                    fov: self.global_fov,
                    enabled: self.fov_override_enabled,
                })
        {
            self.notify
                .error(format!("Could not change global FOV: {e}"));
        }
    }

    fn set_fov(&mut self, fov: &f32) {
        self.global_fov = (*fov).clamp(2.0f32.to_radians(), 360.0f32.to_radians());

        if let Err(e) =
            self.remote
                .as_ref()
                .unwrap()
                .post_event(InboundGameControlEvent::GlobalFov {
                    fov: self.global_fov,
                    enabled: self.fov_override_enabled,
                })
        {
            self.notify
                .error(format!("Could not change global FOV: {e}"));
        }
    }

    fn set_gamespeed(&mut self, multiplier: &f32) {
        self.gamespeed_multiplier = (*multiplier).clamp(0.001, 100.0);

        if let Err(e) =
            self.remote
                .as_ref()
                .unwrap()
                .post_event(InboundGameControlEvent::TimeMultiplier {
                    multiplier: self.gamespeed_multiplier,
                })
        {
            self.notify
                .error(format!("Could not change game speed: {e}"));
        }
    }

    fn toggle_debug_pause_enabled(&mut self) {
        self.debug_pause_enabled = !self.debug_pause_enabled;

        if let Err(e) =
            self.remote
                .as_ref()
                .unwrap()
                .post_event(InboundGameControlEvent::SetDebugPause {
                    enabled: self.debug_pause_enabled,
                })
        {
            self.notify
                .error(format!("Could not toggle debug pause: {e}"));
        }
    }

    fn toggle_freecam(&mut self) {
        self.freecam_enabled = !self.freecam_enabled;

        if let Err(e) =
            self.remote
                .as_ref()
                .unwrap()
                .post_event(InboundGameControlEvent::SetFreecamEnabled {
                    enabled: self.freecam_enabled,
                })
        {
            self.notify
                .error(format!("Could not toggle freecam: {e}"));
        }
    }
}

pub enum KeybindRequest {
    Remove(usize),
    StartBinding(usize),
}

fn keybind_controls(ui: &mut egui::Ui, i: usize, bind: &mut Keybind) -> Option<KeybindRequest> {
    let mut result = None;

    ui.horizontal(|ui| {
        if ui.button("❌").clicked() {
            result = Some(KeybindRequest::Remove(i));
        }

        ui.checkbox(&mut bind.active, "");

        let label = match &bind.input {
            Some(input) => format!("Key: {}", format_input(input)),
            None => "Unbound".to_string(),
        };

        if ui.button(label).clicked() {
            result = Some(KeybindRequest::StartBinding(i));
        }

        ui.add_enabled_ui(bind.active, |ui| {
            ComboBox::new(i, "Action")
                .selected_text(match bind.action {
                    KeybindAction::TogglePlaybackMode => "Toggle playback mode",
                    KeybindAction::CreateKeyframe => "Create keyframe",
                    KeybindAction::ToggleHUD => "Toggle HUD",
                    KeybindAction::ToggleCharacterNoDead => "Toggle character no dead",
                    KeybindAction::ToggleCharacterNoMove => "Toggle character no move",
                    KeybindAction::ToggleFovOverride => "Toggle fov override",
                    KeybindAction::SetFov(_) => "Set Fov",
                    KeybindAction::AdjustFov(_) => "Adjust Fov",
                    KeybindAction::SetGameSpeed(_) => "Set game speed",
                    KeybindAction::AdjustGamespeed(_) => "Adjust game speed",
                    KeybindAction::ToggleDebugPause => "Toggle debug pause",
                    KeybindAction::ToggleFreecam => "Toggle freecam",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::TogglePlaybackMode,
                        "Toggle playback mode",
                    );
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::SetGameSpeed(1.0),
                        "Set game speed",
                    );
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::CreateKeyframe,
                        "Create Keyframe",
                    );
                    ui.selectable_value(&mut bind.action, KeybindAction::ToggleHUD, "Toggle HUD");
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::ToggleCharacterNoDead,
                        "Toggle character no dead",
                    );
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::ToggleCharacterNoMove,
                        "Toggle character no move",
                    );
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::ToggleFovOverride,
                        "Toggle fov override",
                    );
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::SetFov(48.0f32.to_radians()),
                        "Set fov",
                    );
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::AdjustFov(2.0f32.to_radians()),
                        "Adjust fov",
                    );
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::SetGameSpeed(1.0),
                        "Set game speed",
                    );
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::AdjustGamespeed(0.1),
                        "Adjust game speed",
                    );
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::ToggleDebugPause,
                        "Toggle debug pause",
                    );
                    ui.selectable_value(
                        &mut bind.action,
                        KeybindAction::ToggleFreecam,
                        "Toggle freecam",
                    );
                });

            match &mut bind.action {
                KeybindAction::TogglePlaybackMode => {}
                KeybindAction::CreateKeyframe => {}
                KeybindAction::ToggleHUD => {}
                KeybindAction::ToggleCharacterNoDead => {}
                KeybindAction::ToggleCharacterNoMove => {}
                KeybindAction::ToggleFovOverride => {}
                KeybindAction::SetFov(fov) => {
                    drag_angle(ui, "Angle ", fov);
                }
                KeybindAction::AdjustFov(fov) => {
                    drag_angle_signed(ui, "Change in angle ", fov);
                }
                KeybindAction::SetGameSpeed(multiplier) => {
                    drag_percentage(ui, "Game speed ", multiplier, true);
                }
                KeybindAction::AdjustGamespeed(multiplier) => {
                    drag_percentage_delta(ui, "Game speed adjustment ", multiplier, false);
                }
                KeybindAction::ToggleDebugPause => {}
                KeybindAction::ToggleFreecam => {}
            }
        });
    });

    result
}

fn format_input(input: &KeybindInput) -> &str {
    match input {
        KeybindInput::Keyboard(vk) => match vk {
            0x41 => "A",
            0x42 => "B",
            0x43 => "C",
            0x44 => "D",
            0x45 => "E",
            0x46 => "F",
            0x47 => "G",
            0x48 => "H",
            0x49 => "I",
            0x4A => "J",
            0x4B => "K",
            0x4C => "L",
            0x4D => "M",
            0x4E => "N",
            0x4F => "O",
            0x50 => "P",
            0x51 => "Q",
            0x52 => "R",
            0x53 => "S",
            0x54 => "T",
            0x55 => "U",
            0x56 => "V",
            0x57 => "W",
            0x58 => "X",
            0x59 => "Y",
            0x5A => "Z",

            0x60 => "Num0",
            0x61 => "Num1",
            0x62 => "Num2",
            0x63 => "Num3",
            0x64 => "Num4",
            0x65 => "Num5",
            0x66 => "Num6",
            0x67 => "Num7",
            0x68 => "Num8",
            0x69 => "Num9",

            0x1B => "Escape",
            0x0D => "Enter",
            0x09 => "Tab",
            0x08 => "Backspace",
            0x2D => "Insert",
            0x2E => "Delete",
            0x24 => "Home",
            0x23 => "End",
            0x21 => "PageUp",
            0x22 => "PageDown",
            0x25 => "ArrowLeft",
            0x26 => "ArrowUp",
            0x27 => "ArrowRight",
            0x28 => "ArrowDown",
            0x20 => "Space",

            0x70 => "F1",
            0x71 => "F2",
            0x72 => "F3",
            0x73 => "F4",
            0x74 => "F5",
            0x75 => "F6",
            0x76 => "F7",
            0x77 => "F8",
            0x78 => "F9",
            0x79 => "F10",
            0x7A => "F11",
            0x7B => "F12",
            0x7C => "F13",
            0x7D => "F14",
            0x7E => "F15",
            0x7F => "F16",
            0x80 => "F17",
            0x81 => "F18",
            0x82 => "F19",
            0x83 => "F20",
            0x84 => "F21",
            0x85 => "F22",
            0x86 => "F23",
            0x87 => "F24",

            _ => "Unknown",
        },
    }
}

fn egui_key_to_vk(key: egui::Key) -> Option<i32> {
    use egui::Key::*;
    Some(match key {
        // Ordinary keys
        A => 0x41,
        B => 0x42,
        C => 0x43,
        D => 0x44,
        E => 0x45,
        F => 0x46,
        G => 0x47,
        H => 0x48,
        I => 0x49,
        J => 0x4A,
        K => 0x4B,
        L => 0x4C,
        M => 0x4D,
        N => 0x4E,
        O => 0x4F,
        P => 0x50,
        Q => 0x51,
        R => 0x52,
        S => 0x53,
        T => 0x54,
        U => 0x55,
        V => 0x56,
        W => 0x57,
        X => 0x58,
        Y => 0x59,
        Z => 0x5A,

        // Numpad shit
        Num0 => 0x60,
        Num1 => 0x61,
        Num2 => 0x62,
        Num3 => 0x63,
        Num4 => 0x64,
        Num5 => 0x65,
        Num6 => 0x66,
        Num7 => 0x67,
        Num8 => 0x68,
        Num9 => 0x69,

        Enter => 0x0D,
        Tab => 0x09,
        Backspace => 0x08,
        Insert => 0x2D,
        Delete => 0x2E,
        Home => 0x24,
        End => 0x23,
        PageUp => 0x21,
        PageDown => 0x22,
        ArrowLeft => 0x25,
        ArrowUp => 0x26,
        ArrowRight => 0x27,
        ArrowDown => 0x28,
        Space => 0x20,

        // Function keys
        F1 => 0x70,
        F2 => 0x71,
        F3 => 0x72,
        F4 => 0x73,
        F5 => 0x74,
        F6 => 0x75,
        F7 => 0x76,
        F8 => 0x77,
        F9 => 0x78,
        F10 => 0x79,
        F11 => 0x7A,
        F12 => 0x7B,
        F13 => 0x7C,
        F14 => 0x7D,
        F15 => 0x7E,
        F16 => 0x7F,
        F17 => 0x80,
        F18 => 0x81,
        F19 => 0x82,
        F20 => 0x83,
        F21 => 0x84,
        F22 => 0x85,
        F23 => 0x86,
        F24 => 0x87,

        _ => return None,
    })
}
