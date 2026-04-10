use std::mem::transmute;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use std::time::Instant;

use camera::freecam::FreeCam;
use camera::playback::PlaybackCam;
use camera::Camera;
use crossbeam::queue::SegQueue;
use fromsoftware_shared::F32Matrix4x4;
use fromsoftware_shared::F32ModelMatrix;
use fromsoftware_shared::F32Vector4;
use fromsoftware_shared::FromStatic;
use fromsoftware_shared::{arxan, OwnedPtr, Program, SharedTaskImpExt};
use game::get_offsets;
use game_compat::CSFlipperImp;
use game_compat::MoveMapStep;
use glam::Mat3;
use glam::Vec3;
use log::LevelFilter;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};
use game_compat::{
    CSCamera, CSPersCam, CSTaskGroupIndex, CSTaskImp, WorldChrMan, FD4TaskData
};
#[cfg(not(feature = "darksouls3"))]
use game_compat::WorldAreaTime;

use pelite::pe64::Pe;
use protocol::AgentState;
use protocol::CameraMode;
use protocol::InboundGameControlEvent;
use protocol::OutboundGameControlEvent;
use protocol::PlaybackSettingsData;
use protocol::{CameraState, RemoteError, SettingsData};

use retour::static_detour;

use crate::gamespeed::GameSpeed;
use crate::input::{Input, InputBlockMode};
use crate::keybind::Keybinds;
use crate::player::Player;

mod game;
mod gamespeed;
mod input;
mod keybind;
mod player;
mod game_compat;

const SPACE: Mat3 = Mat3::from_cols(Vec3::X, Vec3::Y, Vec3::Z);

dll_syringe::payload_procedure! {
    fn snapshot_camera_state() -> Result<CameraState, RemoteError> {
        let Ok(cs_camera) = (unsafe { CSCamera::instance() }) else {
            return Err(RemoteError::AcquireCSCamera);
        };

        let position = cs_camera.pers_cam_1.matrix.3;
        let F32ModelMatrix(rx, ry, rz, _) = cs_camera.pers_cam_1.matrix;

        let rot = glam::Mat3::from_cols(
            glam::Vec3::new(rx.0, rx.1, rx.2),
            glam::Vec3::new(ry.0, ry.1, ry.2),
            glam::Vec3::new(rz.0, rz.1, rz.2),
        );
        let [qx, qy, qz, qw] = glam::Quat::from_mat3(&rot).to_array();

        Ok(CameraState {
            position: protocol::keyframe::Vec3::new(
                position.0,
                position.1,
                position.2,
            ),
            orientation: protocol::keyframe::Quat(qx, qy, qz, qw),
            fov: cs_camera.pers_cam_1.fov,
        })
    }
}

dll_syringe::payload_procedure! {
    fn get_agent_state() -> Result<AgentState, RemoteError> {
        Ok(AgentState {
            camera_mode: CameraMode::Playback,
            gamespeed_enabled: GAMESPEED_ENABLED.load(Ordering::Relaxed),
            debug_pause_enabled: DEBUG_PAUSE_ENABLED.load(Ordering::Relaxed),
            hud_disabled: DISABLE_HUD.load(Ordering::Relaxed),
            freecam_locked: FREECAM_LOCKED.load(Ordering::Relaxed),
        })
    }
}

dll_syringe::payload_procedure! {
    fn post_event(event: InboundGameControlEvent) -> Result<(), RemoteError> {
        INBOUND_EVENT_QUEUE.push(event);
        Ok(())
    }
}

dll_syringe::payload_procedure! {
    fn poll_events() -> Vec<OutboundGameControlEvent> {
        let mut results = Vec::with_capacity(20);

        while let Some(item) = OUTBOUND_EVENT_QUEUE.pop() {
            results.push(item);
        };

        results
    }
}

