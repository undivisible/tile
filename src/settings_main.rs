//! Standalone settings window for Tile.
//!
//! Launched from the Tile menu bar as a separate process so that GPUI
//! can own the run loop without conflicting with the main tile app.

use gpui::Application;
use tile_settings::{open_panel_window, TilePanel};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let panel = match std::env::args().nth(1).as_deref() {
        Some("about") => TilePanel::About,
        _ => TilePanel::Settings,
    };

    Application::new().run(move |cx| {
        open_panel_window(cx, panel);
    });
}
