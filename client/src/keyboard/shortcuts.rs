
use std::sync::{Arc, RwLock};

use hbb_common::log;
use serde::{Deserialize, Serialize};

const LOCAL_CONFIG_KEY: &str = "keyboard-shortcuts";

lazy_static::lazy_static! {
    static ref CACHE: RwLock<Arc<Bindings>> = RwLock::new(Arc::new(Bindings::default()));
}

#[allow(dead_code)]
pub mod action_id {
    pub const SEND_CTRL_ALT_DEL: &str    = "send_ctrl_alt_del";
    pub const TOGGLE_FULLSCREEN: &str    = "toggle_fullscreen";
    pub const SWITCH_DISPLAY_NEXT: &str  = "switch_display_next";
    pub const SWITCH_DISPLAY_PREV: &str  = "switch_display_prev";
    pub const SWITCH_DISPLAY_ALL: &str   = "switch_display_all";
    pub const SCREENSHOT: &str           = "screenshot";
    pub const INSERT_LOCK: &str          = "insert_lock";
    pub const REFRESH: &str              = "refresh";
    pub const TOGGLE_BLOCK_INPUT: &str   = "toggle_block_input";
    pub const TOGGLE_RECORDING: &str     = "toggle_recording";
    pub const SWITCH_SIDES: &str         = "switch_sides";
    pub const CLOSE_TAB: &str            = "close_tab";
    pub const TOGGLE_TOOLBAR: &str       = "toggle_toolbar";
    pub const RESTART_REMOTE: &str       = "restart_remote";
    pub const RESET_CANVAS: &str         = "reset_canvas";
    pub const TOGGLE_MUTE: &str          = "toggle_mute";
    pub const PIN_TOOLBAR: &str          = "pin_toolbar";
    pub const VIEW_MODE_ORIGINAL: &str   = "view_mode_original";
    pub const VIEW_MODE_ADAPTIVE: &str   = "view_mode_adaptive";
    pub const TOGGLE_CHAT: &str               = "toggle_chat";
    pub const TOGGLE_QUALITY_MONITOR: &str    = "toggle_quality_monitor";
    pub const TOGGLE_SHOW_REMOTE_CURSOR: &str = "toggle_show_remote_cursor";
    pub const TOGGLE_SHOW_MY_CURSOR: &str     = "toggle_show_my_cursor";
    pub const TOGGLE_DISABLE_CLIPBOARD: &str  = "toggle_disable_clipboard";
    pub const PRIVACY_MODE_1: &str            = "privacy_mode_1";
    pub const PRIVACY_MODE_2: &str            = "privacy_mode_2";
    pub const KEYBOARD_MODE_MAP: &str         = "keyboard_mode_map";
    pub const KEYBOARD_MODE_TRANSLATE: &str   = "keyboard_mode_translate";
    pub const KEYBOARD_MODE_LEGACY: &str      = "keyboard_mode_legacy";
    pub const CODEC_AUTO: &str                = "codec_auto";
    pub const CODEC_VP8: &str                 = "codec_vp8";
    pub const CODEC_VP9: &str                 = "codec_vp9";
    pub const CODEC_AV1: &str                 = "codec_av1";
    pub const CODEC_H264: &str                = "codec_h264";
    pub const CODEC_H265: &str                = "codec_h265";
    pub const PLUG_OUT_ALL_VIRTUAL_DISPLAYS: &str = "plug_out_all_virtual_displays";
    pub const TOGGLE_RELATIVE_MOUSE_MODE: &str = "toggle_relative_mouse_mode";
    pub const TOGGLE_FOLLOW_REMOTE_CURSOR: &str = "toggle_follow_remote_cursor";
    pub const TOGGLE_FOLLOW_REMOTE_WINDOW: &str = "toggle_follow_remote_window";
    pub const TOGGLE_ZOOM_CURSOR: &str        = "toggle_zoom_cursor";
    pub const TOGGLE_REVERSE_MOUSE_WHEEL: &str = "toggle_reverse_mouse_wheel";
    pub const TOGGLE_SWAP_LEFT_RIGHT_MOUSE: &str = "toggle_swap_left_right_mouse";
    pub const TOGGLE_LOCK_AFTER_SESSION_END: &str = "toggle_lock_after_session_end";
    pub const TOGGLE_TRUE_COLOR: &str         = "toggle_true_color";
    pub const TOGGLE_SWAP_CTRL_CMD: &str      = "toggle_swap_ctrl_cmd";
    pub const TOGGLE_ENABLE_FILE_COPY_PASTE: &str = "toggle_enable_file_copy_paste";
    pub const VIEW_MODE_CUSTOM: &str          = "view_mode_custom";
    pub const IMAGE_QUALITY_BEST: &str        = "image_quality_best";
    pub const IMAGE_QUALITY_BALANCED: &str    = "image_quality_balanced";
    pub const IMAGE_QUALITY_LOW: &str         = "image_quality_low";
    pub const SEND_CLIPBOARD_KEYSTROKES: &str = "send_clipboard_keystrokes";
    pub const SWITCH_TAB_NEXT: &str           = "switch_tab_next";
    pub const SWITCH_TAB_PREV: &str           = "switch_tab_prev";
    pub const TOGGLE_VOICE_CALL: &str         = "toggle_voice_call";
    pub const TOGGLE_VIEW_ONLY: &str          = "toggle_view_only";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Primary,
    Ctrl,
    Alt,
    Shift,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub action: String,
    pub mods: Vec<Modifier>,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Bindings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub pass_through: bool,
    #[serde(default)]
    pub bindings: Vec<Binding>,
}

