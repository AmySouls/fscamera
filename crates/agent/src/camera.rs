use std::{sync::atomic::Ordering, time::Duration};

use eldenring_util::{input};
use fromsoft_shared::{F32Vector4, OwnedPtr, Program, get_instance};
use pelite::pe::Pe;
use protocol::{
    InboundGameControlEvent, KeybindInput, OutboundGameControlEvent, SettingsData,
    keyframe::{Keyframe, Vec3},
};

use crate::{
    DEBUG_PAUSE_ENABLED,
    DISABLE_HUD,
    OUTBOUND_EVENT_QUEUE,
    // freecam::{FORWARD, Freecam, RIGHT, UP},
    game::{CSCamera, CSFlipperImp, FieldArea, FreecamMode, GameOffsets, WorldAreaTime, WorldChrMan},
    get_offsets,
    input::Input
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

    // freecam: FreeCam,
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

            // freecam: Default::default(),
        }
    }
}

impl CameraManager {
    pub fn handle_event(&mut self, event: &InboundGameControlEvent) {
        // match event {
        //     InboundGameControlEvent::Initialize { settings } => {
        //         self.keyframes = vec![];
        //         self.active = false;
        //         self.playing = false;
        //         self.time_multiplier = 1.0;
        //         self.settings = settings.clone();
        //     }
        //     InboundGameControlEvent::Settings { settings } => {
        //         self.settings = settings.clone();
        //     }
        //     InboundGameControlEvent::Keyframes { keyframes } => {
        //         self.keyframes = keyframes.clone();
        //     }
        //     InboundGameControlEvent::Play { time, keyframes } => {
        //         self.play_playback(*time, keyframes)
        //     }
        //     InboundGameControlEvent::Pause { time } => self.pause_playback(*time),
        //     InboundGameControlEvent::Scrub { time } => self.scrub_playback(*time),
        //     InboundGameControlEvent::PlaybackModeState { state } => self.set_playback_state(*state),
        //     InboundGameControlEvent::TimeMultiplier { multiplier } => {
        //         self.time_multiplier = *multiplier
        //     }
        //     InboundGameControlEvent::GlobalFov { fov, enabled } => {
        //         self.global_fov = *fov;
        //         self.override_fov = *enabled;
        //     }
        //     InboundGameControlEvent::RequestTimeOfDay {
        //         hours,
        //         minutes,
        //         seconds,
        //     } => {
        //         let world_area_time = unsafe { get_instance::<WorldAreaTime>() };
        //         let Some(world_area_time) = world_area_time
        //         else {
        //             return;
        //         };
        //
        //         world_area_time.request_hour = *hours as _;
        //         world_area_time.request_minute = *minutes as _;
        //         world_area_time.request_second = *seconds as _;
        //     }
        //     InboundGameControlEvent::SetHudDisabled { hidden } => {
        //         DISABLE_HUD.store(*hidden, Ordering::Relaxed);
        //     }
        //     InboundGameControlEvent::SetCharacterNoDead { value } => {
        //         self.character_no_dead = *value;
        //     }
        //     InboundGameControlEvent::SetCharacterNoMove { value } => {
        //         self.character_no_move = *value;
        //     }
        //     InboundGameControlEvent::SetFreecamMovementSpeed { value } => {
        //         self.freecam.set_movement_speed_multiplier(*value);
        //     }
        //     InboundGameControlEvent::SetFreecamRotationSpeed { value } => {
        //         self.freecam.set_rotation_speed_multiplier(*value);
        //     }
        //     InboundGameControlEvent::SetDebugPause { enabled } => {
        //         DEBUG_PAUSE_ENABLED.store(*enabled, Ordering::Relaxed);
        //     }
        //     InboundGameControlEvent::SetFreecamEnabled { enabled } => {
        //         self.freecam.set_enabled(*enabled);
        //         self.character_no_move = true;
        //     }
        // }
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

        // if self.active {
        //     // Test sent keyframe
        //     if let Some(kf) = self.keyframes.first()
        //         && let Some(wbi) = field_area.world_info_owner.world_block_info_by_map(&kf.map_id.into()) {
        //         let q_pitch = glm::quat_angle_axis(kf.orientation.0, &RIGHT);
        //         let q_yaw = glm::quat_angle_axis(kf.orientation.1, &UP);
        //         let q_roll = glm::quat_angle_axis(kf.orientation.2, &FORWARD);
        //         let rot = glm::quat_to_mat3(&(q_roll * q_pitch * q_yaw).normalize());
        //
        //         // Build up new matrix to swap the primary view matrix with.
        //         camera.pers_cam_1.matrix.0 = F32Vector4(rot.m11, rot.m12, rot.m13, 0.0);
        //         camera.pers_cam_1.matrix.1 = F32Vector4(rot.m21, rot.m22, rot.m23, 0.0);
        //         camera.pers_cam_1.matrix.2 = F32Vector4(rot.m31, rot.m32, rot.m33, 0.0);
        //         camera.pers_cam_1.matrix.3 = F32Vector4(
        //             kf.position.x + wbi.physics_center.0,
        //             kf.position.y + wbi.physics_center.1,
        //             kf.position.z + wbi.physics_center.2,
        //             1.0,
        //         );
        //         camera.pers_cam_1.fov = kf.fov;
        //     }
        //
        //     // Map keyframes to usable format
        //     // TODO: we can avoid doing this every frame.
        //     // let playback_frames = self
        //     //     .keyframes
        //     //     .iter()
        //     //     .map(|kf| {
        //     //         let mut position = Vec3::new(0.0, 0.0, 0.0);
        //     //         if let Some(world_block_info) = field_area
        //     //             .world_info_owner
        //     //             .world_block_info_by_map(&kf.map_id.into())
        //     //         {
        //     //             position = Vec3::new(
        //     //                 world_block_info.physics_center.0 + kf.position.x,
        //     //                 world_block_info.physics_center.1 + kf.position.y,
        //     //                 world_block_info.physics_center.2 + kf.position.z,
        //     //             );
        //     //         }
        //     //
        //     //         PlaybackFrame {
        //     //             time: kf.time,
        //     //             position,
        //     //             orientation: kf.orientation,
        //     //             fov: kf.fov,
        //     //             tension: kf.tension,
        //     //         }
        //     //     })
        //     //     .collect::<Vec<_>>();
        //
        //     // if let Some((position, rotation, fov)) =
        //     //     PlaybackFrame::interpolate(&playback_frames, self.playback_time)
        //     // {
        //     //     let rotation = glm::quat_to_mat4(&glm::make_quat(&[
        //     //         rotation.0, rotation.1, rotation.2, rotation.3,
        //     //     ]));
        //     //
        //     //     // Build up new matrix to swap the primary view matrix with.
        //     //     camera.pers_cam_1.matrix.0 =
        //     //         F32Vector4(rotation.m11, rotation.m12, rotation.m13, rotation.m14);
        //     //     camera.pers_cam_1.matrix.1 =
        //     //         F32Vector4(rotation.m21, rotation.m22, rotation.m23, rotation.m24);
        //     //     camera.pers_cam_1.matrix.2 =
        //     //         F32Vector4(rotation.m31, rotation.m32, rotation.m33, rotation.m34);
        //     //     camera.pers_cam_1.matrix.3 = F32Vector4(position.x, position.y, position.z, 1.0);
        //     //     camera.pers_cam_1.fov = fov;
        //     // }
        // }

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

        // self.freecam.apply();

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

    pub fn update(&mut self, input: &Input, delta: &Duration) {
        // self.freecam.update(input, delta);

        if self.playing {
            self.playback_time += delta.as_secs_f32();
        }
    }
}
