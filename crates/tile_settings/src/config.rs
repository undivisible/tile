//! Keybind configuration: load, save, and default bindings.

use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tile_core::TileAction;

/// A serializable keybinding: modifier flags + keycode → action name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub keycode: u32,
    pub modifiers: u32,
}

/// Full configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileConfig {
    /// Map from action name (e.g. "LeftHalf") to its keybinding.
    pub bindings: BTreeMap<String, KeyBinding>,
    /// Gap between windows (outer).
    #[serde(default = "default_gap")]
    pub gap_outer: f64,
    /// Gap between windows (inner).
    #[serde(default = "default_gap")]
    pub gap_inner: f64,
}

fn default_gap() -> f64 {
    8.0
}

// Carbon modifier constants (same as tile_hotkeys)
pub const CMD_KEY: u32 = 1 << 8;
pub const SHIFT_KEY: u32 = 1 << 9;
pub const OPTION_KEY: u32 = 1 << 11;
pub const CONTROL_KEY: u32 = 1 << 12;

impl TileConfig {
    /// Path to the config file: ~/.config/tile/config.json
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("tile")
            .join("config.json")
    }

    /// Load config from disk, falling back to defaults.
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(config) => {
                        info!("Loaded config from {}", path.display());
                        return config;
                    }
                    Err(e) => {
                        warn!("Failed to parse config: {}, using defaults", e);
                    }
                },
                Err(e) => {
                    warn!("Failed to read config: {}, using defaults", e);
                }
            }
        }
        Self::default()
    }

    /// Save config to disk.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {}", e))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        std::fs::write(&path, json)
            .map_err(|e| format!("Failed to write config: {}", e))?;
        info!("Saved config to {}", path.display());
        Ok(())
    }

    /// Convert to the (keycode, modifiers, TileAction) tuples that HotkeyManager expects.
    pub fn to_bindings(&self) -> Vec<(u32, u32, TileAction)> {
        let mut result = Vec::new();
        for (name, binding) in &self.bindings {
            if let Some(action) = action_from_name(name) {
                result.push((binding.keycode, binding.modifiers, action));
            }
        }
        result
    }
}

impl Default for TileConfig {
    fn default() -> Self {
        let mut bindings = BTreeMap::new();

        // Use the same defaults as HotkeyManager::default_bindings()
        let defaults = default_binding_list();
        for (name, keycode, modifiers) in defaults {
            bindings.insert(name.to_string(), KeyBinding { keycode, modifiers });
        }

        Self {
            bindings,
            gap_outer: 8.0,
            gap_inner: 8.0,
        }
    }
}

/// All default bindings as (action_name, keycode, modifiers).
pub fn default_binding_list() -> Vec<(&'static str, u32, u32)> {
    use crate::config::keycodes::*;
    vec![
        // Halves
        ("LeftHalf", K_VK_LEFT_ARROW, CONTROL_KEY | OPTION_KEY),
        ("RightHalf", K_VK_RIGHT_ARROW, CONTROL_KEY | OPTION_KEY),
        ("TopHalf", K_VK_UP_ARROW, CONTROL_KEY | OPTION_KEY),
        ("BottomHalf", K_VK_DOWN_ARROW, CONTROL_KEY | OPTION_KEY),
        // Thirds
        ("LeftThird", K_VK_D, CONTROL_KEY | OPTION_KEY),
        ("CenterThird", K_VK_F, CONTROL_KEY | OPTION_KEY),
        ("RightThird", K_VK_G, CONTROL_KEY | OPTION_KEY),
        // Two-thirds
        ("LeftTwoThirds", K_VK_E, CONTROL_KEY | OPTION_KEY),
        ("CenterTwoThirds", K_VK_R, CONTROL_KEY | OPTION_KEY),
        ("RightTwoThirds", K_VK_T, CONTROL_KEY | OPTION_KEY),
        // Quarters
        ("TopLeftQuarter", K_VK_U, CONTROL_KEY | OPTION_KEY),
        ("TopRightQuarter", K_VK_I, CONTROL_KEY | OPTION_KEY),
        ("BottomLeftQuarter", K_VK_J, CONTROL_KEY | OPTION_KEY),
        ("BottomRightQuarter", K_VK_K, CONTROL_KEY | OPTION_KEY),
        // Special
        ("Maximize", K_VK_RETURN, CONTROL_KEY | OPTION_KEY),
        ("Center", K_VK_C, CONTROL_KEY | OPTION_KEY),
        ("Restore", K_VK_DELETE, CONTROL_KEY | OPTION_KEY),
        ("EqualizeAll", K_VK_EQUAL, CONTROL_KEY | OPTION_KEY),
        ("ToggleZoom", K_VK_Z, CONTROL_KEY | OPTION_KEY),
        // Move
        ("MovePaneLeft", K_VK_LEFT_ARROW, CONTROL_KEY | OPTION_KEY | SHIFT_KEY),
        ("MovePaneRight", K_VK_RIGHT_ARROW, CONTROL_KEY | OPTION_KEY | SHIFT_KEY),
        ("MovePaneUp", K_VK_UP_ARROW, CONTROL_KEY | OPTION_KEY | SHIFT_KEY),
        ("MovePaneDown", K_VK_DOWN_ARROW, CONTROL_KEY | OPTION_KEY | SHIFT_KEY),
        // Swap
        ("SwapPaneLeft", K_VK_LEFT_ARROW, CONTROL_KEY | OPTION_KEY | CMD_KEY),
        ("SwapPaneRight", K_VK_RIGHT_ARROW, CONTROL_KEY | OPTION_KEY | CMD_KEY),
        ("SwapPaneUp", K_VK_UP_ARROW, CONTROL_KEY | OPTION_KEY | CMD_KEY),
        ("SwapPaneDown", K_VK_DOWN_ARROW, CONTROL_KEY | OPTION_KEY | CMD_KEY),
    ]
}

