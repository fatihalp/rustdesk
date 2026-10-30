
use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};

use hbb_common::libc::c_int;
use hbb_common::x11::xlib::{Display, XCloseDisplay, XGetPointerMapping, XOpenDisplay};
use libxdo_sys::{self, xdo_t, CURRENTWINDOW};
use std::{borrow::Cow, ffi::CString};

const DEFAULT_DELAY: u64 = 12000;

const MAX_DELAY: u64 = u32::MAX as u64;

fn mousebutton(button: MouseButton) -> c_int {
    match button {
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 3,
        MouseButton::ScrollUp => 4,
        MouseButton::ScrollDown => 5,
        MouseButton::ScrollLeft => 6,
        MouseButton::ScrollRight => 7,
        MouseButton::Back => 8,
        MouseButton::Forward => 9,
    }
}

const MIN_POINTER_BUTTONS: usize = 9;

fn check_x11_button_map() {
    if std::env::var_os("DISPLAY").is_none() {
        return;
    }

    let display: *mut Display = unsafe { XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        log::warn!("XOpenDisplay failed, cannot check button map");
        return;
    }

    let mut current_map = [0u8; 32];
    let nbuttons =
        unsafe { XGetPointerMapping(display, current_map.as_mut_ptr(), current_map.len() as i32) };
    unsafe { XCloseDisplay(display) };

    if nbuttons < 0 {
        log::warn!("XGetPointerMapping failed (returned {nbuttons})");
        return;
    }

    let nbuttons = nbuttons as usize;
    if nbuttons >= MIN_POINTER_BUTTONS {
        log::info!("X11 pointer has {nbuttons} buttons, side buttons supported");
    } else {
        log::warn!(
            "X11 pointer has only {nbuttons} buttons (need {MIN_POINTER_BUTTONS}); \
             back/forward side buttons may not work until a device with more buttons is added"
        );
    }
}

pub(super) struct EnigoXdo {
    xdo: *mut xdo_t,
    delay: u64,
}
unsafe impl Send for EnigoXdo {}

impl Default for EnigoXdo {
    fn default() -> Self {
        let xdo = unsafe { libxdo_sys::xdo_new(std::ptr::null()) };
        if xdo.is_null() {
            log::warn!("Failed to create xdo context, xdo functions will be disabled");
        } else {
            log::info!("xdo context created successfully");
            check_x11_button_map();
        }
        Self {
            xdo,
            delay: DEFAULT_DELAY,
        }
    }
}

impl EnigoXdo {
    pub fn delay(&self) -> u64 {
        self.delay
    }

    pub fn set_delay(&mut self, delay: u64) {
        self.delay = delay.min(MAX_DELAY);
        if delay > MAX_DELAY {
            log::warn!(
                "delay value {} exceeds maximum {}, clamped",
                delay,
                MAX_DELAY
            );
        }
    }
}

impl Drop for EnigoXdo {
    fn drop(&mut self) {
        if !self.xdo.is_null() {
            unsafe {
                libxdo_sys::xdo_free(self.xdo);
            }
        }
    }
}

impl MouseControllable for EnigoXdo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn mouse_move_to(&mut self, x: i32, y: i32) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            libxdo_sys::xdo_move_mouse(self.xdo as *const _, x, y, 0);
        }
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            libxdo_sys::xdo_move_mouse_relative(self.xdo as *const _, x, y);
        }
    }

    fn mouse_down(&mut self, button: MouseButton) -> crate::ResultType {
        if self.xdo.is_null() {
            return Ok(());
        }
        unsafe {
            libxdo_sys::xdo_mouse_down(self.xdo as *const _, CURRENTWINDOW, mousebutton(button));
        }
        Ok(())
    }

    fn mouse_up(&mut self, button: MouseButton) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            libxdo_sys::xdo_mouse_up(self.xdo as *const _, CURRENTWINDOW, mousebutton(button));
        }
    }

    fn mouse_click(&mut self, button: MouseButton) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            libxdo_sys::xdo_click_window(self.xdo as *const _, CURRENTWINDOW, mousebutton(button));
        }
    }

    fn mouse_scroll_x(&mut self, length: i32) {
        let button;
        let mut length = length;

        if length < 0 {
            button = MouseButton::ScrollLeft;
        } else {
            button = MouseButton::ScrollRight;
        }

        if length < 0 {
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
        }
    }

    fn mouse_scroll_y(&mut self, length: i32) {
        let button;
        let mut length = length;

        if length < 0 {
            button = MouseButton::ScrollUp;
        } else {
            button = MouseButton::ScrollDown;
        }

        if length < 0 {
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
        }
    }
}