dll_syringe::payload_procedure! {
    // TODO: clean me up please
    fn initialize(settings: SettingsData) -> Result<(), RemoteError> {
        let offsets = get_offsets().map_err(|e| e.clone())?;
        let program = Program::current();

        if INITIALIZED.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).is_err() {
            INBOUND_EVENT_QUEUE.push(InboundGameControlEvent::Initialize { settings });
            return Ok(())
        }

        let logfile = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
            .build("./log/camera.log")
            .unwrap();

        let config = Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .build(
                Root::builder().appender("logfile")
                .build(LevelFilter::Info),
            )
            .unwrap();

        LOG_HANDLE.set(log4rs::init_config(config).unwrap()).unwrap();
        log_panics::init();

        let mut input = input::Input::default();
        let mut keybinds = Keybinds::from_mapping(settings.keybinds.clone());

        let cs_task = unsafe { CSTaskImp::instance() }.unwrap();

        // Keep track of delta time between task execution as we'll be messing with the one offered
        // by the game.
        let mut last_update = Instant::now();

        let mut camera_mode = CameraMode::Game;
        let mut freecam = FreeCam::new(
            SPACE,
            glam::Vec3::ZERO,
            glam::Quat::IDENTITY,
            0.0,
            48.0f32.to_radians(),
        );

        let mut playback = PlaybackCam::new(
            SPACE,
            glam::Vec3::ZERO,
            glam::Quat::IDENTITY,
            48.0f32.to_radians(),
        );

        let mut playback_settings = PlaybackSettingsData::default();
        let mut player = Player::default();
        let mut gamespeed = GameSpeed::default();

        unsafe { input::setup_hook() };

        {
            #[cfg(feature = "darksouls3")]
            let task_group = CSTaskGroupIndex::CameraStep;
            #[cfg(not(feature = "darksouls3"))]
            let task_group = CSTaskGroupIndex::Draw_Pre;

            // Hijack camera matrix by enqueueing a task to happen right before the draw happens.
            // It's very important that we do this before the draw and after the OG camera update.
            cs_task.run_recurring(move |_: &FD4TaskData| {
                // Keep track of our own delta time as the game speed override influences the one
                // coming in with the task data.
                let now = Instant::now();
                let delta = now - last_update;
                last_update = now;

                // Acquire a pile of shit from the game we can't really live without.

                // Camera's isn't necessarily there and we've got nothing to do in such a situation.
                let Ok(cs_camera) = (unsafe { CSCamera::instance() }) else {
                    return;
                };

                // WorldChrMan is responsible for managing characters including our main player.
                // Our main player is required for a few of the other patches as well as figuring
                // out what map to use as a base for our coordinate conversions.
                let Ok(world_chr_man) = (unsafe { WorldChrMan::instance() }) else {
                    return;
                };

                // Flipper is responsible keeping track of the delta time between frames.
                // We need it to mess with the game speed.
                let Ok(flipper) = (unsafe { CSFlipperImp::instance() }) else {
                    return;
                };

                gamespeed.set_playback_multiplier_enabled(
                    playback_settings.gamespeed_enabled_on_playback && camera_mode == CameraMode::Playback
                );

                #[cfg(not(feature = "darksouls3"))]
                gamespeed.apply(flipper);

                // Update input state for reading.
                input.update();

                // Apply no dead and no move if enabled.
                player.apply(world_chr_man);

                match camera_mode {
                    CameraMode::Game => {},
                    CameraMode::Freecam => {
                        keybinds.execute_freecam_bindings(&input);

                        let freecam_input = keybinds.make_freecam_input(&input);
                        freecam.update(&freecam_input, delta.as_secs_f32());

                        if freecam_input.fov_delta != 0.0 {
                            OUTBOUND_EVENT_QUEUE.push(OutboundGameControlEvent::UpdateFreecamFov {
                                fov: freecam.camera.fov,
                            });
                        }

                        apply_camera_to_game_camera(
                            &freecam.camera,
                            &mut cs_camera.pers_cam_1,
                        );
                    },
                    CameraMode::Playback => {
                        playback.update(delta.as_secs_f32());

                        apply_camera_to_game_camera(
                            &playback.camera,
                            &mut cs_camera.pers_cam_1,
                        );
                    },
                }

                // Handle incoming events from the GUI
                while let Some(event) = INBOUND_EVENT_QUEUE.pop() {
                    handle_gui_message(
                        event,
                        &mut input,
                        &mut keybinds,
                        &mut camera_mode,
                        &mut freecam,
                        &mut playback,
                        &mut playback_settings,
                        &mut player,
                        &mut gamespeed,
                    );
                }

                // Listen for these keybinds regardless of freecam enabled state.
                // Since this might sample the current camera state, we should run this
                // after applying the matrix patches.
                keybinds.execute_general_bindings(&mut input);
            }, task_group);
        }

        // Fuck the code restoration routines as they remove MoveMapStep hooks
        unsafe { arxan::disable_code_restoration(&program) }.unwrap();

        // Hook the move map step so we can enable the debug frame-by-frame pause when users enter
        // into the first freecam mode. The reason for me putting this in a hook is because I can't
        // find a straightforward way to locate an instance of MoveMapStep.
        let move_map_step_hook_va = program.rva_to_va(offsets.move_map_step).unwrap();
        unsafe {
            MOVE_MAP_STEP
                .initialize(
                    transmute::<u64, unsafe extern "C" fn(OwnedPtr<MoveMapStep>, usize)>(move_map_step_hook_va),
                    |mut move_map_step, delta| {
                        // This flag indicates if the debug timestop is active, we should only
                        // enable it when the freecam mode has been set appropriately
                        move_map_step.debug_pause = DEBUG_PAUSE_ENABLED.load(Ordering::Relaxed);

                        MOVE_MAP_STEP.call(move_map_step, delta);
                    }
                ).unwrap()
                .enable().unwrap();
        }
        
        // Hook the Scaleform update so we can conditionally skip calling it, causing the rendered
        // scaleform output to never get copied into the final render.
        let scaleform_update_b_va = program.rva_to_va(offsets.scaleform_update_b).unwrap();
        unsafe {
            SCALEFORM_UPDATE_B
                .initialize(
                    std::mem::transmute::<u64, unsafe extern "C" fn(usize, usize)>(scaleform_update_b_va),
                    |param_1, param_2| {
                        if !DISABLE_HUD.load(Ordering::Relaxed) {
                            SCALEFORM_UPDATE_B.call(param_1, param_2);
                        }
                    }
                ).unwrap()
                .enable().unwrap();
        }

        Ok(())
    }
}