pub fn default_bindings() -> Vec<Binding> {
    let prefix = || vec![Modifier::Primary, Modifier::Alt, Modifier::Shift];
    vec![
        Binding { action: action_id::SEND_CTRL_ALT_DEL.into(),         mods: prefix(), key: "delete".into() },
        Binding { action: action_id::TOGGLE_FULLSCREEN.into(),         mods: prefix(), key: "enter".into() },
        Binding { action: action_id::SWITCH_DISPLAY_NEXT.into(),       mods: prefix(), key: "arrow_right".into() },
        Binding { action: action_id::SWITCH_DISPLAY_PREV.into(),       mods: prefix(), key: "arrow_left".into() },
        Binding { action: action_id::SCREENSHOT.into(),                mods: prefix(), key: "p".into() },
        Binding { action: action_id::TOGGLE_SHOW_REMOTE_CURSOR.into(), mods: prefix(), key: "m".into() },
        Binding { action: action_id::TOGGLE_MUTE.into(),               mods: prefix(), key: "s".into() },
        Binding { action: action_id::TOGGLE_BLOCK_INPUT.into(),        mods: prefix(), key: "i".into() },
        Binding { action: action_id::TOGGLE_CHAT.into(),               mods: prefix(), key: "c".into() },
    ]
}

pub fn match_normalized<'a>(key: &str, mods: &[Modifier], b: &'a Bindings) -> Option<&'a str> {
    if !b.enabled || b.pass_through {
        return None;
    }
    for binding in &b.bindings {
        if binding.mods.is_empty() {
            continue;
        }
        if binding.key == key && mods_equal(&binding.mods, mods) {
            return Some(binding.action.as_str());
        }
    }
    None
}

pub fn normalize_modifiers(alt: bool, ctrl: bool, shift: bool, command: bool) -> Vec<Modifier> {
    let mut v = Vec::new();
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        if command { v.push(Modifier::Primary); }
        if ctrl    { v.push(Modifier::Ctrl); }
    } else {
        if ctrl    { v.push(Modifier::Primary); }
    }
    if alt     { v.push(Modifier::Alt); }
    if shift   { v.push(Modifier::Shift); }
    v
}

