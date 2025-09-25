use std::{sync::atomic::Ordering, time::Duration};

use eldenring_util::{input};
use fromsoft_shared::{F32Vector4, OwnedPtr, Program, get_instance};
use nalgebra_glm as glm;
use pelite::pe::Pe;
use protocol::{
    InboundGameControlEvent, KeybindInput, OutboundGameControlEvent, SettingsData,
    keyframe::{Keyframe, Quat, Vec3},
};

use crate::{
    game::{CSCamera, CSFlipperImp, FieldArea, FreecamMode, GameOffsets, WorldAreaTime, WorldChrMan}, get_offsets, DISABLE_HUD, OUTBOUND_EVENT_QUEUE
};

pub(crate) struct CameraManager {
    time_multiplier: f32,

    global_fov: f32,
    override_fov: bool,
    // Playback time
    playback_time: f32,
    // Are we playing the playback
    playing: bool,
    // Do we have the camera attached
    active: bool,
    // Keyframes sent in from the GUI
    keyframes: Vec<Keyframe>,
    settings: SettingsData,

    // Character control stuff
    character_no_dead: bool,
    character_no_move: bool,
}

impl Default for CameraManager {
    fn default() -> Self {
        Self {
            time_multiplier: 1.0,
            global_fov: 48.0f32.to_radians(),
            override_fov: false,
            playback_time: Default::default(),
            playing: Default::default(),
            active: Default::default(),
            keyframes: Default::default(),
            settings: Default::default(),
            character_no_dead: false,
            character_no_move: false,
        }
    }
}

impl CameraManager {
    pub fn handle_event(&mut self, event: &InboundGameControlEvent) {
        match event {
            InboundGameControlEvent::Initialize { settings } => {
                self.keyframes = vec![];
                self.active = false;
                self.playing = false;
                self.time_multiplier = 1.0;
                self.settings = settings.clone();
            }
            InboundGameControlEvent::Settings { settings } => {
                self.settings = settings.clone();
            }
            InboundGameControlEvent::Keyframes { keyframes } => {
                self.keyframes = keyframes.clone();
            }
            InboundGameControlEvent::Play { time, keyframes } => {
                self.play_playback(*time, keyframes)
            }
            InboundGameControlEvent::Pause { time } => self.pause_playback(*time),
            InboundGameControlEvent::Scrub { time } => self.scrub_playback(*time),
            InboundGameControlEvent::PlaybackModeState { state } => self.set_playback_state(*state),
            InboundGameControlEvent::TimeMultiplier { multiplier } => {
                self.time_multiplier = *multiplier
            }
            InboundGameControlEvent::GlobalFov { fov, enabled } => {
                self.global_fov = *fov;
                self.override_fov = *enabled;
            }
            InboundGameControlEvent::RequestTimeOfDay {
                hours,
                minutes,
                seconds,
            } => {
                let Some(world_area_time) = (unsafe { get_instance::<WorldAreaTime>() })
                else {
                    return;
                };

                world_area_time.request_hour = *hours as _;
                world_area_time.request_minute = *minutes as _;
                world_area_time.request_second = *seconds as _;
            }
            InboundGameControlEvent::HudState { hidden } => {
                DISABLE_HUD.store(*hidden, Ordering::Relaxed);
            }
            InboundGameControlEvent::SetCharacterNoDead { value } => {
                self.character_no_dead = *value;
            }
            InboundGameControlEvent::SetCharacterNoMove { value } => {
                self.character_no_move = *value;
            }
        }
    }

    fn set_playback_state(&mut self, state: bool) {
        self.playing = false;
        self.active = state;

        if state && self.settings.enabling_playback_disables_freecam {
            Self::disable_freecam();
        }
    }

    fn play_playback(&mut self, time: f32, keyframes: &[Keyframe]) {
        self.playback_time = time;
        self.keyframes = keyframes.to_vec();
        self.playing = true;
        self.active = true;

        if self.settings.enabling_playback_disables_freecam {
            Self::disable_freecam();
        }
    }

    fn disable_freecam() {
        let program = Program::current();
        let offsets = get_offsets(&program).unwrap();

        let mut field_area = unsafe {
            std::mem::transmute::<u64, OwnedPtr<Option<OwnedPtr<FieldArea>>>>(
                program.rva_to_va(offsets.field_area).unwrap(),
            )
        };
        let Some(field_area) = field_area.as_mut() else {
            return;
        };

        let field_area = field_area.as_mut();
        field_area.game_rend.freecam_mode = FreecamMode::Inactive;
    }

