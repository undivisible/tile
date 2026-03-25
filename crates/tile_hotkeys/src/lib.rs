//! Global hotkey registration using Carbon's RegisterEventHotKey.
//!
//! This is the same approach used by Rectangle, Amethyst, and other macOS
//! tiling window managers. We link against Carbon directly since objc2
//! does not yet wrap the Carbon Event Manager.

use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;

pub mod keycodes;

use keycodes::*;
pub use tile_core::TileAction;

// Carbon Event Manager FFI
#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn GetApplicationEventTarget() -> *mut c_void;
    fn RegisterEventHotKey(
        hot_key_code: u32,
        hot_key_modifiers: u32,
        hot_key_id: EventHotKeyID,
        target: *mut c_void,
        options: u32,
        out_ref: *mut *mut c_void,
    ) -> i32;
    fn UnregisterEventHotKey(hot_key_ref: *mut c_void) -> i32;
    fn InstallEventHandler(
        target: *mut c_void,
        handler: EventHandlerProcPtr,
        num_types: u32,
        list: *const EventTypeSpec,
        user_data: *mut c_void,
        handler_ref: *mut *mut c_void,
    ) -> i32;
    fn GetEventParameter(
        event: *mut c_void,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        buf_size: u32,
        actual_size: *mut u32,
        data: *mut c_void,
    ) -> i32;
}

type EventHandlerProcPtr =
    extern "C" fn(handler: *mut c_void, event: *mut c_void, user_data: *mut c_void) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EventHotKeyID {
    pub signature: u32,
    pub id: u32,
}

// Carbon constants
const K_EVENT_CLASS_KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const K_EVENT_HOT_KEY_PRESSED: u32 = 5;
const K_EVENT_PARAM_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
const TYPE_EVENT_HOT_KEY_ID: u32 = u32::from_be_bytes(*b"hkid");

// Modifier flags (Carbon-style)
const CMD_KEY: u32 = 1 << 8;
const SHIFT_KEY: u32 = 1 << 9;
const OPTION_KEY: u32 = 1 << 11;
const CONTROL_KEY: u32 = 1 << 12;

// Our signature for identifying hotkeys
const TILE_SIGNATURE: u32 = u32::from_be_bytes(*b"TILE");

/// Callback for when a hotkey is pressed.
pub type HotkeyCallback = Box<dyn Fn(TileAction) + Send + 'static>;

/// Global state for the hotkey handler.
static HOTKEY_STATE: Mutex<Option<HotkeyState>> = Mutex::new(None);

struct HotkeyState {
    callback: HotkeyCallback,
    action_map: HashMap<u32, TileAction>,
}

/// Registered hotkey handle.
struct RegisteredHotkey {
    _ref: *mut c_void,
}

// SAFETY: The hotkey ref is only used on the main thread
unsafe impl Send for RegisteredHotkey {}

/// The hotkey manager. Registers all hotkeys and dispatches to a callback.
pub struct HotkeyManager {
    hotkeys: Vec<RegisteredHotkey>,
    _handler_ref: *mut c_void,
}

// SAFETY: Handler ref is only used on the main thread
unsafe impl Send for HotkeyManager {}

impl HotkeyManager {
    /// Create a new hotkey manager and register all Rectangle-compatible shortcuts.
    pub fn new(callback: HotkeyCallback) -> Result<Self, String> {
        let mut action_map = HashMap::new();
        let bindings = Self::default_bindings();

        // Store action map
        for (idx, (_, _, action)) in bindings.iter().enumerate() {
            action_map.insert(idx as u32, *action);
        }

        // Install the event handler first
        let mut handler_ref: *mut c_void = std::ptr::null_mut();
        let event_type = EventTypeSpec {
            event_class: K_EVENT_CLASS_KEYBOARD,
            event_kind: K_EVENT_HOT_KEY_PRESSED,
        };

        // Store state globally for the C callback
        {
            let mut state = HOTKEY_STATE.lock().unwrap();
            *state = Some(HotkeyState {
                callback,
                action_map,
            });
        }

        let err = unsafe {
            InstallEventHandler(
                GetApplicationEventTarget(),
                hotkey_handler,
                1,
                &event_type,
                std::ptr::null_mut(),
                &mut handler_ref,
            )
        };

        if err != 0 {
            return Err(format!("Failed to install event handler: {}", err));
        }

        // Register all hotkeys
        let mut hotkeys = Vec::new();
        for (idx, (keycode, modifiers, action)) in bindings.iter().enumerate() {
            let mut hotkey_ref: *mut c_void = std::ptr::null_mut();
            let hotkey_id = EventHotKeyID {
                signature: TILE_SIGNATURE,
                id: idx as u32,
            };

            let err = unsafe {
                RegisterEventHotKey(
                    *keycode,
                    *modifiers,
                    hotkey_id,
                    GetApplicationEventTarget(),
                    0,
                    &mut hotkey_ref,
                )
            };

            if err != 0 {
                warn!(
                    "Failed to register hotkey {:?}: error {}",
                    action, err
                );
            } else {
                debug!("Registered hotkey {:?}", action);
                hotkeys.push(RegisteredHotkey { _ref: hotkey_ref });
            }
        }

        info!("Registered {} hotkeys", hotkeys.len());

        Ok(Self {
            hotkeys,
            _handler_ref: handler_ref,
        })
    }

