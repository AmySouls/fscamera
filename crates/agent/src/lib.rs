use std::mem::transmute;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use std::time::Instant;

use camera::CameraManager;
use crossbeam::queue::SegQueue;
use eldenring::cs::CSTaskGroupIndex;
use eldenring::cs::CSTaskImp;
use eldenring::fd4::FD4TaskData;
use eldenring_util::task::CSTaskImpExt;
use fromsoft_shared::{arxan, OwnedPtr, Program, get_instance};
use game::get_offsets;
use game::CSCamera;
use game::CSFlipperImp;
use game::FieldArea;
use game::FreecamMode;
use game::MoveMapStep;
use log::LevelFilter;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};
use nalgebra::RowVector4;
use nalgebra_glm::Mat4;
use nalgebra_glm::TMat4;
use pelite::pe64::Pe;
use protocol::keyframe::Quat;
use protocol::InboundGameControlEvent;
use protocol::OutboundGameControlEvent;
use protocol::SettingsData;
use protocol::{keyframe::Vec3, CameraState, RemoteError};

use nalgebra_glm as glm;
use retour::static_detour;

mod camera;
mod game;

dll_syringe::payload_procedure! {
    fn snapshot_camera_state() -> Result<CameraState, RemoteError> {
        let program = Program::current();
        let offsets = get_offsets(&program)?;

        let Some(cs_camera) = (unsafe { get_instance::<game::CSCamera>() }) else {
            return Err(RemoteError::AcquireCSCamera);
        };

        let field_area = unsafe { std::mem::transmute::<u64, OwnedPtr<Option<OwnedPtr<FieldArea>>>>(program.rva_to_va(offsets.field_area).unwrap()) };
        let Some(field_area) = field_area.as_ref() else {
            return Err(RemoteError::AcquireFieldArea)
        };

        let field_area = field_area.as_ref();
        let map_id = field_area.map_id;
        let Some(world_block_info) = field_area.world_info_owner.world_block_info_by_map(&map_id) else {
            return Err(RemoteError::AcquireWorldBlockInfo)
        };

        let block_pos = cs_camera.pers_cam_1.position() - world_block_info.physics_center;
        let position = Vec3::new(
            block_pos.0,
            block_pos.1,
            block_pos.2,
        );

        let orientation = {
            // let matrix: TMat4<f32> = cs_camera.pers_cam_1.matrix.clone().into();
            let mtx = &cs_camera.pers_cam_1.matrix;
            let matrix = Mat4::from_rows(&[
                RowVector4::new(mtx.0 .0, mtx.0 .1, mtx.0 .2, mtx.0 .3),
                RowVector4::new(mtx.1 .0, mtx.1 .1, mtx.1 .2, mtx.1 .3),
                RowVector4::new(mtx.2 .0, mtx.2 .1, mtx.2 .2, mtx.2 .3),
                RowVector4::new(mtx.3 .0, mtx.3 .1, mtx.3 .2, mtx.3 .3),
            ]);

            let rotation = matrix.fixed_view::<3, 3>(0, 0).into_owned();
            glm::mat3_to_quat(&rotation)
        };

        let orientation_vec = orientation.as_vector();
        Ok(CameraState {
            map_id: world_block_info.map_id.into(),
            position,
            orientation: Quat(
                orientation_vec.x,
                orientation_vec.y,
                orientation_vec.z,
                orientation_vec.w,
            ),
            fov: cs_camera.pers_cam_1.fov,
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
        let program = Program::current();
        let offsets = get_offsets(&program)?;

        if INITIALIZED.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).is_err() {
            log::info!("Already initialized camera agent");
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

        log::info!("Initializing camera agent");

        let mut camera_manager = CameraManager::default();
        let cs_task = (unsafe { get_instance::<CSTaskImp>() }).unwrap();

        // Keep track of delta time between task execution as we'll be messing with the one offered
        // by the game.
        let mut last_update = Instant::now();

        {
            let offsets = get_offsets(&program)?;
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
                    camera_manager.handle_event(&event);
                }

                // Camera's isn't necessarily there and we've got nothing to do in such a situation.
                let Some(cs_camera) = (unsafe { get_instance::<CSCamera>() }) else {
                    return;
                };

                camera_manager.update(&delta);

                // We need FieldArea to translate coordinates from havok space (where the cam lives) to
                // block space as havok space shifts around a lot making it unusable for persisting
                // coordinates with.
                let field_area = unsafe {
                    transmute::<u64, OwnedPtr<Option<OwnedPtr<FieldArea>>>>(program.rva_to_va(offsets.field_area).unwrap())
                };

                // CSFlipper is responsible for flipping the framebuffer so it should be available if
                // we're rendering stuff...
                let Some(cs_flipper) = (unsafe { get_instance::<CSFlipperImp>() }) else {
                    return;
                };

                // WorldChrMan is responsible for managing characters including our main player
                let Some(world_chr_man) = (unsafe { get_instance::<game::WorldChrMan>() }) else {
                    return;
                };

                // Field area is responsible for translating havok AABB coords to block coords, we
                // can't displace the camera sensibly without it.
                let Some(field_area) = field_area.as_ref() else {
                    return;
                };

                // Update the camera's matrix if necessary
                camera_manager.apply(&offsets, cs_camera, field_area.as_ref(), cs_flipper, world_chr_man);

            }, CSTaskGroupIndex::Draw_Pre);
        }

        // Patch in freecam controls
        let return_true: [u8; 3] = [0xB0, 0x01, 0xC3];
        let enable_freecam_controls_va = program.rva_to_va(offsets.enable_freecam_controls).unwrap();
        unsafe { std::ptr::copy_nonoverlapping(&return_true, enable_freecam_controls_va as _, 3); }

        // Patch in L3+X enabling byte
        let enable_freecam_toggle: *mut bool = program.rva_to_va(offsets.enable_freecam_toggle).unwrap() as _;
        unsafe { *enable_freecam_toggle = true };

        // Fuck the code restoration routines as they remove MoveMapStep hooks
        unsafe { arxan::disable_code_restoration(&program) }.unwrap();

        // Hook the move map step so we can enable the debug frame-by-frame pause when users enter
        // into the first freecam mode.
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

        // Hook the move map step so we can enable the debug frame-by-frame pause when users enter
        // into the first freecam mode.
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

static_detour! {
    static MOVE_MAP_STEP: unsafe extern "C" fn(OwnedPtr<MoveMapStep>, usize);
    static SCALEFORM_UPDATE_B: unsafe extern "C" fn(usize, usize);
}

static DISABLE_HUD: AtomicBool= AtomicBool::new(false);
static LOG_HANDLE: OnceLock<log4rs::Handle> = OnceLock::new();
static INITIALIZED: AtomicBool = AtomicBool::new(false);
static INBOUND_EVENT_QUEUE: SegQueue<InboundGameControlEvent> = SegQueue::new();
static OUTBOUND_EVENT_QUEUE: SegQueue<OutboundGameControlEvent> = SegQueue::new();
