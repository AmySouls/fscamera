use std::mem::transmute;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

use ::camera::FreeCam;
use ::camera::FreeCamInput;
use camera::CameraManager;
use crossbeam::queue::SegQueue;
use eldenring::cs::CSTaskGroupIndex;
use eldenring::cs::CSTaskImp;
use eldenring::cs::CSWindowImp;
use eldenring::fd4::FD4TaskData;
use eldenring::position::PositionDelta;
use eldenring_util::task::CSTaskImpExt;
use fromsoft_shared::F32Vector4;
use fromsoft_shared::{arxan, get_instance, OwnedPtr, Program};
use game::get_offsets;
use game::CSCamera;
use game::CSFlipperImp;
use game::FieldArea;
use game::FreecamMode;
use game::MoveMapStep;
use game::WorldChrMan;
use glam::Mat3;
use glam::Quat;
use glam::Vec2;
use glam::Vec3;
use log::LevelFilter;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};
use nalgebra::Quaternion;
use nalgebra::RowVector3;
use nalgebra::RowVector4;
use nalgebra_glm::Mat4;
use nalgebra_glm::TMat4;
use nalgebra_glm::Vec4;
use pelite::pe64::Pe;
use protocol::keyframe::Orientation;
use protocol::CameraMode;
use protocol::InboundGameControlEvent;
use protocol::OutboundGameControlEvent;
use protocol::SettingsData;
use protocol::{CameraState, RemoteError};

use nalgebra_glm as glm;
use retour::static_detour;
use windows::Win32::Foundation::HWND;

