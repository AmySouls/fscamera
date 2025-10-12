use std::mem::transmute;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

use camera::freecam::{FreeCam, FreeCamInput};
use camera::playback::PlaybackCam;
use camera::Camera;
use camera::Space;
use crossbeam::queue::SegQueue;
use eldenring::cs::CSPersCam;
use eldenring::cs::CSTaskGroupIndex;
use eldenring::cs::CSTaskImp;
use eldenring::fd4::FD4TaskData;
use eldenring::position::PositionDelta;
use eldenring_util::task::CSTaskImpExt;
use fromsoft_shared::F32Matrix4x4;
use fromsoft_shared::F32Vector4;
use fromsoft_shared::{arxan, get_instance, OwnedPtr, Program};
use game::get_offsets;
use game::physics_coords_to_block_coords;
use game::CSCamera;
use game::CSFlipperImp;
use game::FieldArea;
use game::FreecamMode;
use game::MoveMapStep;
use game::WorldChrMan;
use log::LevelFilter;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};
use pelite::pe64::Pe;
use protocol::CameraMode;
use protocol::InboundGameControlEvent;
use protocol::OutboundGameControlEvent;
use protocol::{CameraState, RemoteError, SettingsData};

use retour::static_detour;
use windows::Win32::Foundation::HWND;

use crate::fov::FovOverride;
use crate::game::WorldAreaTime;
use crate::input::{Input, InputBlockMode};
use crate::player::Player;
use crate::keybind::Keybinds;

mod fov;
mod game;
mod input;
mod player;
mod keybind;

