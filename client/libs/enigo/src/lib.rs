

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;


#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
pub use win::Enigo;
#[cfg(target_os = "windows")]
pub use win::ENIGO_INPUT_EXTRA_VALUE;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::Enigo;
#[cfg(target_os = "macos")]
pub use macos::ENIGO_INPUT_EXTRA_VALUE;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use crate::linux::Enigo;

pub mod dsl;

#[cfg(feature = "with_serde")]
#[macro_use]
extern crate serde_derive;

#[cfg(feature = "with_serde")]
extern crate serde;

pub type ResultType = std::result::Result<(), Box<dyn std::error::Error>>;

#[cfg_attr(feature = "with_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,

    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
}

pub trait MouseControllable {
    fn as_any(&self) -> &dyn std::any::Any;

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any;

    fn mouse_move_to(&mut self, x: i32, y: i32);

    fn mouse_move_relative(&mut self, x: i32, y: i32);

    fn mouse_down(&mut self, button: MouseButton) -> ResultType;

    fn mouse_up(&mut self, button: MouseButton);

    fn mouse_click(&mut self, button: MouseButton);

    fn mouse_scroll_x(&mut self, length: i32);

    fn mouse_scroll_y(&mut self, length: i32);
}

#[cfg_attr(feature = "with_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Alt,
    Backspace,
    CapsLock,
    Command,
    Control,
    Delete,
    DownArrow,
    End,
    Escape,
    F1,
    F10,
    F11,
    F12,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    Home,
    LeftArrow,
    Meta,
    Option,
    PageDown,
    PageUp,
    Return,
    RightArrow,
    Shift,
    Space,
    Super,
    Tab,
    UpArrow,
    Windows,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    Cancel,
    Clear,
    Pause,
    Kana,
    Hangul,
    Junja,
    Final,
    Hanja,
    Kanji,
    Convert,
    Select,
    Print,
    Execute,
    Snapshot,
    Insert,
    Help,
    Sleep,
    Separator,
    VolumeUp,
    VolumeDown,
    Mute,
    Scroll,
    NumLock,
    RWin,
    Apps,
    Multiply,
    Add,
    Subtract,
    Decimal,
    Divide,
    Equals,
    NumpadEnter,
    RightShift,
    RightControl,
    RightAlt,
    Layout(char),
    Raw(u16),
}

pub trait KeyboardControllable {
    fn as_any(&self) -> &dyn std::any::Any;

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any;

    fn key_sequence_parse(&mut self, sequence: &str)
    where
        Self: Sized,
    {
        if let Err(..) = self.key_sequence_parse_try(sequence) {
            println!("Could not parse sequence");
        }
    }
    fn key_sequence_parse_try(&mut self, sequence: &str) -> Result<(), dsl::ParseError>
    where
        Self: Sized,
    {
        dsl::eval(self, sequence)
    }

    fn key_sequence(&mut self, sequence: &str);

    fn key_down(&mut self, key: Key) -> ResultType;

    fn key_up(&mut self, key: Key);

    fn key_click(&mut self, key: Key);

    fn get_key_state(&mut self, key: Key) -> bool;
}

#[cfg(any(target_os = "android", target_os = "ios"))]
struct Enigo;

impl Enigo {
    pub fn new() -> Self {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        return Enigo {};
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Self::default()
    }
}

use std::fmt;

impl fmt::Debug for Enigo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Enigo")
    }
}