    fn pause_playback(&mut self, time: f32) {
        self.playback_time = time;
        self.playing = false;
    }

    fn scrub_playback(&mut self, time: f32) {
        self.playback_time = time;
        self.playing = false;
        self.active = true;
    }

    pub fn apply(
        &mut self,
        offsets: &GameOffsets,
        camera: &mut CSCamera,
        field_area: &FieldArea,
        flipper: &mut CSFlipperImp,
        world_chr_man: &mut WorldChrMan,
    ) {
        for bind in self.settings.keybinds.iter().filter(|b| b.active) {
            if let Some(KeybindInput::Keyboard(key)) = bind.input
                && input::is_key_pressed(key)
            {
                OUTBOUND_EVENT_QUEUE
                    .push(OutboundGameControlEvent::KeybindAction(bind.action.clone()));
            }
        }

        if self.active {
            // Map keyframes to usable format
            // TODO: we can avoid doing this every frame.
            let playback_frames = self
                .keyframes
                .iter()
                .map(|kf| {
                    let mut position = Vec3::new(0.0, 0.0, 0.0);
                    if let Some(world_block_info) = field_area
                        .world_info_owner
                        .world_block_info_by_map(&kf.map_id.into())
                    {
                        position = Vec3::new(
                            world_block_info.physics_center.0 + kf.position.x,
                            world_block_info.physics_center.1 + kf.position.y,
                            world_block_info.physics_center.2 + kf.position.z,
                        );
                    }

                    PlaybackFrame {
                        time: kf.time,
                        position,
                        orientation: kf.orientation,
                        fov: kf.fov,
                        tension: kf.tension,
                    }
                })
                .collect::<Vec<_>>();

            if let Some((position, rotation, fov)) =
                PlaybackFrame::interpolate(&playback_frames, self.playback_time)
            {
                let rotation = glm::quat_to_mat4(&glm::make_quat(&[
                    rotation.0, rotation.1, rotation.2, rotation.3,
                ]));

                // Build up new matrix
                camera.pers_cam_1.matrix.0 =
                    F32Vector4(rotation.m11, rotation.m12, rotation.m13, rotation.m14);
                camera.pers_cam_1.matrix.1 =
                    F32Vector4(rotation.m21, rotation.m22, rotation.m23, rotation.m24);
                camera.pers_cam_1.matrix.2 =
                    F32Vector4(rotation.m31, rotation.m32, rotation.m33, rotation.m34);
                camera.pers_cam_1.matrix.3 = F32Vector4(position.x, position.y, position.z, 1.0);
                camera.pers_cam_1.fov = fov;
            }
        }

        if !self.active && self.override_fov {
            camera.pers_cam_1.fov = self.global_fov;
        }

        if let Some(player) = world_chr_man.main_player.as_mut() {
            let mut chr_control_flags = player.chr_ctrl.flags & 0b11011111;
            if !self.character_no_move {
                chr_control_flags |= 0b00100000;
            }

            player.chr_ctrl.flags = chr_control_flags;
            let no_dead: *mut bool = Program::current().rva_to_va(offsets.no_dead_flag).unwrap() as _;
            unsafe { *no_dead = self.character_no_dead };
        }

        // Apply time multiplier appropriate to settings
        match (
            self.settings.apply_gamespeed_only_when_playback_mode_active,
            self.active,
        ) {
            (true, false) => flipper.time_multiplier = 1.0,
            (true, true) => flipper.time_multiplier = self.time_multiplier,
            (false, _) => flipper.time_multiplier = self.time_multiplier,
        }
    }