fn handle_gui_message(
    event: InboundGameControlEvent,
    input: &mut Input,
    keybinds: &mut Keybinds,
    camera_mode: &mut CameraMode,
    freecam: &mut FreeCam,
    playback: &mut PlaybackCam,
    playback_settings: &mut PlaybackSettingsData,
    player: &mut Player,
    gamespeed: &mut GameSpeed,
) {
    match event {
        InboundGameControlEvent::Initialize { settings } => {
            keybinds.set_mapping(settings.keybinds);
        }
        InboundGameControlEvent::Settings { settings } => {
            keybinds.set_mapping(settings.keybinds);
        }
        InboundGameControlEvent::SetTimeOfDay { hours, minutes } => {
            #[cfg(not(feature = "darksouls3"))]
            {
                let Ok(world_area_time) = (unsafe { WorldAreaTime::instance() }) else {
                    return;
                };

                world_area_time.request_time(hours as _, minutes as _, 0);
            }
            
        }
        InboundGameControlEvent::SetCharacterNoDead { enabled } => player.no_dead = enabled,
        InboundGameControlEvent::SetCharacterNoMove { enabled } => player.no_move = enabled,
        InboundGameControlEvent::SetCameraMode { mode } => {
            let Ok(cs_camera) = (unsafe { CSCamera::instance() }) else {
                return;
            };

            match mode {
                CameraMode::Game => {
                    input.block_input(InputBlockMode::None);
                }
                CameraMode::Freecam => {
                    input.block_input(InputBlockMode::KeyboardAndMouse);
                    let fov = cs_camera.pers_cam_1.fov;
                    *freecam = freecam_from_game_camera(&cs_camera.pers_cam_1);
                    OUTBOUND_EVENT_QUEUE.push(OutboundGameControlEvent::UpdateFreecamFov { fov });
                }
                CameraMode::Playback => {
                    if playback_settings.unpause_on_playback {
                        DEBUG_PAUSE_ENABLED.store(false, Ordering::Relaxed);
                    }

                    input.block_input(InputBlockMode::None);
                }
            }

            *camera_mode = mode;
        }
        InboundGameControlEvent::SetFreecamMovementSpeed { value } => {
            freecam.movement_speed_modifier = value;
        }
        InboundGameControlEvent::SetFreecamRotationSpeed { value } => {
            freecam.rotation_speed_modifier = value;
        }
        InboundGameControlEvent::SetFreecamSpeedModifiers { slow, fast } => {
            freecam.slow_speed_modifier = slow;
            freecam.fast_speed_modifier = fast;
        }
        InboundGameControlEvent::SetFreecamFov { fov } => {
            freecam.target_fov = fov;
        }
        InboundGameControlEvent::SetHudDisabled { disabled } => {
            DISABLE_HUD.store(disabled, Ordering::Relaxed)
        }
        InboundGameControlEvent::SetDebugPauseEnabled { enabled } => {
            DEBUG_PAUSE_ENABLED.store(enabled, Ordering::Relaxed)
        }
        InboundGameControlEvent::SetFreecamLocked { locked } => {
            // FREECAM_LOCKED.store(locked, Ordering::Relaxed);
            freecam.locked = locked;
        }
        InboundGameControlEvent::SetGameSpeedMultiplier { value } => {
            gamespeed.set_multiplier(value);
        }
        InboundGameControlEvent::SetGameSpeedMultiplierEnabled { enabled } => {
            gamespeed.set_multiplier_enabled(enabled);
        }
        InboundGameControlEvent::SetKeyframes { keyframes } => {
            playback.set_keyframes(keyframes);
        }
        InboundGameControlEvent::SetPlaybackState { playing, time } => {
            playback.playing = playing;
            playback.time = time;

            let cs_camera = unsafe { CSCamera::instance() };
            let Ok(cs_camera) = cs_camera else {
                return;
            };

            if *camera_mode != CameraMode::Playback {
                input.block_input(InputBlockMode::None);
                *camera_mode = CameraMode::Playback;

                if playback_settings.unpause_on_playback {
                    DEBUG_PAUSE_ENABLED.store(false, Ordering::Relaxed);
                }
            } else if *camera_mode == CameraMode::Playback && !playing {
                input.block_input(InputBlockMode::KeyboardAndMouse);
                *camera_mode = CameraMode::Freecam;
                *freecam = freecam_from_game_camera(&cs_camera.pers_cam_1);
            }
        }
        InboundGameControlEvent::SetPlaybackSettings { settings } => {
            gamespeed.set_playback_multiplier(settings.gamespeed_multiplier);
            *playback_settings = settings;
        }
    }
}

