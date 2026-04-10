use eframe::egui::{Align, Align2, Color32, DragValue, FontId, Key, Layout, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use egui_notify::Toasts;
use protocol::{RemoteError, keyframe::{Keyframe, Quat, Vec3}};

use crate::{controls::drag_fov, game::RemoteGame};

const TIMELINE_HEIGHT: f32 = 50.0;

pub struct TimelineControl {
    keyframes: Vec<Keyframe>,
    dragging_index: Option<usize>,
    selected_index: Option<usize>,
    playing: bool,
    time: f32,
    path_duration: f32,
    last_update: Option<std::time::Instant>,
}

pub enum TimelineControlCommand {
    PlaybackDone,
    Scrub,
}

impl TimelineControl {
    pub fn new() -> Self {
        Self {
            keyframes: Vec::new(),
            dragging_index: None,
            selected_index: None,
            playing: false,
            time: 0.0,
            path_duration: 30.0,
            last_update: None,
        }
    }

    pub fn update(
        &mut self,
        ui: &mut Ui,
        remote: &RemoteGame,
        notify: &mut Toasts,
    ) -> Option<TimelineControlCommand> {
        let mut result = None;

        let now = std::time::Instant::now();
        let delta_seconds = if let Some(last) = self.last_update {
            (now - last).as_secs_f32()
        } else {
            0.0
        };

        if self.playing {
            self.time += delta_seconds;
            self.last_update = Some(now);

            let end = self.keyframes.last().map(|k| k.time).unwrap_or_default();
            if self.time > end {
                self.playing = false;
                self.time = 0.0;

                result = Some(TimelineControlCommand::PlaybackDone);
            }
        } else {
            self.last_update = None;
        }

        let mut flush_keyframes = false;
        ui.vertical(|ui| {
            if self.selected_index.is_some() {
                flush_keyframes |= self.keyframe_controls(ui);
                ui.separator();
            }

            ui.horizontal(|ui| {
                if ui.button("➕ Add Keyframe").clicked() {
                    if let Err(e) = self.create_keyframe(remote) {
                        notify.error(format!("Could not create new keyframe: {e}"));
                    } else {
                        flush_keyframes = true;
                    }
                }

                ui.add_enabled_ui(self.selected_index.is_some(), |ui| {
                    if ui.button("❌ Delete Keyframe").clicked() {
                        self.keyframes.remove(self.selected_index.take().unwrap());

                        self.spread_frames();

                        if !self.keyframes.is_empty() {
                            self.selected_index = Some(self.keyframes.len() - 1);
                        }
                    }
                });

                let original_path_duration = self.path_duration;
                ui.with_layout(
                    Layout::default().with_cross_align(Align::RIGHT),
                    |ui| {
                        ui.horizontal(|ui| {
                            if ui.add(
                                DragValue::new(&mut self.path_duration)
                                .speed(1.0)
                                .suffix("s")
                                .range(1.00..=240.0),
                            ).changed() {
                                self.spread_frames();
                                flush_keyframes = true;

                                // Correct playback time to remain at the same point relative to the keyframes.
                                let ratio = self.path_duration / original_path_duration;
                                self.time *= ratio;
                            }

                            ui.label("Path duration");
                        });
                    },
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
            let pixels_per_second = ui.available_width() / self.path_duration;

            // Draw second markers
            let total_seconds = self.path_duration.ceil() as usize;
            for second in 0..=total_seconds {
                let x = timeline_left + second as f32 * pixels_per_second;
                painter.line_segment(
                    [
                        Pos2::new(x, timeline_top),
                        Pos2::new(x, timeline_top + 10.0),
                    ],
                    (1.0, Color32::WHITE),
                );

                let denominator = if self.path_duration < 120.0 {
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

            for (i, keyframe) in self.keyframes.iter_mut().enumerate() {
                let x = timeline_left + keyframe.time * pixels_per_second;
                let color = if self.selected_index == Some(i) {
                    Color32::YELLOW
                } else {
                    Color32::LIGHT_BLUE
                };

                let kf_top = timeline_top + 30.0;
                // let kf_bottom = timeline_rect.bottom() - 5.0;

                // let line_rect =
                //     Rect::from_min_max(Pos2::new(x - 4.0, kf_top), Pos2::new(x + 4.0, kf_bottom));
                // let response = ui.allocate_rect(line_rect, Sense::click_and_drag());
                // painter.rect_filled(line_rect, 2.0, color);
                let size = 6.0;
                let center = Pos2::new(x, kf_top);

                let points = vec![
                    Pos2::new(center.x, center.y - size), // top
                    Pos2::new(center.x + size, center.y), // right
                    Pos2::new(center.x, center.y + size), // bottom
                    Pos2::new(center.x - size, center.y), // left
                ];

                let response = ui.allocate_rect(
                    Rect::from_center_size(center, Vec2::splat(size * 2.0)),
                    Sense::click_and_drag(),
                );

                painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
                // painter.circle_filled(center, radius, color);

                if response.clicked() {
                    self.selected_index = Some(i);
                }

                if response.drag_started() {
                    self.dragging_index = Some(i);
                }

                if response.dragged() && self.dragging_index == Some(i) {
                    keyframe.time += response.drag_delta().x / pixels_per_second;
                    keyframe.time = keyframe.time.clamp(0.0, self.path_duration);
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
                let new_time = (relative_x / pixels_per_second).clamp(0.0, self.path_duration);
                self.time = new_time;
                self.playing = false;
                if let Err(e) = remote.set_playback_state(false, self.time) {
                    notify.error(format!("Could not send scrub command to game: {e}"));
                }

                result = Some(TimelineControlCommand::Scrub);
            }

            let input = ui.input(|i| i.clone());
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

        result
    }

    fn keyframe_controls(
        &mut self,
        ui: &mut Ui,
    ) -> bool {
        let Some(selected) = self.selected_index else {
            return false;
        };

        let kf = &mut self.keyframes[selected];
        let mut flush_keyframes = false;

        ui.horizontal(|ui| {
            ui.label("Time (seconds)");

            flush_keyframes |= ui.add(
                DragValue::new(&mut kf.time)
                    .speed(0.01)
                    .range(0.0..=self.path_duration)
            ).changed();
        });

        ui.horizontal(|ui| {
            ui.label("Position (xyz)");

            flush_keyframes |= ui
                .add(DragValue::new(&mut kf.position.x).speed(0.1))
                .changed();
            flush_keyframes |= ui
                .add(DragValue::new(&mut kf.position.y).speed(0.1))
                .changed();
            flush_keyframes |= ui
                .add(DragValue::new(&mut kf.position.z).speed(0.1))
                .changed();
        });

        ui.horizontal(|ui| {
            ui.label("Rotation (ypr)".to_string());

            let q = glam::quat(
                kf.orientation.0,
                kf.orientation.1,
                kf.orientation.2,
                kf.orientation.3,
            );
            let (mut yaw, mut pitch, mut roll) = q.to_euler(glam::EulerRot::YXZ);

            let mut changed_orientation = false;
            changed_orientation |= ui.drag_angle(&mut yaw).changed();
            changed_orientation |= ui.drag_angle(&mut pitch).changed();
            changed_orientation |= ui.drag_angle(&mut roll).changed();

            if changed_orientation {
                let q = glam::Quat::from_euler(
                    glam::EulerRot::YXZ,
                    yaw,
                    pitch,
                    roll,
                );

                kf.orientation = protocol::keyframe::Quat(q.x, q.y, q.z, q.w);
                flush_keyframes = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("FoV");
            flush_keyframes |= drag_fov(ui, "", &mut kf.fov).changed();
        });

        flush_keyframes
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
        self.time = 0.0;
        self.playing = true;
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    pub fn create_keyframe(
        &mut self,
        remote: &RemoteGame,
    ) -> Result<(), RemoteError> {
        let camera_state = remote.snapshot_camera_state()?;
        let pos = camera_state.position;
        let rot = camera_state.orientation;

        let mut kf = Keyframe {
            time: 0.0,
            position: Vec3::new(pos.x, pos.y, pos.z),
            orientation: Quat(rot.0, rot.1, rot.2, rot.3),
            fov: camera_state.fov,
        };

        // Assign temp time to ensure we get sorted as last entry.
        kf.time = f32::MAX;
        self.keyframes.push(kf);
        self.selected_index = Some(self.keyframes.len() - 1);
        self.spread_frames();

        Ok(())
    }

    fn spread_frames(&mut self) {
        self.keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));

        // Calculate required time between frames.  
        let spacing = self.path_duration / (self.keyframes.len().max(1) - 1) as f32;

        let mut current_time = 0.0;
        for kf in self.keyframes.iter_mut() {
            kf.time = current_time;
            current_time += spacing;
        }
    }
}