/// Convert a TileAction to its serialized name.
pub fn action_name(action: TileAction) -> &'static str {
    match action {
        TileAction::LeftHalf => "LeftHalf",
        TileAction::RightHalf => "RightHalf",
        TileAction::TopHalf => "TopHalf",
        TileAction::BottomHalf => "BottomHalf",
        TileAction::LeftThird => "LeftThird",
        TileAction::CenterThird => "CenterThird",
        TileAction::RightThird => "RightThird",
        TileAction::LeftTwoThirds => "LeftTwoThirds",
        TileAction::CenterTwoThirds => "CenterTwoThirds",
        TileAction::RightTwoThirds => "RightTwoThirds",
        TileAction::TopLeftQuarter => "TopLeftQuarter",
        TileAction::TopRightQuarter => "TopRightQuarter",
        TileAction::BottomLeftQuarter => "BottomLeftQuarter",
        TileAction::BottomRightQuarter => "BottomRightQuarter",
        TileAction::Maximize => "Maximize",
        TileAction::Center => "Center",
        TileAction::Restore => "Restore",
        TileAction::EqualizeAll => "EqualizeAll",
        TileAction::ToggleZoom => "ToggleZoom",
        TileAction::MovePaneLeft => "MovePaneLeft",
        TileAction::MovePaneRight => "MovePaneRight",
        TileAction::MovePaneUp => "MovePaneUp",
        TileAction::MovePaneDown => "MovePaneDown",
        TileAction::SwapPaneLeft => "SwapPaneLeft",
        TileAction::SwapPaneRight => "SwapPaneRight",
        TileAction::SwapPaneUp => "SwapPaneUp",
        TileAction::SwapPaneDown => "SwapPaneDown",
        TileAction::StackNext => "StackNext",
        TileAction::StackPrev => "StackPrev",
        TileAction::SnapToNearest => "SnapToNearest",
    }
}

/// Convert a name back to a TileAction.
pub fn action_from_name(name: &str) -> Option<TileAction> {
    match name {
        "LeftHalf" => Some(TileAction::LeftHalf),
        "RightHalf" => Some(TileAction::RightHalf),
        "TopHalf" => Some(TileAction::TopHalf),
        "BottomHalf" => Some(TileAction::BottomHalf),
        "LeftThird" => Some(TileAction::LeftThird),
        "CenterThird" => Some(TileAction::CenterThird),
        "RightThird" => Some(TileAction::RightThird),
        "LeftTwoThirds" => Some(TileAction::LeftTwoThirds),
        "CenterTwoThirds" => Some(TileAction::CenterTwoThirds),
        "RightTwoThirds" => Some(TileAction::RightTwoThirds),
        "TopLeftQuarter" => Some(TileAction::TopLeftQuarter),
        "TopRightQuarter" => Some(TileAction::TopRightQuarter),
        "BottomLeftQuarter" => Some(TileAction::BottomLeftQuarter),
        "BottomRightQuarter" => Some(TileAction::BottomRightQuarter),
        "Maximize" => Some(TileAction::Maximize),
        "Center" => Some(TileAction::Center),
        "Restore" => Some(TileAction::Restore),
        "EqualizeAll" => Some(TileAction::EqualizeAll),
        "ToggleZoom" => Some(TileAction::ToggleZoom),
        "MovePaneLeft" => Some(TileAction::MovePaneLeft),
        "MovePaneRight" => Some(TileAction::MovePaneRight),
        "MovePaneUp" => Some(TileAction::MovePaneUp),
        "MovePaneDown" => Some(TileAction::MovePaneDown),
        "SwapPaneLeft" => Some(TileAction::SwapPaneLeft),
        "SwapPaneRight" => Some(TileAction::SwapPaneRight),
        "SwapPaneUp" => Some(TileAction::SwapPaneUp),
        "SwapPaneDown" => Some(TileAction::SwapPaneDown),
        "StackNext" => Some(TileAction::StackNext),
        "StackPrev" => Some(TileAction::StackPrev),
        "SnapToNearest" => Some(TileAction::SnapToNearest),
        _ => None,
    }
}