pub fn event_to_key_name(event: &rdev::Event) -> Option<String> {
    use rdev::{EventType, Key};
    let key = match event.event_type {
        EventType::KeyPress(k) => k,
        _ => return None,
    };
    Some(match key {
        Key::Delete => "delete".into(),
        Key::Backspace => "backspace".into(),
        Key::Tab => "tab".into(),
        Key::Space => "space".into(),
        Key::Home => "home".into(),
        Key::End => "end".into(),
        Key::PageUp => "page_up".into(),
        Key::PageDown => "page_down".into(),
        Key::Insert => "insert".into(),
        Key::Return | Key::KpReturn => "enter".into(),
        Key::LeftArrow => "arrow_left".into(),
        Key::RightArrow => "arrow_right".into(),
        Key::UpArrow => "arrow_up".into(),
        Key::DownArrow => "arrow_down".into(),
        Key::KeyA => "a".into(),
        Key::KeyB => "b".into(),
        Key::KeyC => "c".into(),
        Key::KeyD => "d".into(),
        Key::KeyE => "e".into(),
        Key::KeyF => "f".into(),
        Key::KeyG => "g".into(),
        Key::KeyH => "h".into(),
        Key::KeyI => "i".into(),
        Key::KeyJ => "j".into(),
        Key::KeyK => "k".into(),
        Key::KeyL => "l".into(),
        Key::KeyM => "m".into(),
        Key::KeyN => "n".into(),
        Key::KeyO => "o".into(),
        Key::KeyP => "p".into(),
        Key::KeyQ => "q".into(),
        Key::KeyR => "r".into(),
        Key::KeyS => "s".into(),
        Key::KeyT => "t".into(),
        Key::KeyU => "u".into(),
        Key::KeyV => "v".into(),
        Key::KeyW => "w".into(),
        Key::KeyX => "x".into(),
        Key::KeyY => "y".into(),
        Key::KeyZ => "z".into(),
        Key::Num0 => "digit0".into(),
        Key::Num1 => "digit1".into(),
        Key::Num2 => "digit2".into(),
        Key::Num3 => "digit3".into(),
        Key::Num4 => "digit4".into(),
        Key::Num5 => "digit5".into(),
        Key::Num6 => "digit6".into(),
        Key::Num7 => "digit7".into(),
        Key::Num8 => "digit8".into(),
        Key::Num9 => "digit9".into(),
        Key::F1 => "f1".into(),
        Key::F2 => "f2".into(),
        Key::F3 => "f3".into(),
        Key::F4 => "f4".into(),
        Key::F5 => "f5".into(),
        Key::F6 => "f6".into(),
        Key::F7 => "f7".into(),
        Key::F8 => "f8".into(),
        Key::F9 => "f9".into(),
        Key::F10 => "f10".into(),
        Key::F11 => "f11".into(),
        Key::F12 => "f12".into(),
        _ => return None,
    })
}

pub fn reload_from_config() {
    let raw = hbb_common::config::LocalConfig::get_option(LOCAL_CONFIG_KEY);
    #[cfg(target_os = "linux")]
    let is_wayland = crate::platform::linux::current_is_wayland();
    #[cfg(not(target_os = "linux"))]
    let is_wayland = false;
    reload_from_raw(&raw, std::env::consts::OS, is_wayland);
}

fn reload_from_raw(raw: &str, platform: &str, is_wayland: bool) {
    let mut parsed: Bindings = if raw.is_empty() {
        Bindings::default()
    } else {
        match serde_json::from_str(&raw) {
            Ok(parsed) => parsed,
            Err(e) => {
                log::warn!("Failed to parse keyboard shortcut config: {}", e);
                Bindings::default()
            }
        }
    };
    parsed.bindings.retain(|b| action_available_on_platform(&b.action, platform, is_wayland));
    match CACHE.write() {
        Ok(mut w) => {
            *w = Arc::new(parsed);
        }
        Err(poison) => {
            log::error!("Keyboard shortcut cache write lock is poisoned");
            *poison.into_inner() = Arc::new(parsed);
        }
    }
}

fn action_available_on_platform(action: &str, platform: &str, is_wayland: bool) -> bool {
    let mobile = matches!(platform, "android" | "ios");
    match action {
        action_id::TOGGLE_RELATIVE_MOUSE_MODE => !mobile && !is_wayland,
        action_id::TOGGLE_FULLSCREEN | action_id::SCREENSHOT | action_id::SWITCH_TAB_NEXT
        | action_id::SWITCH_TAB_PREV | action_id::CLOSE_TAB | action_id::SWITCH_SIDES
        | action_id::TOGGLE_TOOLBAR | action_id::PIN_TOOLBAR | action_id::SWITCH_DISPLAY_ALL
        | action_id::TOGGLE_ENABLE_FILE_COPY_PASTE | action_id::KEYBOARD_MODE_MAP
        | action_id::KEYBOARD_MODE_TRANSLATE | action_id::KEYBOARD_MODE_LEGACY
        | action_id::TOGGLE_SHOW_MY_CURSOR | action_id::TOGGLE_ZOOM_CURSOR => !mobile,
        action_id::RESET_CANVAS => mobile,
        action_id::TOGGLE_RECORDING | action_id::TOGGLE_VOICE_CALL => platform != "ios",
        _ => true,
    }
}