dll_syringe::payload_procedure! {
    fn snapshot_camera_state() -> Result<CameraState, RemoteError> {
        unsafe {
            let program = Program::current();
            let offsets = get_offsets(&program).map_err(|e| e.clone())?;

            let Some(cs_camera) = get_instance::<CSCamera>() else {
                return Err(RemoteError::AcquireCSCamera);
            };

            let field_area = transmute::<u64, OwnedPtr<Option<OwnedPtr<FieldArea>>>>(program.rva_to_va(offsets.field_area).unwrap());
            let Some(field_area) = field_area.as_ref() else {
                return Err(RemoteError::AcquireFieldArea)
            };

            let field_area = field_area.as_ref();
            let map_id = field_area.map_id;

            let position = cs_camera.pers_cam_1.position();
            // let Some(position) = physics_coords_to_block_coords(field_area, &map_id, &position) else {
            //     return Err(RemoteError::AcquireWorldBlockInfo);
            // };

            let matrix = &cs_camera.pers_cam_1.matrix;
            let rx = matrix.0;
            let ry = matrix.1;
            let rz = matrix.2;
            let rot = glam::Mat3::from_cols(
                glam::Vec3::new(rx.0, rx.1, rx.2),
                glam::Vec3::new(ry.0, ry.1, ry.2),
                glam::Vec3::new(rz.0, rz.1, rz.2),
            );
            let [qx, qy, qz, qw] = glam::Quat::from_mat3(&rot).to_array();

            Ok(CameraState {
                map_id: map_id.0,
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
        let program = Program::current();
        let offsets = get_offsets(&program).map_err(|e| e.clone())?;

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
        let mut keybinds = Keybinds::default();
        let cs_task = unsafe { get_instance::<CSTaskImp>() }.unwrap();

        // Keep track of delta time between task execution as we'll be messing with the one offered
        // by the game.
        let mut last_update = Instant::now();

        let space = Space {
            right: glam::Vec3::X,
            up: glam::Vec3::Y,
            forward: glam::Vec3::Z,
        };

        let mut camera_mode = CameraMode::Game;
        let mut freecam = FreeCam::from(
            space.clone(),
            glam::Vec3::ZERO,
            glam::Quat::IDENTITY,
        );
        let mut playback = PlaybackCam::new(
            space,
            glam::Vec3::ZERO,
            glam::Quat::IDENTITY,
        );
        let mut fov_override = FovOverride::default();
        let mut player = Player::default();

        unsafe { input::setup_hook() };

        {
            // Hijack camera matrix by enqueueing a task to happen right before the draw happens.
            // It's very important that we do this before the draw and after the OG camera update.
            cs_task.run_recurring(move |_: &FD4TaskData| {
                // Keep track of our own delta time as the game speed override influences the one
                // coming in with the task data.
                let now = Instant::now();
                let delta = now - last_update;
                last_update = now;

                // Handle incoming events from the GUI
                while let Some(event) = INBOUND_EVENT_QUEUE.pop() {
                    handle_gui_message(
                        &event,
                        &mut input,
                        &mut keybinds,
                        &mut camera_mode,
                        &mut freecam,
                        &mut playback,
                        &mut fov_override,
                        &mut player,
                    );
                }

                // Acquire a pile of shit from the game we can't really live without.

                // We need FieldArea to translate coordinates from havok space (where the cam lives) to
                // block space as havok space shifts around a lot making it unusable for persisting
                // coordinates with.
                let field_area = unsafe {
                    transmute::<u64, OwnedPtr<Option<OwnedPtr<FieldArea>>>>(program.rva_to_va(offsets.field_area).unwrap())
                };

                // Camera's isn't necessarily there and we've got nothing to do in such a situation.
                let cs_camera = unsafe { get_instance::<CSCamera>() };
                let Some(cs_camera) = cs_camera else {
                    return;
                };

                // WorldChrMan is responsible for managing characters including our main player.
                // Our main player is required for a few of the other patches as well as figuring
                // out what map to use as a base for our coordinate conversions.
                let world_chr_man = unsafe { get_instance::<WorldChrMan>() };
                let Some(world_chr_man) = world_chr_man else {
                    return;
                };

                // Update input state for reading.
                input.update();

                // Apply no dead and no move
                player.apply(world_chr_man);

                keybinds.execute(&mut input);

                // Update game camera
                match camera_mode {
                    CameraMode::Game => {},
                    CameraMode::Freecam => {
                        let mut freecam_input = FreeCamInput::default();

                        let orientation_delta = input.orientation_delta();
                        freecam_input.mouse_delta = glam::Vec2::new(
                            orientation_delta.0,
                            orientation_delta.1,
                        );

                        // Movement
                        input.key_pressed(0x57).then(|| freecam_input.forward += 1.0);
                        input.key_pressed(0x53).then(|| freecam_input.forward -= 1.0);
                        input.key_pressed(0x44).then(|| freecam_input.right += 1.0);
                        input.key_pressed(0x41).then(|| freecam_input.right -= 1.0);
                        input.key_pressed(0x45).then(|| freecam_input.up += 1.0);
                        input.key_pressed(0x51).then(|| freecam_input.up -= 1.0);

                        // Tilt
                        input.key_pressed(0x4D).then(|| freecam_input.roll_delta += 1.0);
                        input.key_pressed(0x4E).then(|| freecam_input.roll_delta -= 1.0);

                        // Speed modifiers
                        input.key_pressed(0xA4).then(|| freecam_input.speed_multiplier = 4.0);
                        input.key_pressed(0xA2).then(|| freecam_input.speed_multiplier = 1.0 / 4.0);

                        freecam.update(&freecam_input, delta.as_secs_f32());

                        apply_camera_to_game_camera(
                            &freecam.camera,
                            &mut cs_camera.pers_cam_1,
                        );

                        fov_override.apply(cs_camera);
                    },
                    CameraMode::Playback => {
                        playback.update(delta.as_secs_f32());

                        apply_camera_to_game_camera(
                            &playback.camera,
                            &mut cs_camera.pers_cam_1,
                        );
                    },
                }

                // Switch to and from freecam mode. This has to happen after the playback mode
                // patched the matrix already.
                if input.key_pressed_debounced(0x78, Duration::from_millis(500)) {
                    camera_mode = match camera_mode {
                        CameraMode::Game => CameraMode::Freecam,
                        CameraMode::Playback => CameraMode::Freecam,
                        CameraMode::Freecam => CameraMode::Game,
                    };

                    // Initialize freecam with current position and rotation.
                    if camera_mode == CameraMode::Freecam {
                        input.block_input(InputBlockMode::KeyboardAndMouse);
                        freecam = freecam_from_viewmatrix(&cs_camera.pers_cam_1.matrix);
                    } else {
                        input.block_input(InputBlockMode::None);
                    }
                }
            }, CSTaskGroupIndex::Draw_Pre);
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
                    std::mem::transmute::<u64, unsafe extern "C" fn(OwnedPtr<MoveMapStep>, usize)>(move_map_step_hook_va),
                    |mut move_map_step, delta| {
                        // This flag indicates if the debug timestop is active, we should only
                        // enable it when the freecam mode has been set appropriately
                        move_map_step.debug_pause = move_map_step.field_area.game_rend.freecam_mode == FreecamMode::ActivePaused;

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
                .enable()
                .unwrap();
        }

        Ok(())
    }
}

fn handle_gui_message(
    event: &InboundGameControlEvent,
    input: &mut Input,
    keybinds: &mut Keybinds,
    camera_mode: &mut CameraMode,
    freecam: &mut FreeCam,
    playback: &mut PlaybackCam,
    fov_override: &mut FovOverride,
    player: &mut Player,
) {
    match event {
        InboundGameControlEvent::SetTimeOfDay {
            hours,
            minutes,
            seconds,
        } => {
            let world_area_time = unsafe { get_instance::<WorldAreaTime>() };
            let Some(world_area_time) = world_area_time else {
                return;
            };

            world_area_time.request_hour = *hours as _;
            world_area_time.request_minute = *minutes as _;
            world_area_time.request_second = *seconds as _;
        }
        InboundGameControlEvent::SetFreecamEnabled { enabled } => {
            let cs_camera = unsafe { get_instance::<CSCamera>() };
            let Some(cs_camera) = cs_camera else {
                return;
            };

            *camera_mode = if *enabled {
                CameraMode::Freecam
            } else {
                CameraMode::Game
            };

            if *camera_mode == CameraMode::Freecam {
                input.block_input(InputBlockMode::KeyboardAndMouse);
                *freecam = freecam_from_viewmatrix(&cs_camera.pers_cam_1.matrix);
            } else {
                input.block_input(InputBlockMode::None);
            }
        }
        InboundGameControlEvent::SetHudDisabled { disabled } => {
            DISABLE_HUD.store(*disabled, Ordering::Relaxed)
        }
        InboundGameControlEvent::SetFreecamMovementSpeed { value } => {
            freecam.movement_speed_multiplier = *value
        }
        InboundGameControlEvent::SetFreecamRotationSpeed { value } => {
            freecam.rotation_speed_multiplier = *value
        }
        InboundGameControlEvent::TimeMultiplier { multiplier } => {
            let cs_flipper = unsafe { get_instance::<CSFlipperImp>() };
            let Some(cs_flipper) = cs_flipper else {
                return;
            };

            cs_flipper.time_multiplier = *multiplier;
        }
        InboundGameControlEvent::GlobalFov { fov, enabled } => {
            fov_override.active = *enabled;
            fov_override.applied_fov = *fov;
        }
        InboundGameControlEvent::SetCharacterNoDead { value } => player.no_dead = *value,
        InboundGameControlEvent::SetCharacterNoMove { value } => player.no_move = *value,
        InboundGameControlEvent::Keyframes { keyframes } => {
            playback.set_keyframes(fixup_keyframe_positions(keyframes).clone());
        }
        InboundGameControlEvent::Play { time, keyframes } => {
            playback.playing = true;
            playback.time = *time;
            input.block_input(InputBlockMode::None);
            *camera_mode = CameraMode::Playback;
        }
        InboundGameControlEvent::Pause { time } => {
            playback.playing = false;
            playback.time = *time;
        }
        InboundGameControlEvent::Scrub { time } => playback.time = *time,
        InboundGameControlEvent::Initialize { settings } => {
            keybinds.keybinds = settings.keybinds.to_vec();
        },
        InboundGameControlEvent::Settings { settings } => {
            keybinds.keybinds = settings.keybinds.to_vec();
        },
        _ => {
            log::info!("Got game unknown control event: {event:?}")
        }
    }
}

fn freecam_from_viewmatrix(matrix: &F32Matrix4x4) -> FreeCam {
    // Sample position of camera view matrix
    let rx = matrix.0;
    let ry = matrix.1;
    let rz = matrix.2;
    let rw = matrix.3;

    let rot = glam::Mat3::from_cols(
        glam::Vec3::new(rx.0, rx.1, rx.2),
        glam::Vec3::new(ry.0, ry.1, ry.2),
        glam::Vec3::new(rz.0, rz.1, rz.2),
    );

    FreeCam::from(
        Space {
            right: glam::Vec3::X,
            up: glam::Vec3::Y,
            forward: glam::Vec3::Z,
        },
        glam::Vec3::new(rw.0, rw.1, rw.2),
        glam::Quat::from_mat3(&rot),
    )
}

fn apply_camera_to_game_camera(camera: &Camera, game_camera: &mut CSPersCam) {
    let rot = glam::Mat3::from_quat(camera.rotation);

    game_camera.matrix.0 = F32Vector4(rot.col(0).x, rot.col(0).y, rot.col(0).z, 0.0);
    game_camera.matrix.1 = F32Vector4(rot.col(1).x, rot.col(1).y, rot.col(1).z, 0.0);
    game_camera.matrix.2 = F32Vector4(rot.col(2).x, rot.col(2).y, rot.col(2).z, 0.0);

    // Patch up main cam matrix with our overriden position and rotation.
    game_camera.matrix.3 = F32Vector4(
        camera.translation.x,
        camera.translation.y,
        camera.translation.z,
        1.0,
    );

    game_camera.fov = camera.fov;
}

fn fixup_keyframe_positions(
    frames: &[protocol::keyframe::Keyframe],
) -> Vec<protocol::keyframe::Keyframe> {
    frames
        .iter()
        .map(|f| {
            /// Keyframe positions are MSB space, so we'll need to
            let position = f.position;

            protocol::keyframe::Keyframe {
                time: f.time,
                map_id: f.map_id,
                position,
                orientation: f.orientation,
                fov: f.fov,
            }
        })
        .collect()
}

static_detour! {
    static MOVE_MAP_STEP: unsafe extern "C" fn(OwnedPtr<MoveMapStep>, usize);
    static SCALEFORM_UPDATE_B: unsafe extern "C" fn(usize, usize);
}

static DISABLE_HUD: AtomicBool = AtomicBool::new(false);
static DEBUG_PAUSE_ENABLED: AtomicBool = AtomicBool::new(false);
static LOG_HANDLE: OnceLock<log4rs::Handle> = OnceLock::new();
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static INBOUND_EVENT_QUEUE: SegQueue<InboundGameControlEvent> = SegQueue::new();
static OUTBOUND_EVENT_QUEUE: SegQueue<OutboundGameControlEvent> = SegQueue::new();