fn keysequence<'a>(key: Key) -> Cow<'a, str> {
    if let Key::Layout(c) = key {
        return Cow::Owned(format!("U{:X}", c as u32));
    }
    if let Key::Raw(k) = key {
        return Cow::Owned(format!("{}", k as u16));
    }
    #[allow(deprecated)]
    Cow::Borrowed(match key {
        Key::Alt => "Alt",
        Key::Backspace => "BackSpace",
        Key::CapsLock => "Caps_Lock",
        Key::Control => "Control",
        Key::Delete => "Delete",
        Key::DownArrow => "Down",
        Key::End => "End",
        Key::Escape => "Escape",
        Key::F1 => "F1",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::Home => "Home",
        Key::LeftArrow => "Left",
        Key::Option => "Option",
        Key::PageDown => "Page_Down",
        Key::PageUp => "Page_Up",
        Key::Return => "Return",
        Key::RightArrow => "Right",
        Key::Shift => "Shift",
        Key::Space => "space",
        Key::Tab => "Tab",
        Key::UpArrow => "Up",
        Key::Numpad0 => "U30",
        Key::Numpad1 => "U31",
        Key::Numpad2 => "U32",
        Key::Numpad3 => "U33",
        Key::Numpad4 => "U34",
        Key::Numpad5 => "U35",
        Key::Numpad6 => "U36",
        Key::Numpad7 => "U37",
        Key::Numpad8 => "U38",
        Key::Numpad9 => "U39",
        Key::Decimal => "U2E",
        Key::Cancel => "Cancel",
        Key::Clear => "Clear",
        Key::Pause => "Pause",
        Key::Kana => "Kana",
        Key::Hangul => "Hangul",
        Key::Junja => "",
        Key::Final => "",
        Key::Hanja => "Hanja",
        Key::Kanji => "Kanji",
        Key::Convert => "",
        Key::Select => "Select",
        Key::Print => "Print",
        Key::Execute => "Execute",
        Key::Snapshot => "3270_PrintScreen",
        Key::Insert => "Insert",
        Key::Help => "Help",
        Key::Sleep => "",
        Key::Separator => "KP_Separator",
        Key::VolumeUp => "",
        Key::VolumeDown => "",
        Key::Mute => "",
        Key::Scroll => "Scroll_Lock",
        Key::NumLock => "Num_Lock",
        Key::RWin => "Super_R",
        Key::Apps => "Menu",
        Key::Multiply => "KP_Multiply",
        Key::Add => "KP_Add",
        Key::Subtract => "KP_Subtract",
        Key::Divide => "KP_Divide",
        Key::Equals => "KP_Equal",
        Key::NumpadEnter => "KP_Enter",
        Key::RightShift => "Shift_R",
        Key::RightControl => "Control_R",
        Key::RightAlt => "Alt_R",

        Key::Command | Key::Super | Key::Windows | Key::Meta => "Super",

        _ => "",
    })
}

impl KeyboardControllable for EnigoXdo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_key_state(&mut self, key: Key) -> bool {
        if self.xdo.is_null() {
            return false;
        }
        let mod_shift = 1 << 0;
        let mod_lock = 1 << 1;
        let mod_control = 1 << 2;
        let mod_alt = 1 << 3;
        let mod_numlock = 1 << 4;
        let mod_meta = 1 << 6;
        let mask = unsafe { libxdo_sys::xdo_get_input_state(self.xdo as *const _) };
        match key {
            Key::Shift => mask & mod_shift != 0,
            Key::CapsLock => mask & mod_lock != 0,
            Key::Control => mask & mod_control != 0,
            Key::Alt => mask & mod_alt != 0,
            Key::NumLock => mask & mod_numlock != 0,
            Key::Meta => mask & mod_meta != 0,
            _ => false,
        }
    }

    fn key_sequence(&mut self, sequence: &str) {
        if self.xdo.is_null() {
            return;
        }
        if let Ok(string) = CString::new(sequence) {
            unsafe {
                libxdo_sys::xdo_enter_text_window(
                    self.xdo as *const _,
                    CURRENTWINDOW,
                    string.as_ptr(),
                    self.delay as libxdo_sys::useconds_t,
                );
            }
        }
    }

    fn key_down(&mut self, key: Key) -> crate::ResultType {
        if self.xdo.is_null() {
            return Ok(());
        }
        let string = CString::new(&*keysequence(key))?;
        unsafe {
            libxdo_sys::xdo_send_keysequence_window_down(
                self.xdo as *const _,
                CURRENTWINDOW,
                string.as_ptr(),
                self.delay as libxdo_sys::useconds_t,
            );
        }
        Ok(())
    }

    fn key_up(&mut self, key: Key) {
        if self.xdo.is_null() {
            return;
        }
        if let Ok(string) = CString::new(&*keysequence(key)) {
            unsafe {
                libxdo_sys::xdo_send_keysequence_window_up(
                    self.xdo as *const _,
                    CURRENTWINDOW,
                    string.as_ptr(),
                    self.delay as libxdo_sys::useconds_t,
                );
            }
        }
    }

    fn key_click(&mut self, key: Key) {
        if self.xdo.is_null() {
            return;
        }
        if let Ok(string) = CString::new(&*keysequence(key)) {
            unsafe {
                libxdo_sys::xdo_send_keysequence_window(
                    self.xdo as *const _,
                    CURRENTWINDOW,
                    string.as_ptr(),
                    self.delay as libxdo_sys::useconds_t,
                );
            }
        }
    }

    fn key_sequence_parse(&mut self, sequence: &str)
    where
        Self: Sized,
    {
        if let Err(..) = self.key_sequence_parse_try(sequence) {
            println!("Could not parse sequence");
        }
    }

    fn key_sequence_parse_try(&mut self, sequence: &str) -> Result<(), crate::dsl::ParseError>
    where
        Self: Sized,
    {
        crate::dsl::eval(self, sequence)
    }
}