fn freecam_from_game_camera(camera: &CSPersCam) -> FreeCam {
    let matrix = camera.matrix;

    // Sample position of camera view matrix
    let rx = matrix.0;
    let ry = matrix.1;
    let rz = matrix.2;
    let rw = matrix.3;

    let rot = Mat3::from_cols(
        Vec3::new(rx.0, rx.1, rx.2),
        Vec3::new(ry.0, ry.1, ry.2),
        Vec3::new(rz.0, rz.1, rz.2),
    );

    // Break down current camera into euler angles so we can extract the roll and build a level
    // rotation.
    let (yaw, pitch, roll) = rot.to_euler(glam::EulerRot::YXZ);
    let level_orientation = glam::Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, 0.0);

    FreeCam::new(
        SPACE,
        Vec3::new(rw.0, rw.1, rw.2),
        level_orientation,
        roll,
        camera.fov,
    )
}

fn apply_camera_to_game_camera(camera: &Camera, game_camera: &mut CSPersCam) {
    // Patch up main cam with our own position and rotation.
    let rot = Mat3::from_quat(camera.rotation);
    let t = &camera.translation;

    game_camera.matrix.0 = F32Vector4(rot.col(0).x, rot.col(0).y, rot.col(0).z, 0.0);
    game_camera.matrix.1 = F32Vector4(rot.col(1).x, rot.col(1).y, rot.col(1).z, 0.0);
    game_camera.matrix.2 = F32Vector4(rot.col(2).x, rot.col(2).y, rot.col(2).z, 0.0);
    game_camera.matrix.3 = F32Vector4(t.x, t.y, t.z, 1.0);
    game_camera.fov = camera.fov;
}

static_detour! {
    static MOVE_MAP_STEP: unsafe extern "C" fn(OwnedPtr<MoveMapStep>, usize);
    static SCALEFORM_UPDATE_B: unsafe extern "C" fn(usize, usize);
}


static FREECAM_LOCKED: AtomicBool = AtomicBool::new(false);
static DISABLE_HUD: AtomicBool = AtomicBool::new(false);
static DEBUG_PAUSE_ENABLED: AtomicBool = AtomicBool::new(false);
static GAMESPEED_ENABLED: AtomicBool = AtomicBool::new(false);

static LOG_HANDLE: OnceLock<log4rs::Handle> = OnceLock::new();
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static INBOUND_EVENT_QUEUE: SegQueue<InboundGameControlEvent> = SegQueue::new();
static OUTBOUND_EVENT_QUEUE: SegQueue<OutboundGameControlEvent> = SegQueue::new();