    pub fn update(&mut self, delta: &Duration) {
        if self.playing {
            self.playback_time += delta.as_secs_f32();
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlaybackFrame {
    pub time: f32,
    pub position: Vec3,
    pub orientation: Quat,
    pub fov: f32,
    pub tension: f32,
}

pub trait KeyframeInterpolate {
    fn interpolate(frames: &[PlaybackFrame], time: f32) -> Option<(Vec3, Quat, f32)>;
}

impl KeyframeInterpolate for PlaybackFrame {
    fn interpolate(frames: &[PlaybackFrame], time: f32) -> Option<(Vec3, Quat, f32)> {
        let len = frames.len();
        if len < 2 {
            return None;
        }

        // Handle before/after range
        if time <= frames[0].time {
            let f = &frames[0];
            return Some((f.position, f.orientation, f.fov));
        }
        if time >= frames[len - 1].time {
            let f = &frames[len - 1];
            return Some((f.position, f.orientation, f.fov));
        }

        // Find segment
        let (i1, i2) = frames.windows(2).enumerate().find_map(|(i, w)| {
            if time >= w[0].time && time <= w[1].time {
                Some((i, i + 1))
            } else {
                None
            }
        })?;

        let f0 = if i1 > 0 { &frames[i1 - 1] } else { &frames[i1] };
        let f1 = &frames[i1];
        let f2 = &frames[i2];
        let f3 = if i2 + 1 < len {
            &frames[i2 + 1]
        } else {
            &frames[i2]
        };

        let t = (time - f1.time) / (f2.time - f1.time);

        let pos = hermite_position(f0, f1, f2, f3, t, f1.tension);
        let rot = if len > 3 {
            interpolate_rotation(f0, f1, f2, f3, t)
        } else {
            Quat::from_glm(glm::quat_slerp(
                &f1.orientation.to_glm(),
                &f2.orientation.to_glm(),
                t,
            ))
        };
        let fov = lerp(f1.fov, f2.fov, t);

        Some((Vec3::from_glm(pos), rot, fov))
    }
}

fn hermite_position(
    f0: &PlaybackFrame,
    f1: &PlaybackFrame,
    f2: &PlaybackFrame,
    f3: &PlaybackFrame,
    t: f32,
    tension: f32,
) -> glm::Vec3 {
    let p0 = f0.position.to_glm();
    let p1 = f1.position.to_glm();
    let p2 = f2.position.to_glm();
    let p3 = f3.position.to_glm();

    // Tangents (can adjust tension)
    let m1 = (p2 - p0) * 0.5 * (1.0 - tension);
    let m2 = (p3 - p1) * 0.5 * (1.0 - tension);

    let t2 = t * t;
    let t3 = t2 * t;

    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;

    h00 * p1 + h10 * m1 + h01 * p2 + h11 * m2
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn squad_tangent(q_prev: glm::Quat, q: glm::Quat, q_next: glm::Quat) -> glm::Quat {
    let inv_q = glm::quat_inverse(&q);
    let log1 = glm::quat_log(&(inv_q * q_prev));
    let log2 = glm::quat_log(&(inv_q * q_next));
    q * glm::quat_exp(&((-0.25) * (log1 + log2)))
}

fn squad(q1: glm::Quat, q2: glm::Quat, s1: glm::Quat, s2: glm::Quat, t: f32) -> glm::Quat {
    let slerp_1 = glm::quat_slerp(&q1, &q2, t);
    let slerp_2 = glm::quat_slerp(&s1, &s2, t);
    glm::quat_slerp(&slerp_1, &slerp_2, 2.0 * t * (1.0 - t))
}

fn interpolate_rotation(
    f0: &PlaybackFrame,
    f1: &PlaybackFrame,
    f2: &PlaybackFrame,
    f3: &PlaybackFrame,
    t: f32,
) -> Quat {
    let q1 = f1.orientation.to_glm();
    let mut q2 = f2.orientation.to_glm();

    // Always ensure q2 is in the same hemisphere as q1
    q2 = ensure_shortest_path(q1, q2);

    // If q0 == q1 or we're at the start, fallback to SLERP
    let use_slerp = f0.time == f1.time;

    if use_slerp {
        let rot = glm::quat_slerp(&q1, &q2, t);
        return Quat::from_glm(rot);
    }

    // Normal case: use SQUAD
    let mut q0 = f0.orientation.to_glm();
    let mut q3 = f3.orientation.to_glm();
    q0 = ensure_shortest_path(q1, q0);
    q3 = ensure_shortest_path(q2, q3);

    let s1 = squad_tangent(q0, q1, q2);
    let s2 = squad_tangent(q1, q2, q3);

    let result = squad(q1, q2, s1, s2, t);
    Quat::from_glm(result)
}

fn ensure_shortest_path(q1: glm::Quat, q2: glm::Quat) -> glm::Quat {
    if glm::quat_dot(&q1, &q2) < 0.0 {
        -q2
    } else {
        q2
    }
}