mod camera;
mod freecam;
mod game;
mod input;
mod keyframe;

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
            let Some(world_block_info) = field_area.world_info_owner.world_block_info_by_map(&map_id) else {
                return Err(RemoteError::AcquireWorldBlockInfo)
            };

            let block_pos = cs_camera.pers_cam_1.position() - world_block_info.physics_center;
            let position = protocol::keyframe::Vec3::new(
                block_pos.0,
                block_pos.1,
                block_pos.2,
            );

            // let orientation = {
            //     let PositionDelta(rx, ry, rz) = cs_camera.pers_cam_1.right();
            //     let PositionDelta(ux, uy, uz) = cs_camera.pers_cam_1.up();
            //     let PositionDelta(fx, fy, fz) = cs_camera.pers_cam_1.forward();
            //
            //     let rotation = Mat3::from_rows(&[
            //         RowVector3::new(rx, ry, rz),
            //         RowVector3::new(ux, uy, uz),
            //         RowVector3::new(fx, fy, fz),
            //     ]);
            //
            //     glm::mat3_to_quat(&rotation)
            // };
            let orientation = Quaternion::identity();

            let euler = glm::quat_euler_angles(&orientation);
            Ok(CameraState {
                map_id: world_block_info.map_id.into(),
                position,
                orientation: Orientation(
                    euler.x,
                    euler.y,
                    euler.z,
                ),
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

        // let mut camera_manager = CameraManager::default();
        let mut input = input::Input::default();
        let cs_task = unsafe { get_instance::<CSTaskImp>() }.unwrap();

        // Keep track of delta time between task execution as we'll be messing with the one offered
        // by the game.
        let mut last_update = Instant::now();

        let mut camera_mode = CameraMode::Game;
        let mut freecam = FreeCam::default();

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
                    handle_inbound_event(
                        &event,
                        &mut camera_mode,
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

                // CSFlipper is responsible for flipping the framebuffer so it should be available if
                // we're rendering stuff...
                let cs_flipper = unsafe { get_instance::<CSFlipperImp>() };
                let Some(cs_flipper) = cs_flipper else {
                    return;
                };

                // WorldChrMan is responsible for managing characters including our main player.
                // Our main player is required for a few of the other patches as well as figuring
                // out what map to use as a base for our coordinate conversions.
                let world_chr_man = unsafe { get_instance::<WorldChrMan>() };
                let Some(world_chr_man) = world_chr_man else {
                    return;
                };

                input.update();

                // Test code to switch camera modes
                if input.key_pressed_debounced(0x78, Duration::from_secs(500)) {
                    camera_mode = match camera_mode {
                        CameraMode::Game => CameraMode::Freecam,
                        CameraMode::Freecam => CameraMode::Game,
                    };

                    // Initialize freecam with current position and rotation.
                    if camera_mode == CameraMode::Freecam {
                        // Sample position of camera
                        let F32Vector4(tx, ty, tz, _) = cs_camera.pers_cam_1.matrix.3;

                        freecam = FreeCam::from(
                            Vec3::new(tx, ty, tz),
                            Quat::IDENTITY,
                        );
                    }
                }

                match camera_mode {
                    CameraMode::Game => {},
                    CameraMode::Freecam => {
                        let mut freecam_input = FreeCamInput::default();

                        let orientation_delta = input.orientation_delta();
                        freecam_input.mouse_delta = Vec2::new(
                            orientation_delta.0,
                            orientation_delta.1,
                        );

                        input.key_pressed(0x57).then(|| freecam_input.forward += 1.0);
                        input.key_pressed(0x53).then(|| freecam_input.forward -= 1.0);
                        input.key_pressed(0x44).then(|| freecam_input.right += 1.0);
                        input.key_pressed(0x41).then(|| freecam_input.right -= 1.0);

                        freecam.update(&freecam_input, delta.as_secs_f32());

                        let rot = Mat3::from_quat(freecam.rotation);
                        cs_camera.pers_cam_1.matrix.0 = F32Vector4(
                            rot.col(0).x,
                            rot.col(0).y,
                            rot.col(0).z,
                            0.0,
                        );

                        cs_camera.pers_cam_1.matrix.1 = F32Vector4(
                            rot.col(1).x,
                            rot.col(1).y,
                            rot.col(1).z,
                            0.0,
                        );

                        cs_camera.pers_cam_1.matrix.2 = F32Vector4(
                            rot.col(2).x,
                            rot.col(2).y,
                            rot.col(2).z,
                            0.0,
                        );

                        // Patch up main cam matrix with our overriden position and rotation. 
                        cs_camera.pers_cam_1.matrix.3 = F32Vector4(
                            freecam.translation.x,
                            freecam.translation.y,
                            freecam.translation.z,
                            1.0,
                        );
                    },
                }

                // camera_manager.update(&input, &delta);
                // // Update the camera's matrix if necessary
                // camera_manager.apply(
                //     &offsets,
                //     cs_camera,
                //     field_area.as_ref(),
                //     cs_flipper,
                //     world_chr_man,
                // );
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

fn handle_inbound_event(event: &InboundGameControlEvent, camera_mode: &mut CameraMode) {
    match event {
        _ => {
            log::info!("Got game control event: {event:?}")
        }
        // InboundGameControlEvent::Initialize { settings } => todo!(),
        // InboundGameControlEvent::Settings { settings } => todo!(),
        // InboundGameControlEvent::Keyframes { keyframes } => todo!(),
        // InboundGameControlEvent::Play { time, keyframes } => todo!(),
        // InboundGameControlEvent::Pause { time } => todo!(),
        // InboundGameControlEvent::Scrub { time } => todo!(),
        // InboundGameControlEvent::PlaybackModeState { state } => todo!(),
        // InboundGameControlEvent::TimeMultiplier { multiplier } => todo!(),
        // InboundGameControlEvent::GlobalFov { fov, enabled } => todo!(),
        // InboundGameControlEvent::RequestTimeOfDay { hours, minutes, seconds } => todo!(),
        // InboundGameControlEvent::HudState { hidden } => todo!(),
        // InboundGameControlEvent::SetCharacterNoDead { value } => todo!(),
        // InboundGameControlEvent::SetCharacterNoMove { value } => todo!(),
        // InboundGameControlEvent::SetFreecamMovementSpeed { value } => todo!(),
        // InboundGameControlEvent::SetFreecamRotationSpeed { value } => todo!(),
        // InboundGameControlEvent::SetDebugPause { enabled } => todo!(),
        // InboundGameControlEvent::SetFreecamEnabled { enabled } => todo!(),
    }
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
