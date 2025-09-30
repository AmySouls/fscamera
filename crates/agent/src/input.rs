use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::transmute;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossbeam::queue::SegQueue;
use retour::static_detour;
use windows::core::{s, w};
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::Foundation::LRESULT;
use windows::Win32::Foundation::WPARAM;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::LibraryLoader::GetProcAddress;
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
use windows::Win32::UI::Input::RegisterRawInputDevices;
use windows::Win32::UI::Input::HRAWINPUT;
use windows::Win32::UI::Input::RAWINPUT;
use windows::Win32::UI::Input::RAWINPUTDEVICE;
use windows::Win32::UI::Input::RAWINPUTHEADER;
use windows::Win32::UI::Input::RIDEV_INPUTSINK;
use windows::Win32::UI::Input::RID_INPUT;
use windows::Win32::UI::Input::RIM_TYPEMOUSE;
use windows::Win32::UI::Input::{GetRawInputData, RIM_TYPEKEYBOARD};
use windows::Win32::UI::WindowsAndMessaging::CallWindowProcW;
use windows::Win32::UI::WindowsAndMessaging::DefWindowProcW;
use windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW;
use windows::Win32::UI::WindowsAndMessaging::GWLP_WNDPROC;
use windows::Win32::UI::WindowsAndMessaging::WM_INPUT;
use windows::Win32::UI::WindowsAndMessaging::WNDPROC;

use windows::Win32::UI::Input::XboxController::XINPUT_STATE;

enum InputEvent {
    MouseDelta(i32, i32),
    Keyboard { key: u16 },
}

#[derive(Debug, Default)]
pub struct Input {
    orientation_delta: (f32, f32),
    movement_delta: (f32, f32, f32),

    debounce: HashMap<i32, Instant>,
}

type GetRawInputDataFn = unsafe extern "system" fn(
    h_raw_input: HRAWINPUT,
    ui_command: u32,
    p_data: *mut c_void,
    pcb_size: *mut u32,
    cb_size_header: u32,
) -> i32;

type XInputGetState = unsafe extern "system" fn(u32, *mut XINPUT_STATE) -> u32;

static_detour! {
    static GET_RAW_INPUT_DATA: unsafe extern "system" fn(
        HRAWINPUT, u32, *mut c_void, *mut u32, u32
    ) -> i32;

    // static DIRECT_INPUT_8_CREATE: unsafe extern "system" fn(HINSTANCE, u32, REFIID) -> u32;
}

/// Block input from going into the game.
static BLOCK_INPUT: AtomicBool = AtomicBool::new(false);

static INPUT_EVENTS: SegQueue<InputEvent> = SegQueue::new();

impl Input {
    pub fn update(&mut self) {
        // If no mouse movement events are available in the queue, the mouse did not move.
        self.orientation_delta = (0.0, 0.0);

        while let Some(event) = INPUT_EVENTS.pop() {
            match event {
                InputEvent::Keyboard { key } => {
                    // log::info!("Keyboard {key:?}");
                }
                InputEvent::MouseDelta(x, y) => {
                    self.orientation_delta = (
                        self.orientation_delta.0 + x as f32,
                        self.orientation_delta.1 + y as f32,
                    );
                }
            }
        }
    }

    pub fn key_pressed(&self, key: i32) -> bool {
        return unsafe { GetKeyState(key) } < 0;
    }

    pub fn key_pressed_debounced(&mut self, key: i32, timeout: Duration) -> bool {
        if self.key_pressed(key)
            && self
                .debounce
                .get(&key)
                .is_none_or(|e| (Instant::now() - *e) > timeout)
        {
            let _ = self.debounce.insert(key, Instant::now());
            return true;
        }

        false
    }

    pub fn orientation_delta(&self) -> (f32, f32) {
        self.orientation_delta
    }

    pub fn movement_delta(&self) -> (f32, f32, f32) {
        self.movement_delta
    }
}

pub unsafe fn setup_hook() {
    let target: GetRawInputDataFn = unsafe {
        let hinst: HMODULE = GetModuleHandleW(w!("user32")).expect("Could not locate user32");
        let target =
            GetProcAddress(hinst, s!("GetRawInputData")).expect("Could not locate GetRawInputData");
        transmute(target)
    };

    unsafe {
        GET_RAW_INPUT_DATA
            .initialize(
                target,
                |h_raw_input: HRAWINPUT,
                 ui_command: u32,
                 p_data: *mut c_void,
                 pcb_size: *mut u32,
                 cba_size_header: u32| {
                    let ret = GET_RAW_INPUT_DATA.call(
                        h_raw_input,
                        ui_command,
                        p_data,
                        pcb_size,
                        cba_size_header,
                    );

                    if ret > 0 && ui_command == RID_INPUT.0 && !p_data.is_null() {
                        let ri = unsafe { &mut *(p_data as *mut RAWINPUT) };

                        // "Good enough"
                        let block_input = BLOCK_INPUT.load(Ordering::Relaxed);
                        if ri.header.dwType == RIM_TYPEMOUSE.0 {
                            let mut mouse = ri.data.mouse;
                            INPUT_EVENTS.push(InputEvent::MouseDelta(mouse.lLastX, mouse.lLastY));

                            if block_input {
                                mouse.lLastX = 0;
                                mouse.lLastY = 0;
                            }
                        } else if ri.header.dwType == RIM_TYPEKEYBOARD.0 {
                            let mut keyboard = ri.data.keyboard;
                            INPUT_EVENTS.push(InputEvent::Keyboard { key: keyboard.VKey });

                            if block_input {
                                keyboard.VKey = 0;
                            }
                        }
                    }

                    ret
                },
            )
            .unwrap()
            .enable()
            .unwrap();
    }
}

// pub unsafe fn setup_dinput8_hook() {
//     let target: XInputGetState = unsafe {
//         let hinst: HMODULE = GetModuleHandleW(w!("DINPUT8")).expect("Could not locate DINPUT8");
//         let target =
//             GetProcAddress(hinst, s!("DirectInput8Create")).expect("Could not locate DirectInput8Create");
//         transmute(target)
//     };
//
//     unsafe {
//         XINPUT_GET_STATE
//             .initialize(target, |dw_user_index: u32, p_state: *mut XINPUT_STATE| {
//                 let ret = XINPUT_GET_STATE.call(dw_user_index, p_state);
//
//                 if ret == 0 {
//                     let state = &*p_state;
//                     let lx = state.Gamepad.sThumbLX;
//                     let ly = state.Gamepad.sThumbLY;
//
//                     let rx = state.Gamepad.sThumbRX;
//                     let ry = state.Gamepad.sThumbRY;
//
//                     // log::info!("XInputGetState: ({lx}, {ly}) ({rx}, {ry})");
//                 }
//
//                 ret
//             })
//             .unwrap()
//             .enable()
//             .unwrap();
//     }
// }