    /// Default Rectangle-compatible keybindings.
    fn default_bindings() -> Vec<(u32, u32, TileAction)> {
        vec![
            // Halves: Ctrl+Opt+Arrow
            (K_VK_LEFT_ARROW, CONTROL_KEY | OPTION_KEY, TileAction::LeftHalf),
            (K_VK_RIGHT_ARROW, CONTROL_KEY | OPTION_KEY, TileAction::RightHalf),
            (K_VK_UP_ARROW, CONTROL_KEY | OPTION_KEY, TileAction::TopHalf),
            (K_VK_DOWN_ARROW, CONTROL_KEY | OPTION_KEY, TileAction::BottomHalf),
            // Thirds: Ctrl+Opt+D/F/G
            (K_VK_D, CONTROL_KEY | OPTION_KEY, TileAction::LeftThird),
            (K_VK_F, CONTROL_KEY | OPTION_KEY, TileAction::CenterThird),
            (K_VK_G, CONTROL_KEY | OPTION_KEY, TileAction::RightThird),
            // Two-thirds: Ctrl+Opt+E/R/T (note: R is not standard Rectangle, but fits the pattern)
            (K_VK_E, CONTROL_KEY | OPTION_KEY, TileAction::LeftTwoThirds),
            (K_VK_R, CONTROL_KEY | OPTION_KEY, TileAction::CenterTwoThirds),
            (K_VK_T, CONTROL_KEY | OPTION_KEY, TileAction::RightTwoThirds),
            // Quarters: Ctrl+Opt+U/I/J/K
            (K_VK_U, CONTROL_KEY | OPTION_KEY, TileAction::TopLeftQuarter),
            (K_VK_I, CONTROL_KEY | OPTION_KEY, TileAction::TopRightQuarter),
            (K_VK_J, CONTROL_KEY | OPTION_KEY, TileAction::BottomLeftQuarter),
            (K_VK_K, CONTROL_KEY | OPTION_KEY, TileAction::BottomRightQuarter),
            // Special: Ctrl+Opt+Return/C/Backspace/=/Z
            (K_VK_RETURN, CONTROL_KEY | OPTION_KEY, TileAction::Maximize),
            (K_VK_C, CONTROL_KEY | OPTION_KEY, TileAction::Center),
            (K_VK_DELETE, CONTROL_KEY | OPTION_KEY, TileAction::Restore),
            (K_VK_EQUAL, CONTROL_KEY | OPTION_KEY, TileAction::EqualizeAll),
            (K_VK_Z, CONTROL_KEY | OPTION_KEY, TileAction::ToggleZoom),
            // Move: Ctrl+Opt+Shift+Arrow
            (K_VK_LEFT_ARROW, CONTROL_KEY | OPTION_KEY | SHIFT_KEY, TileAction::MovePaneLeft),
            (K_VK_RIGHT_ARROW, CONTROL_KEY | OPTION_KEY | SHIFT_KEY, TileAction::MovePaneRight),
            (K_VK_UP_ARROW, CONTROL_KEY | OPTION_KEY | SHIFT_KEY, TileAction::MovePaneUp),
            (K_VK_DOWN_ARROW, CONTROL_KEY | OPTION_KEY | SHIFT_KEY, TileAction::MovePaneDown),
            // Swap: Ctrl+Opt+Cmd+Arrow
            (K_VK_LEFT_ARROW, CONTROL_KEY | OPTION_KEY | CMD_KEY, TileAction::SwapPaneLeft),
            (K_VK_RIGHT_ARROW, CONTROL_KEY | OPTION_KEY | CMD_KEY, TileAction::SwapPaneRight),
            (K_VK_UP_ARROW, CONTROL_KEY | OPTION_KEY | CMD_KEY, TileAction::SwapPaneUp),
            (K_VK_DOWN_ARROW, CONTROL_KEY | OPTION_KEY | CMD_KEY, TileAction::SwapPaneDown),
        ]
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        for hk in &self.hotkeys {
            unsafe {
                UnregisterEventHotKey(hk._ref);
            }
        }
        let mut state = HOTKEY_STATE.lock().unwrap();
        *state = None;
    }
}

/// The C callback for hotkey events.
extern "C" fn hotkey_handler(
    _handler: *mut c_void,
    event: *mut c_void,
    _user_data: *mut c_void,
) -> i32 {
    let mut hotkey_id = EventHotKeyID {
        signature: 0,
        id: 0,
    };

    let err = unsafe {
        GetEventParameter(
            event,
            K_EVENT_PARAM_DIRECT_OBJECT,
            TYPE_EVENT_HOT_KEY_ID,
            std::ptr::null_mut(),
            std::mem::size_of::<EventHotKeyID>() as u32,
            std::ptr::null_mut(),
            &mut hotkey_id as *mut _ as *mut c_void,
        )
    };

    if err != 0 {
        error!("Failed to get hotkey ID from event: {}", err);
        return -1; // eventNotHandledErr
    }

    if hotkey_id.signature != TILE_SIGNATURE {
        return -1;
    }

    let state = HOTKEY_STATE.lock().unwrap();
    if let Some(state) = state.as_ref() {
        if let Some(action) = state.action_map.get(&hotkey_id.id) {
            debug!("Hotkey pressed: {:?}", action);
            (state.callback)(*action);
            return 0; // noErr
        }
    }

    -1
}