pub fn current() -> Arc<Bindings> {
    match CACHE.read() {
        Ok(b) => Arc::clone(&b),
        Err(poison) => {
            log::error!("Keyboard shortcut cache read lock is poisoned");
            Arc::clone(&poison.into_inner())
        }
    }
}

pub fn match_event(event: &rdev::Event) -> Option<String> {
    let bindings = current();
    if !bindings.enabled || bindings.pass_through {
        return None;
    }
    let key_name = event_to_key_name(event)?;
    let (alt, ctrl, shift, command) =
        crate::keyboard::client::get_modifiers_state(false, false, false, false);
    let mods = normalize_modifiers(alt, ctrl, shift, command);
    let action = match_normalized(&key_name, &mods, &bindings)?;
    #[cfg(target_os = "windows")]
    if unsafe { super::IS_0X021D_DOWN }
        && super::MODIFIERS_STATE.lock().unwrap().get(&rdev::Key::AltGr) == Some(&true)
    {
        return None;
    }
    #[cfg(target_os = "linux")]
    if super::MODIFIERS_STATE.lock().unwrap().get(&rdev::Key::AltGr) == Some(&true) {
        match crate::platform::linux::right_alt_is_level3_shift() {
            Ok(true) => return None,
            Ok(false) => {}
            Err(err) => {
                hbb_common::throttled_log!(std::time::Duration::from_secs(5), warn,
                    "Cannot identify AltGraph for keyboard shortcuts: {}", err);
                return None;
            }
        }
    }
    Some(action.to_owned())
}

#[cfg(feature = "flutter")]
lazy_static::lazy_static! {
    static ref FIRED_KEYS: std::sync::Mutex<std::collections::HashSet<rdev::Key>> =
        Default::default();
    static ref RELEASE_ACTIONS: std::sync::Mutex<
        std::collections::HashMap<rdev::Key, (hbb_common::SessionID, String)>,
    > = Default::default();
    static ref RELEASED_MODIFIERS: std::sync::Mutex<
        std::collections::HashMap<hbb_common::SessionID, std::collections::HashMap<rdev::Key, rdev::Event>>,
    > = Default::default();
}

const RELEASE_ACTION_IDS: &[&str] = &[
    action_id::CLOSE_TAB,
    action_id::SWITCH_TAB_NEXT,
    action_id::SWITCH_TAB_PREV,
    action_id::RESTART_REMOTE,
    action_id::SWITCH_SIDES,
    action_id::TOGGLE_CHAT,
];

pub fn runs_on_release(action_id: &str) -> bool {
    action_id == action_id::SEND_CLIPBOARD_KEYSTROKES || RELEASE_ACTION_IDS.contains(&action_id)
}

#[cfg(feature = "flutter")]
pub fn clear_session_state(session_id: &hbb_common::SessionID) {
    RELEASE_ACTIONS.lock().unwrap().retain(|_, (sid, _)| sid != session_id);
    let held = RELEASED_MODIFIERS.lock().unwrap().remove(session_id);
    if let Some(held) = held {
        {
            let mut state = super::MODIFIERS_STATE.lock().unwrap();
            for key in held.keys() {
                state.insert(*key, false);
            }
        }
        let mut to_release = super::TO_RELEASE.lock().unwrap();
        for key in held.keys() {
            to_release.remove(key);
        }
    }
}

#[cfg(feature = "flutter")]
pub(super) fn release_modifier_on_leave(key: rdev::Key) -> bool {
    let sid = crate::flutter::get_cur_session_id();
    let mut released = RELEASED_MODIFIERS.lock().unwrap();
    let Some(held) = released.get_mut(&sid) else { return false };
    if held.remove(&key).is_none() {
        return false;
    }
    if held.is_empty() {
        released.remove(&sid);
    }
    drop(released);
    super::MODIFIERS_STATE.lock().unwrap().insert(key, false);
    true
}

#[cfg(feature = "flutter")]
pub fn enter_view_only(session_id: &hbb_common::SessionID) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if super::IS_RDEV_ENABLED.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    FIRED_KEYS.lock().unwrap().clear();
    RELEASE_ACTIONS.lock().unwrap().clear();
    clear_session_state(session_id);
}