/// Human-readable display for a keybinding.
pub fn format_binding(binding: &KeyBinding) -> String {
    let mut parts = Vec::new();
    if binding.modifiers & CONTROL_KEY != 0 {
        parts.push("Ctrl");
    }
    if binding.modifiers & OPTION_KEY != 0 {
        parts.push("Opt");
    }
    if binding.modifiers & SHIFT_KEY != 0 {
        parts.push("Shift");
    }
    if binding.modifiers & CMD_KEY != 0 {
        parts.push("Cmd");
    }
    parts.push(keycode_name(binding.keycode));
    parts.join("+")
}

/// Human-readable display name for an action.
pub fn action_display_name(name: &str) -> String {
    // Insert spaces before capitals: "LeftHalf" → "Left Half"
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if i > 0 && ch.is_uppercase() {
            result.push(' ');
        }
        result.push(ch);
    }
    result
}

/// Group name for an action (for display sections).
pub fn action_group(name: &str) -> &'static str {
    if name.contains("Half") {
        "Halves"
    } else if name.contains("TwoThirds") {
        "Two-Thirds"
    } else if name.contains("Third") {
        "Thirds"
    } else if name.contains("Quarter") {
        "Quarters"
    } else if name.starts_with("Move") {
        "Move Focus"
    } else if name.starts_with("Swap") {
        "Swap Panes"
    } else {
        "Special"
    }
}

/// Keycode to display name.
fn keycode_name(keycode: u32) -> &'static str {
    use keycodes::*;
    match keycode {
        K_VK_A => "A", K_VK_B => "B", K_VK_C => "C", K_VK_D => "D",
        K_VK_E => "E", K_VK_F => "F", K_VK_G => "G", K_VK_H => "H",
        K_VK_I => "I", K_VK_J => "J", K_VK_K => "K", K_VK_L => "L",
        K_VK_M => "M", K_VK_N => "N", K_VK_O => "O", K_VK_P => "P",
        K_VK_Q => "Q", K_VK_R => "R", K_VK_S => "S", K_VK_T => "T",
        K_VK_U => "U", K_VK_V => "V", K_VK_W => "W", K_VK_X => "X",
        K_VK_Y => "Y", K_VK_Z => "Z",
        K_VK_0 => "0", K_VK_1 => "1", K_VK_2 => "2", K_VK_3 => "3",
        K_VK_4 => "4", K_VK_5 => "5", K_VK_6 => "6", K_VK_7 => "7",
        K_VK_8 => "8", K_VK_9 => "9",
        K_VK_RETURN => "Return", K_VK_TAB => "Tab", K_VK_SPACE => "Space",
        K_VK_DELETE => "Backspace", K_VK_ESCAPE => "Escape",
        K_VK_LEFT_ARROW => "\u{2190}", K_VK_RIGHT_ARROW => "\u{2192}",
        K_VK_UP_ARROW => "\u{2191}", K_VK_DOWN_ARROW => "\u{2193}",
        K_VK_EQUAL => "=", K_VK_MINUS => "-",
        _ => "?",
    }
}

