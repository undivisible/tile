//! Settings UI and configuration for Tile window manager.
//!
//! Provides a GPUI-based settings window for keybind customization
//! and a JSON-backed configuration system.

pub mod config;
pub mod window;

pub use config::TileConfig;
pub use window::open_settings_window;