#[cfg(feature = "flutter")]
pub fn transfer_released_modifiers(from: &hbb_common::SessionID, to: &hbb_common::SessionID) {
    if from == to {
        return;
    }
    let mut released = RELEASED_MODIFIERS.lock().unwrap();
    if let Some(held) = released.remove(from) {
        released.entry(*to).or_default().extend(held);
    }
}

#[cfg(feature = "flutter")]
pub fn try_dispatch(
    session_id: Option<&hbb_common::SessionID>,
    event: &rdev::Event,
    keyboard_mode: &str,
    peer: impl FnOnce() -> String,
    send: impl Fn(&base::message_proto::KeyEvent),
) -> bool {
    use rdev::EventType;
    let resolved;
    let sid = match session_id {
        Some(id) => id,
        None => {
            resolved = crate::flutter::get_cur_session_id();
            &resolved
        }
    };
    if let EventType::KeyRelease(key) = event.event_type {
        let mut released = RELEASED_MODIFIERS.lock().unwrap();
        if let Some(held) = released.get_mut(sid) {
            held.remove(&key);
            if held.is_empty() {
                released.remove(sid);
            }
        }
    }
    {
        let mut fired = FIRED_KEYS.lock().unwrap();
        match event.event_type {
            EventType::KeyPress(k) if fired.contains(&k) => return true,
            EventType::KeyRelease(k) if fired.remove(&k) => {
                drop(fired);
                let pending = RELEASE_ACTIONS.lock().unwrap().remove(&k);
                if let Some((sid, action_id)) = pending {
                    push_shortcut_event(&sid, &action_id);
                }
                return true;
            }
            _ => {}
        }
    }
    let action_id = match event.event_type {
        EventType::KeyPress(key) if super::TO_RELEASE.lock().unwrap().contains_key(&key) => None,
        _ => match_event(event),
    };
    let Some(action_id) = action_id else {
        restore_remote_modifiers(sid, event, keyboard_mode, peer, &send);
        return false;
    };
    let peer = peer();
    if runs_on_release(&action_id)
        || matches!(action_id.as_str(),
            action_id::TOGGLE_VIEW_ONLY | action_id::TOGGLE_SHOW_MY_CURSOR
            | action_id::INSERT_LOCK | action_id::SEND_CTRL_ALT_DEL
            | action_id::KEYBOARD_MODE_MAP | action_id::KEYBOARD_MODE_TRANSLATE
            | action_id::KEYBOARD_MODE_LEGACY)
    {
        release_remote_non_modifiers(keyboard_mode, &peer, &send);
    }
    release_remote_keys(sid, keyboard_mode, &peer, &send);
    if let EventType::KeyPress(k) = event.event_type {
        FIRED_KEYS.lock().unwrap().insert(k);
        let mut pending = RELEASE_ACTIONS.lock().unwrap();
        if runs_on_release(&action_id) {
            pending.insert(k, (*sid, action_id));
            return true;
        }
        pending.remove(&k);
    }
    push_shortcut_event(sid, &action_id);
    true
}

#[cfg(feature = "flutter")]
fn push_shortcut_event(session_id: &hbb_common::SessionID, action_id: &str) {
    if let Some(session) = crate::flutter::sessions::get_session_by_session_id(session_id) {
        session.ui_handler.push_event_to(
            "shortcut_triggered",
            &[("action", action_id)],
            &[session_id],
        );
    }
}

#[cfg(feature = "flutter")]
fn restore_remote_modifiers(
    session_id: &hbb_common::SessionID,
    event: &rdev::Event,
    keyboard_mode: &str,
    peer: impl FnOnce() -> String,
    send: &impl Fn(&base::message_proto::KeyEvent),
) {
    use super::{event_to_key_events, get_keyboard_mode_enum, is_modifier, MODIFIERS_STATE};

    let rdev::EventType::KeyPress(key) = event.event_type else { return };
    if is_modifier(&key) {
        return;
    }
    let held = RELEASED_MODIFIERS.lock().unwrap().remove(session_id);
    let Some(mut held) = held else { return };
    {
        let state = MODIFIERS_STATE.lock().unwrap();
        held.retain(|key, _| state.get(key).copied().unwrap_or(false));
    }
    if held.is_empty() {
        return;
    }
    let peer = peer();
    let mode = get_keyboard_mode_enum(keyboard_mode);
    for event in held.into_values() {
        for key_event in event_to_key_events(peer.clone(), &event, mode, None) {
            send(&key_event);
        }
    }
}

