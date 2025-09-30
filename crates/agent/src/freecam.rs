use eldenring::{
    cs::{BlockId, CSWindowImp},
    position::{HavokPosition, PositionDelta},
};
use nalgebra::{Quaternion, RowVector3, RowVector4};
use pelite::pe64::Pe;
use std::{mem::transmute, time::Duration};
use thiserror::Error;
use windows::Win32::{
    Foundation::{HWND, POINT},
    System::Threading::GetCurrentProcessId,
    UI::{
        Input::{KeyboardAndMouse::GetKeyState, XboxController::{XInputGetState, XINPUT_STATE, XUSER_MAX_COUNT}},
        WindowsAndMessaging::{GetCursorPos, GetForegroundWindow, GetWindowThreadProcessId},
    },
};

use eldenring_util::input;
use fromsoft_shared::{F32Vector4, OwnedPtr, Program, get_instance};
use nalgebra_glm::{self as glm, Mat3, Mat4, Vec3, Vec4};

use crate::{game::{get_offsets, CSCamera, FieldArea, MapId}, input::Input};

#[derive(Debug, Error)]
pub enum FreecamError {
    #[error("Game or version of game is not supported.")]
    UnknownGame,
    #[error("Could not obtain an instance of CSCamera")]
    AcquireCSCamera,
    #[error("Could not obtain an instance of FieldArea")]
    AcquireFieldArea,
    #[error("Could not obtain an instance of WorldBlockInfo")]
    AcquireWorldBlockInfo,
}