/// Inline copy of keycodes (to avoid depending on tile_hotkeys from settings).
mod keycodes {
    pub const K_VK_A: u32 = 0x00;
    pub const K_VK_S: u32 = 0x01;
    pub const K_VK_D: u32 = 0x02;
    pub const K_VK_F: u32 = 0x03;
    pub const K_VK_H: u32 = 0x04;
    pub const K_VK_G: u32 = 0x05;
    pub const K_VK_Z: u32 = 0x06;
    pub const K_VK_X: u32 = 0x07;
    pub const K_VK_C: u32 = 0x08;
    pub const K_VK_V: u32 = 0x09;
    pub const K_VK_B: u32 = 0x0B;
    pub const K_VK_Q: u32 = 0x0C;
    pub const K_VK_W: u32 = 0x0D;
    pub const K_VK_E: u32 = 0x0E;
    pub const K_VK_R: u32 = 0x0F;
    pub const K_VK_Y: u32 = 0x10;
    pub const K_VK_T: u32 = 0x11;
    pub const K_VK_1: u32 = 0x12;
    pub const K_VK_2: u32 = 0x13;
    pub const K_VK_3: u32 = 0x14;
    pub const K_VK_4: u32 = 0x15;
    pub const K_VK_6: u32 = 0x16;
    pub const K_VK_5: u32 = 0x17;
    pub const K_VK_EQUAL: u32 = 0x18;
    pub const K_VK_9: u32 = 0x19;
    pub const K_VK_7: u32 = 0x1A;
    pub const K_VK_MINUS: u32 = 0x1B;
    pub const K_VK_8: u32 = 0x1C;
    pub const K_VK_0: u32 = 0x1D;
    pub const K_VK_O: u32 = 0x1F;
    pub const K_VK_U: u32 = 0x20;
    pub const K_VK_I: u32 = 0x22;
    pub const K_VK_P: u32 = 0x23;
    pub const K_VK_L: u32 = 0x25;
    pub const K_VK_J: u32 = 0x26;
    pub const K_VK_K: u32 = 0x28;
    pub const K_VK_N: u32 = 0x2D;
    pub const K_VK_M: u32 = 0x2E;
    pub const K_VK_RETURN: u32 = 0x24;
    pub const K_VK_TAB: u32 = 0x30;
    pub const K_VK_SPACE: u32 = 0x31;
    pub const K_VK_DELETE: u32 = 0x33;
    pub const K_VK_ESCAPE: u32 = 0x35;
    pub const K_VK_LEFT_ARROW: u32 = 0x7B;
    pub const K_VK_RIGHT_ARROW: u32 = 0x7C;
    pub const K_VK_DOWN_ARROW: u32 = 0x7D;
    pub const K_VK_UP_ARROW: u32 = 0x7E;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_has_all_bindings() {
        let config = TileConfig::default();
        assert_eq!(config.bindings.len(), 27);
    }

    #[test]
    fn test_roundtrip_serialize() {
        let config = TileConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: TileConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.bindings.len(), config.bindings.len());
    }

    #[test]
    fn test_to_bindings() {
        let config = TileConfig::default();
        let bindings = config.to_bindings();
        assert_eq!(bindings.len(), 27);
    }

    #[test]
    fn test_format_binding() {
        let b = KeyBinding {
            keycode: 0x7B, // left arrow
            modifiers: CONTROL_KEY | OPTION_KEY,
        };
        let s = format_binding(&b);
        assert!(s.contains("Ctrl"));
        assert!(s.contains("Opt"));
        assert!(s.contains("\u{2190}"));
    }

    #[test]
    fn test_action_display_name() {
        assert_eq!(action_display_name("LeftHalf"), "Left Half");
        assert_eq!(action_display_name("TopLeftQuarter"), "Top Left Quarter");
        assert_eq!(action_display_name("EqualizeAll"), "Equalize All");
    }

    #[test]
    fn test_action_group() {
        assert_eq!(action_group("LeftHalf"), "Halves");
        assert_eq!(action_group("LeftThird"), "Thirds");
        assert_eq!(action_group("LeftTwoThirds"), "Two-Thirds");
        assert_eq!(action_group("TopLeftQuarter"), "Quarters");
        assert_eq!(action_group("MovePaneLeft"), "Move Focus");
        assert_eq!(action_group("SwapPaneRight"), "Swap Panes");
        assert_eq!(action_group("Maximize"), "Special");
    }

    #[test]
    fn test_action_name_roundtrip() {
        let actions = [
            TileAction::LeftHalf, TileAction::RightHalf,
            TileAction::TopLeftQuarter, TileAction::Maximize,
            TileAction::MovePaneLeft, TileAction::SwapPaneRight,
        ];
        for action in actions {
            let name = action_name(action);
            let back = action_from_name(name).unwrap();
            assert_eq!(back, action);
        }
    }
}
