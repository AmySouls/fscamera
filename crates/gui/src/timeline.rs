use eframe::egui::{Align2, Color32, DragValue, FontId, Key, Pos2, Rect, Sense, Ui, Vec2};
use egui_notify::Toasts;
use protocol::{RemoteError, SettingsData, keyframe::{Keyframe, Quat, Vec3}};

use crate::{controls::{drag_angle, panel_header}, game::RemoteGame};

const TIMELINE_HEIGHT: f32 = 50.0;
const KEYFRAME_CREATION_INTERVAL: f32 = 5.0;

pub struct TimelineControl {
    keyframes: Vec<Keyframe>,
    dragging_index: Option<usize>,
    selected_index: Option<usize>,
    playing: bool,
    time: f32,
    last_update: Option<std::time::Instant>,
    place_keyframes_at_time: bool,
}

impl Default for TimelineControl {
    fn default() -> Self {
        Self {
            keyframes: Vec::new(),
            dragging_index: None,
            selected_index: None,
            playing: false,
            time: 0.0,
            last_update: None,
            place_keyframes_at_time: false,
        }
    }
}

impl TimelineControl {
    pub fn update(&mut self, ui: &mut Ui, remote: &RemoteGame, notify: &mut Toasts, settings: &SettingsData) {
        let now = std::time::Instant::now();
        let delta_seconds = if let Some(last) = self.last_update {
            (now - last).as_secs_f32()
        } else {
            0.0
        };

        if self.playing {
            self.time += delta_seconds;

            if self.time > settings.path_duration {
                self.playing = false;
                self.time = 0.0;

                // TODO: kick back into freecam mode
            }
        }

        let mut flush_keyframes = false;

        ui.vertical(|ui| {
            if self.selected_index.is_some() {
                flush_keyframes |= self.keyframe_controls(ui, remote, notify);
                ui.separator();
            }

            ui.horizontal(|ui| {
                if ui.button("➕ Add Keyframe").clicked() {
                    if let Err(e) = self.create_keyframe(remote, settings) {
                        notify.error(format!("Could not create new keyframe: {e}"));
                    } else {
                        flush_keyframes = true;
                    }
                }

                ui.add_enabled_ui(self.selected_index.is_some(), |ui| {
                    if ui.button("❌ Delete Keyframe").clicked() {
                        self.keyframes.remove(self.selected_index.take().unwrap());

                        if !self.keyframes.is_empty() {
                            self.selected_index = Some(self.keyframes.len() - 1);
                        }
                    }
                });

                ui.checkbox(
                    &mut self.place_keyframes_at_time,
                    "Place keyframes at current playback time",
                );
            });

            // Allocate space for the timeline to be drawn
            let (timeline_rect, timeline_response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), TIMELINE_HEIGHT),
                Sense::click_and_drag(),
            );

            // Painter for painting on the allocated rectangle.
            let painter = ui.painter_at(timeline_rect);

            // Draw background
            // TODO: swap based on dark/light mode
            painter.rect_filled(timeline_rect, 0.0, Color32::DARK_GRAY);

            let timeline_top = timeline_rect.top();
            let timeline_left = timeline_rect.left();
            let pixels_per_second = ui.available_width() / settings.path_duration;

            // Draw second markers
            let total_seconds = settings.path_duration.ceil() as usize;
            for second in 0..=total_seconds {
                let x = timeline_left + second as f32 * pixels_per_second;
                painter.line_segment(
                    [
                        Pos2::new(x, timeline_top),
                        Pos2::new(x, timeline_top + 10.0),
                    ],
                    (1.0, Color32::WHITE),
                );

                let denominator = if settings.path_duration < 120.0 {
                    5
                } else {
                    10
                };

                // Only draw a marker every X seconds
                if second % denominator == 0 {
                    let minute = second / 60;
                    let second = second % 60;

                    painter.text(
                        Pos2::new(x + 2.0, timeline_top + 12.0),
                        Align2::LEFT_TOP,
                        format!("{minute:#01}:{second:#02}"),
                        FontId::monospace(10.0),
                        Color32::WHITE,
                    );
                }
            }

            // Draw keyframe markers
            for (i, keyframe) in self.keyframes.iter_mut().enumerate() {
                let x = timeline_left + keyframe.time * pixels_per_second;
                let color = if self.selected_index == Some(i) {
                    Color32::YELLOW
                } else {
                    Color32::LIGHT_BLUE
                };

                let kf_top = timeline_top + 30.0;
                let kf_bottom = timeline_rect.bottom() - 5.0;

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
                    keyframe.time = keyframe.time.clamp(0.0, settings.path_duration);
                    flush_keyframes = true;
                }
            }

