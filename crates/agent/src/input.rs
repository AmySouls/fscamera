use std::ffi::c_void;
use std::mem::transmute;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam::queue::SegQueue;
use eldenring::cs::CSWindowImp;
use fromsoft_shared::get_instance;
use retour::static_detour;
use windows::core::{s, w};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::LibraryLoader::GetProcAddress;
use windows::Win32::UI::Input::HRAWINPUT;
use windows::Win32::UI::Input::RAWINPUT;
use windows::Win32::UI::Input::RID_INPUT;
use windows::Win32::UI::Input::RIM_TYPEKEYBOARD;
use windows::Win32::UI::Input::RIM_TYPEMOUSE;
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

enum InputEvent {
    MouseDelta(i32, i32),
    MouseWheel(i16),
    // KeyDown { key: u16 },
    // KeyUp { key: u16 },
}

#[derive(Debug, Default)]
pub enum InputBlockMode {
    #[default]
    None,
    KeyboardAndMouse,
    Gamepad,
}

#[derive(Debug)]
pub struct Input {
    orientation_delta: (f32, f32),
    mousewheel_delta: i16,

    keypress: [bool; KEYSPACE_SIZE],
    keydown: [bool; KEYSPACE_SIZE],
    keyup: [bool; KEYSPACE_SIZE],
}

const KEYSPACE_SIZE: usize = 256;
const RI_MOUSE_WHEEL: u16 = 0x0400;

impl Default for Input {
    fn default() -> Self {
        Self {
            orientation_delta: Default::default(),
            mousewheel_delta: Default::default(),

            keypress: [false; KEYSPACE_SIZE],
            keydown: [false; KEYSPACE_SIZE],
            keyup: [false; KEYSPACE_SIZE],
        }
    }
}

type GetRawInputDataFn = unsafe extern "system" fn(
    h_raw_input: HRAWINPUT,
    ui_command: u32,
    p_data: *mut c_void,
    pcb_size: *mut u32,
    cb_size_header: u32,
) -> i32;

static_detour! {
    static GET_RAW_INPUT_DATA: unsafe extern "system" fn(
        HRAWINPUT, u32, *mut c_void, *mut u32, u32
    ) -> i32;
}

/// Block input from going into the game.
static BLOCK_INPUT_KBM: AtomicBool = AtomicBool::new(false);

static INPUT_EVENTS: SegQueue<InputEvent> = SegQueue::new();

impl Input {
    pub fn update(&mut self) {
        // If no mouse movement events are available in the queue, the mouse did not move.
        self.orientation_delta = (0.0, 0.0);
        self.mousewheel_delta = 0;

        self.keydown.fill(false);
        self.keyup.fill(false);

        // Drain events even when inputs are posted while the game isn't focussed.
        if !is_game_focussed() {
            while let Some(_event) = INPUT_EVENTS.pop() {}
            self.keypress.fill(false);
            return;
        }


        let mut kbd = [0u8; 256];
        unsafe {
            let _ = GetKeyboardState(&mut kbd);
        }
        let prev = self.keypress;

        // Explicitly read the keyboard state as the GetRawInputData isn't consistently called when
        // the gamespeed is active.
        for i in 0..KEYSPACE_SIZE {
            let down_now = (kbd[i] & 0x80) != 0;
            let down_prev = prev[i];

            self.keypress[i] = down_now;
            self.keydown[i] = down_now && !down_prev;
            self.keyup[i] = !down_now && down_prev;
        }

        while let Some(event) = INPUT_EVENTS.pop() {
            match event {
                // InputEvent::KeyDown { key } => {
                //     // self.keyup[key as usize] = false;
                //     // self.keydown[key as usize] = true;
                //     // self.keypress[key as usize] = true;
                // }
                // InputEvent::KeyUp { key } => {
                //     // self.keyup[key as usize] = true;
                //     // self.keydown[key as usize] = false;
                //     // self.keypress[key as usize] = false;
                // }
                InputEvent::MouseDelta(x, y) => {
                    self.orientation_delta = (
                        self.orientation_delta.0 + x as f32,
                        self.orientation_delta.1 + y as f32,
                    );
                }
                InputEvent::MouseWheel(d) => {
                    self.mousewheel_delta = d;
                }
            }
        }
    }

    pub fn key_pressed(&self, key: i32) -> bool {
        self.keypress[key as usize]
    }

