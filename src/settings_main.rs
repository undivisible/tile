//! Standalone settings window for Tile.
//!
//! Launched from the Tile menu bar as a separate process so that GPUI
//! can own the run loop without conflicting with the main tile app.

use gpui::Application;
use tile_settings::open_settings_window;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    Application::new().run(|cx| {
        open_settings_window(cx);
    });
}