            // Draw playhead
            let playhead_x = timeline_left + self.time * pixels_per_second;
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
                Pos2::new(playhead_x + 4.0, playhead_top + TIMELINE_HEIGHT / 1.5),
                Align2::LEFT_TOP,
                format!("{:2.2}s", self.time),
                FontId::monospace(10.0),
                Color32::RED,
            );

            // Handle scrubbing motions
            if (timeline_response.clicked() || timeline_response.dragged())
                && let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos())
            {
                let relative_x = pointer_pos.x - timeline_rect.left();
                let new_time = (relative_x / pixels_per_second).clamp(0.0, settings.path_duration);
                self.time = new_time;
                self.last_update = Some(std::time::Instant::now());

                remote.set_playback_state(false, self.time);
            }

            // Pause when scrubbing
            if timeline_response.dragged() {
                self.playing = false;
            }

            let input = ui.input(|i| i.clone());

            // // Toggle playback
            // if input.key_pressed(Key::Space) {
            //     self.toggle_playing();
            // }

            if input.key_pressed(Key::Delete) && let Some(selected_index) = self.selected_index.take() {
                self.keyframes.remove(selected_index);

                // Select prev keyframe instead
                if !self.keyframes.is_empty() {
                    self.selected_index = Some(self.keyframes.len() - 1);
                }

                flush_keyframes = true;
            }

            if flush_keyframes && let Err(e) = remote.set_keyframes(self.keyframes.clone()) {
                notify.error(format!("Could not flush keyframes to game: {e}"));
            }
        });

        // if changed
        //     && let Err(e) =
        //         self.remote
        //             .as_ref()
        //             .unwrap()
        //             .post_event(InboundGameControlEvent::Keyframes {
        //                 keyframes: self.keyframes.clone(),
        //             })
        // {
        //     self.notify
        //         .error(format!("Could not send keyframes to game: {e}"));
        // }
    }

    fn keyframe_controls(&mut self, ui: &mut Ui, remote: &RemoteGame, notify: &mut Toasts) -> bool {
        let Some(selected) = self.selected_index else {
            return false;
        };

        let kf = &mut self.keyframes[selected];
        let mut flush_keyframes = false;

        ui.horizontal(|ui| {
            ui.label("Time (seconds):");

            flush_keyframes |= ui.add(DragValue::new(&mut kf.time).speed(0.01)).changed();
        });

        ui.horizontal(|ui| {
            ui.label("Position:");

            flush_keyframes |= ui
                .add(DragValue::new(&mut kf.position.x).speed(0.1).prefix("x: "))
                .changed();
            flush_keyframes |= ui
                .add(DragValue::new(&mut kf.position.y).speed(0.1).prefix("y: "))
                .changed();
            flush_keyframes |= ui
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
            flush_keyframes |= drag_angle(ui, "", &mut kf.fov).changed();
        });

        flush_keyframes
    }

    pub fn set_keyframes(&mut self, keyframes: Vec<Keyframe>) {
        self.keyframes = keyframes;
        self.selected_index = None;
    }

    pub fn keyframes(&self) -> Vec<Keyframe> {
        self.keyframes.clone()
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    pub fn time(&self) -> f32 {
        self.time
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    fn toggle_playing(&mut self) {
        self.playing = !self.playing;
    }

    pub fn create_keyframe(&mut self, remote: &RemoteGame, settings: &SettingsData) -> Result<(), RemoteError> {
        let time = if !self.place_keyframes_at_time && let Some(selected) = self.selected_index {
            self.keyframes[selected].time + settings.time_between_created_keyframes
        } else {
            self.time
        };

        let camera_state = remote.snapshot_camera_state()?;
        let pos = camera_state.position;
        let rot = camera_state.orientation;

        let kf = Keyframe {
            time,
            map_id: camera_state.map_id,
            position: Vec3::new(pos.x, pos.y, pos.z),
            orientation: Quat(rot.0, rot.1, rot.2, rot.3),
            fov: camera_state.fov,
        };

        self.keyframes.push(kf);
        self.selected_index = Some(self.keyframes.len() - 1);

        Ok(())
    }
}