    pub fn key_down(&self, key: i32) -> bool {
        self.keydown[key as usize]
    }

    pub fn mousewheel_delta(&self) -> i16 {
        self.mousewheel_delta
    }

    pub fn orientation_delta(&self) -> (f32, f32) {
        self.orientation_delta
    }

    pub fn block_input(&mut self, target: InputBlockMode) {
        match target {
            InputBlockMode::None => BLOCK_INPUT_KBM.store(false, Ordering::Relaxed),
            InputBlockMode::KeyboardAndMouse => BLOCK_INPUT_KBM.store(true, Ordering::Relaxed),
            InputBlockMode::Gamepad => BLOCK_INPUT_KBM.store(false, Ordering::Relaxed),
        }
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
                        let ri = &mut *(p_data as *mut RAWINPUT);

                        // "Good enough"
                        let mut block_input = BLOCK_INPUT_KBM.load(Ordering::Relaxed);

                        if ri.header.dwType == RIM_TYPEMOUSE.0 {
                            let mouse = ri.data.mouse;

                            INPUT_EVENTS.push(InputEvent::MouseDelta(mouse.lLastX, mouse.lLastY));

                            let flags = mouse.Anonymous.Anonymous.usButtonFlags;
                            if flags & RI_MOUSE_WHEEL != 0 {
                                let delta = mouse.Anonymous.Anonymous.usButtonData as i16;
                                INPUT_EVENTS.push(InputEvent::MouseWheel(delta));
                            }

                            // if (flags & RI_MOUSE_HWHEEL.0) != 0 {
                            //     let delta = mouse.Anonymous.Anonymous.usButtonData as i16;
                            //     INPUT_EVENTS.push(InputEvent::MouseHWheel(delta));
                            // }
                        }
                        else if ri.header.dwType == RIM_TYPEKEYBOARD.0 {
                            let kbd = ri.data.keyboard;

                            // // Decide up/down via RI_KEY_BREAK (works for KEYDOWN and SYSKEYDOWN)
                            // let is_break = kbd.Flags & RI_KEY_BREAK as u16 != 0;
                            let key = normalize_vk(kbd.VKey, kbd.MakeCode, kbd.Flags);
                            //
                            // if !is_break {
                            //     INPUT_EVENTS.push(InputEvent::KeyDown { key });
                            // } else {
                            //     INPUT_EVENTS.push(InputEvent::KeyUp { key });
                            // }

                            // Do not filter escape
                            if key == 0x1B {
                                block_input = false;
                            }
                        }

                        if block_input {
                            return 0;
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

pub fn is_game_focussed() -> bool {
    let Some(cs_window) = (unsafe { get_instance::<CSWindowImp>() }) else {
        return false;
    };

    unsafe { GetForegroundWindow() }.0 as isize == cs_window.window_handle
}

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardState, MAPVK_VSC_TO_VK_EX, MapVirtualKeyExW, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_RCONTROL, VK_RMENU, VK_SHIFT
};
use windows::Win32::UI::WindowsAndMessaging::{RI_KEY_E0, RI_KEY_E1};

#[inline]
fn normalize_vk(vkey: u16, make_code: u16, flags: u16) -> u16 {
    // Refine VK using scan code + extended flag to get L/R variants.
    let hkl = unsafe { GetKeyboardLayout(0) };
    let mut sc = make_code as u32;
    if flags & RI_KEY_E0 as u16 != 0 {
        sc |= 0xE000;
    }
    if flags & RI_KEY_E1 as u16 != 0 {
        sc |= 0xE100;
    }
    let mapped = unsafe { MapVirtualKeyExW(sc, MAPVK_VSC_TO_VK_EX, Some(hkl)) } as u16;

    match vkey {
        x if x == VK_SHIFT.0 => {
            if mapped != 0 {
                mapped
            } else {
                VK_LSHIFT.0
            }
        }
        x if x == VK_CONTROL.0 => {
            if flags & RI_KEY_E0 as u16 != 0 {
                VK_RCONTROL.0
            } else {
                VK_LCONTROL.0
            }
        }
        x if x == VK_MENU.0 => {
            if flags & RI_KEY_E0 as u16 != 0 {
                VK_RMENU.0
            } else {
                VK_LMENU.0
            }
        }
        _ => {
            if mapped != 0 {
                mapped
            } else {
                vkey
            }
        }
    }
}