// pub const RIGHT: Vec3 = Vec3::new(1.0, 0.0, 0.0);
// pub const UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);
// pub const FORWARD: Vec3 = Vec3::new(0.0, 0.0, 1.0);
//
// pub const ROTATION_FUDGE: f32 = 0.4;
// pub const MOVEMENT_FUDGE: f32 = 0.2;
//
// pub(crate) struct Freecam {
//     active: bool,
//     origin_block_id: MapId,
//     orientation: Quaternion<f32>,
//     position: Vec3,
//
//     movement_speed_multiplier: f32,
//     rotation_speed_multiplier: f32,
//     tilt_speed: f32,
//
//     yaw: f32,
//     pitch: f32,
//     roll: f32,
//
//     prev_tick_game_focussed: bool,
// }
//
// impl Default for Freecam {
//     fn default() -> Self {
//         Self {
//             active: false,
//             origin_block_id: MapId::none(),
//             orientation: Quaternion::identity(),
//             position: Vec3::new(0.0, 0.0, 0.0),
//
//             movement_speed_multiplier: 0.5f32,
//             rotation_speed_multiplier: 0.5f32,
//             tilt_speed: 2.0f32,
//
//             yaw: 0.0f32,
//             pitch: 0.0f32,
//             roll: 0.0f32,
//
//             prev_tick_game_focussed: false,
//         }
//     }
// }
//
// impl Freecam {
//     pub fn update(&mut self, input: &Input, delta: &Duration) {
//         // Get the delta as a fraction of a second.
//         let delta = delta.as_millis() as f32 / 1000.0;
//         let cursor_change = input.orientation_delta();
//
//         if input::is_key_pressed(0x78) {
//             self.active = !self.active;
//
//             if self.active
//                 && let Err(e) = self.initialize()
//             {
//                 log::error!("Could not initialize freecam: {e:?}");
//             }
//         }
//
//         if self.active {
//             // Filter noise.
//             let exceeds_noise_threshold = (cursor_change.0.abs() + cursor_change.1.abs()) > 2.0;
//
//             // Prevent first frame from causing awkward jumps in the camera orientation.
//             if self.prev_tick_game_focussed && exceeds_noise_threshold {
//                 // Read mouse movement and update yaw/pitch
//                 self.apply_mouse_delta(
//                     (-(cursor_change.0 * delta)) * ROTATION_FUDGE,
//                     (cursor_change.1 * delta) * ROTATION_FUDGE,
//                 );
//             }
//             self.prev_tick_game_focussed = true;
//
//             let mut tilt_change = 0.0;
//             if input.key_pressed(0x4E) {
//                 self.apply_tilt(self.tilt_speed * delta);
//             }
//
//             if input.key_pressed(0x4D) {
//                 self.apply_tilt(-self.tilt_speed * delta);
//             }
//
//             // With the mouse X and Y operating on the yaw and pitch exclusively
//             // it unfortunately makes a tad more sense operating on those instead of
//             // doing some 300IQ quaternion math.
//             let q_pitch = glm::quat_angle_axis(self.pitch, &RIGHT);
//             let q_yaw = glm::quat_angle_axis(self.yaw, &UP);
//             let q_roll = glm::quat_angle_axis(self.roll, &FORWARD);
//             let orientation = (q_roll * q_pitch * q_yaw).normalize();
//
//             let mut movement_mult = self.movement_speed_multiplier * MOVEMENT_FUDGE;
//
//             // Speed modifiers
//             if input.key_pressed(0xA4) {
//                 movement_mult *= 4.0;
//             }
//             if input.key_pressed(0xA2) {
//                 movement_mult /= 4.0;
//             }
//
//             let rotation = glm::quat_to_mat3(&orientation);
//             if input.key_pressed(0x57) {
//                 self.position +=
//                     glm::make_vec3(&[rotation.m31, rotation.m32, rotation.m33]) * movement_mult;
//             }
//
//             if input.key_pressed(0x53) {
//                 self.position -=
//                     glm::make_vec3(&[rotation.m31, rotation.m32, rotation.m33]) * movement_mult;
//             }
//
//             if input.key_pressed(0x44) {
//                 self.position +=
//                     glm::make_vec3(&[rotation.m11, rotation.m12, rotation.m13]) * movement_mult;
//             }
//
//             if input.key_pressed(0x41) {
//                 self.position -=
//                     glm::make_vec3(&[rotation.m11, rotation.m12, rotation.m13]) * movement_mult;
//             }
//
//             if input.key_pressed(0x45) {
//                 self.position +=
//                     glm::make_vec3(&[rotation.m21, rotation.m22, rotation.m23]) * movement_mult;
//             }
//
//             if input.key_pressed(0x51) {
//                 self.position -=
//                     glm::make_vec3(&[rotation.m21, rotation.m22, rotation.m23]) * movement_mult;
//             }
//         }
//
//         if !is_game_focused() {
//             self.prev_tick_game_focussed = false;
//         }
//     }
//
//     pub fn apply(&self) {
//         let program = Program::current();
//         let offsets = get_offsets(&program).unwrap();
//
//         let field_area = unsafe {
//             transmute::<u64, OwnedPtr<Option<OwnedPtr<FieldArea>>>>(
//                 program.rva_to_va(offsets.field_area).unwrap(),
//             )
//         };
//
//         if self.active
//             && let Some(camera) = unsafe { get_instance::<CSCamera>() }
//             && let Some(field_area) = field_area.as_ref()
//             && let Some(block_info) = field_area
//                 .world_info_owner
//                 .world_block_info_by_map(&self.origin_block_id)
//         {
//             let q_yaw = glm::quat_angle_axis(self.yaw, &UP);
//             let q_pitch = glm::quat_angle_axis(self.pitch, &RIGHT);
//             let q_roll = glm::quat_angle_axis(self.roll, &FORWARD);
//             let orientation = (q_roll * q_pitch * q_yaw).normalize();
//
//             // Build up new matrix to swap the primary view matrix with.
//             let rotation = glm::quat_to_mat4(&orientation);
//             camera.pers_cam_1.matrix.0 =
//                 F32Vector4(rotation.m11, rotation.m12, rotation.m13, rotation.m14);
//             camera.pers_cam_1.matrix.1 =
//                 F32Vector4(rotation.m21, rotation.m22, rotation.m23, rotation.m24);
//             camera.pers_cam_1.matrix.2 =
//                 F32Vector4(rotation.m31, rotation.m32, rotation.m33, rotation.m34);
//
//             let position = block_info.physics_center
//                 + PositionDelta(self.position.x, self.position.y, self.position.z);
//             camera.pers_cam_1.matrix.3 = F32Vector4(position.0, position.1, position.2, 1.0);
//         }
//     }
//
//     fn initialize(&mut self) -> Result<(), FreecamError> {
//         let program = Program::current();
//         let offsets = get_offsets(&program).unwrap();
//
//         let cs_camera = unsafe { get_instance::<CSCamera>() };
//         let Some(cs_camera) = cs_camera else {
//             return Err(FreecamError::AcquireCSCamera);
//         };
//
//         let field_area = unsafe {
//             transmute::<u64, OwnedPtr<Option<OwnedPtr<FieldArea>>>>(
//                 program.rva_to_va(offsets.field_area).unwrap(),
//             )
//         };
//         let Some(field_area) = field_area.as_ref() else {
//             return Err(FreecamError::AcquireFieldArea);
//         };
//
//         let field_area = field_area.as_ref();
//         self.origin_block_id = field_area.map_id;
//         let Some(world_block_info) = field_area
//             .world_info_owner
//             .world_block_info_by_map(&self.origin_block_id)
//         else {
//             return Err(FreecamError::AcquireWorldBlockInfo);
//         };
//
//         let block_pos = cs_camera.pers_cam_1.position() - world_block_info.physics_center;
//         self.position = Vec3::new(block_pos.0, block_pos.1, block_pos.2);
//
//         let orientation = {
//             let PositionDelta(rx, ry, rz) = cs_camera.pers_cam_1.right();
//             let PositionDelta(ux, uy, uz) = cs_camera.pers_cam_1.up();
//             let PositionDelta(fx, fy, fz) = cs_camera.pers_cam_1.forward();
//
//             let rotation = Mat3::from_rows(&[
//                 RowVector3::new(rx, ry, rz),
//                 RowVector3::new(ux, uy, uz),
//                 RowVector3::new(fx, fy, fz),
//             ]);
//
//             glm::mat3_to_quat(&rotation)
//         };
//
//         let euler = glm::quat_euler_angles(&orientation);
//         self.pitch = euler.x;
//         self.yaw = euler.y;
//         self.roll = euler.z;
//
//         Ok(())
//     }
//
//     fn apply_tilt(&mut self, delta: f32) {
//         self.roll += delta * self.rotation_speed_multiplier;
//     }
//
//     fn apply_mouse_delta(&mut self, x: f32, y: f32) {
//         let x = x * self.rotation_speed_multiplier;
//         let y = y * self.rotation_speed_multiplier;
//
//         self.yaw += x;
//         // Let us prevent gimbal lock
//         self.pitch = (self.pitch - y).clamp(-1.55334306, 1.55334306);
//     }
//
//     pub fn set_enabled(&mut self, enabled: bool) {
//         self.active = enabled;
//
//         // Detach from current cam location
//         if self.active {
//             let _ = self.initialize();
//         }
//     }
//
//     pub fn set_rotation_speed_multiplier(&mut self, value: f32) {
//         self.rotation_speed_multiplier = value;
//     }
//
//     pub fn set_movement_speed_multiplier(&mut self, value: f32) {
//         self.movement_speed_multiplier = value;
//     }
// }

pub fn is_game_focused() -> bool {
    let game_hwnd = HWND(
        unsafe { get_instance::<CSWindowImp>() }
            .unwrap()
            .window_handle as _,
    );
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return false;
    }

    // Compare directly if you have the HWND already
    if fg == game_hwnd {
        return true;
    }

    // Or fallback: check if foreground window belongs to our process
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(fg, Some(&mut pid)) };
    pid == unsafe { GetCurrentProcessId() }
}
