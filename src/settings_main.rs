//! Standalone Tile settings entry point.

use gpui::{App, Application};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    Application::new().run(|cx: &mut App| {
        tile_settings::open_settings_window(cx);
    });
}