#[cfg(feature = "flutter")]
fn release_remote_non_modifiers(
    keyboard_mode: &str,
    peer: &str,
    send: &impl Fn(&base::message_proto::KeyEvent),
) {
    use super::{event_to_key_events, get_keyboard_mode_enum, is_modifier, TO_RELEASE};
    let held: Vec<_> = TO_RELEASE.lock().unwrap().iter()
        .filter(|(key, _)| !is_modifier(key))
        .map(|(key, event)| (*key, event.clone()))
        .collect();
    let mode = get_keyboard_mode_enum(keyboard_mode);
    for (key, mut event) in held {
        event.event_type = rdev::EventType::KeyRelease(key);
        for key_event in event_to_key_events(peer.to_owned(), &event, mode, None) {
            send(&key_event);
        }
    }
}

#[cfg(feature = "flutter")]
fn release_remote_keys(
    session_id: &hbb_common::SessionID,
    keyboard_mode: &str,
    peer: &str,
    send: &impl Fn(&base::message_proto::KeyEvent),
) {
    use super::{event_to_key_events, get_keyboard_mode_enum, is_modifier, MODIFIERS_STATE, TO_RELEASE};
    use rdev::{EventType, Key};

    let mode = get_keyboard_mode_enum(keyboard_mode);
    let to_release: Vec<_> = TO_RELEASE.lock().unwrap().iter()
        .filter(|(key, _)| is_modifier(key))
        .map(|(key, event)| (*key, event.clone()))
        .collect();
    let held: Vec<(Key, rdev::Event)> = {
        let state = MODIFIERS_STATE.lock().unwrap();
        to_release
            .iter()
            .filter(|(k, _)| state.get(k).copied().unwrap_or(false))
            .map(|(key, event)| (*key, event.clone()))
            .collect()
    };
    let swap_ctrl_cmd = crate::flutter::sessions::get_session_by_session_id(session_id)
        .is_some_and(|session| session.get_toggle_option("allow_swap_key".into()));
    let neutralize_menu = peer == "windows"
        && !to_release.is_empty()
        && to_release.iter().all(|(key, _)| matches!(key, Key::Alt | Key::AltGr)
            || (swap_ctrl_cmd && matches!(key, Key::ControlLeft | Key::ControlRight))
            || (!swap_ctrl_cmd && matches!(key, Key::MetaLeft | Key::MetaRight)));
    if neutralize_menu {
        send_windows_menu_ctrl(true, swap_ctrl_cmd, send);
    }
    for (key, mut event) in to_release {
        event.event_type = EventType::KeyRelease(key);
        for key_event in event_to_key_events(peer.to_owned(), &event, mode, None) {
            send(&key_event);
        }
    }
    if neutralize_menu {
        send_windows_menu_ctrl(false, swap_ctrl_cmd, send);
    }
    {
        let mut state = MODIFIERS_STATE.lock().unwrap();
        for (key, _) in &held {
            state.insert(*key, true);
        }
    }
    RELEASED_MODIFIERS.lock().unwrap().insert(
        *session_id, held.iter().cloned().collect()
    );
    TO_RELEASE.lock().unwrap().extend(held);
}

#[cfg(feature = "flutter")]
fn send_windows_menu_ctrl(
    down: bool,
    swap_ctrl_cmd: bool,
    send: &impl Fn(&base::message_proto::KeyEvent),
) {
    use base::message_proto::{KeyEvent, KeyboardMode};
    const LEFT_CTRL_SCAN_CODE: u32 = 0x1d;
    const LEFT_META_SCAN_CODE: u32 = 0xe05b;
    let mut event = KeyEvent::new();
    event.mode = KeyboardMode::Map.into();
    event.set_chr(if swap_ctrl_cmd {
        LEFT_META_SCAN_CODE
    } else {
        LEFT_CTRL_SCAN_CODE
    });
    event.down = down;
    send(&event);
}

fn mods_bits(m: &[Modifier]) -> u8 {
    let mut bits = 0u8;
    for x in m {
        bits |= match x {
            Modifier::Primary => 1,
            Modifier::Alt     => 2,
            Modifier::Shift   => 4,
            Modifier::Ctrl    => 8,
        };
    }
    bits
}

fn mods_equal(a: &[Modifier], b: &[Modifier]) -> bool {
    mods_bits(a) == mods_bits(b)
}
