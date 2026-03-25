//! Tile — A macOS tiling window manager with stacking, Rectangle keybinds,
//! and drag-to-snap zones.
//!
//! Runs as a menu bar app (LSUIElement = YES) with global hotkeys for
//! window management.

mod app;
mod drag;

use app::TileApp;
use log::{error, info};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("Tile window manager starting");

    // Check accessibility permissions
    if !tile_ax::check_accessibility_permission() {
        info!("Requesting accessibility permissions...");
        tile_ax::request_accessibility_permission();
        eprintln!(
            "Tile needs Accessibility permissions to manage windows.\n\
             Please grant access in System Settings > Privacy & Security > Accessibility,\n\
             then restart Tile."
        );
        // Continue anyway — the permission dialog is shown and some operations
        // may work after the user grants access without restarting.
    }

    // Run the app
    let app = TileApp::new();
    match app {
        Ok(app) => app.run(),
        Err(e) => {
            error!("Failed to start Tile: {}", e);
            std::process::exit(1);
        }
    }
}
