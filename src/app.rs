//! Main application state and event loop.

mod actions;
mod menu;
mod observe;
mod state;
mod window_search;

use crate::app::actions::handle_action;
use crate::app::observe::observe_running_apps;
pub(crate) use state::{lock_state, AppState, PendingSplitResize};
pub(crate) use state::TilingMode;
use log::{debug, error, info};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::MainThreadMarker;
use std::sync::{Arc, Mutex};
use tile_ax::WindowObserverManager;
use tile_hotkeys::{HotkeyManager, ScrollMonitor};

use crate::drag::DragMonitor;

/// The main Tile application.
pub struct TileApp {
    mtm: MainThreadMarker,
}

impl TileApp {
    pub fn new() -> Result<Self, String> {
        let mtm =
            MainThreadMarker::new().ok_or_else(|| "Must be called from the main thread".to_string())?;
        Ok(Self { mtm })
    }

    pub fn run(self) {
        let mtm = self.mtm;

        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

        // Create shared state
        let state = Arc::new(Mutex::new(AppState::new()));

        // Set up status bar
        menu::setup_status_bar(mtm);

        // Load config and apply tiling mode
        let config = tile_settings::TileConfig::load();
        {
            let mut st = crate::app::state::lock_state(&state);
            st.tiling_mode = match config.tiling_mode {
                tile_settings::TilingModeConfig::Bsp => TilingMode::Bsp,
                tile_settings::TilingModeConfig::Snap => TilingMode::Snap,
            };
            st.tree.gaps.outer = config.gap_outer;
            st.tree.gaps.inner = config.gap_inner;
        }
        let bindings = config.to_bindings();
        let state_for_hotkeys = state.clone();
        match HotkeyManager::with_bindings(
            Box::new(move |action| {
                handle_action(&state_for_hotkeys, action);
            }),
            bindings,
        ) {
            Ok(manager) => {
                info!("Hotkey manager initialized");
                std::mem::forget(manager);
            }
            Err(e) => {
                error!("Failed to initialize hotkey manager: {}", e);
            }
        }

        // Set up scroll monitor for Opt+Ctrl+Scroll stack cycling
        let state_for_scroll = state.clone();
        let _scroll_monitor = ScrollMonitor::new(Box::new(move |action| {
            handle_action(&state_for_scroll, action);
        }));

        // Set up drag monitor
        let _drag_monitor = DragMonitor::new(mtm, state.clone());

        // Set up window observer
        {
            let state_for_observer = state.clone();
            let observer = WindowObserverManager::new(Box::new(move |event| {
                debug!("Window event: {:?}", event);
                let mut st = lock_state(&state_for_observer);
                st.needs_relayout = true;
            }));
            let mut st = lock_state(&state);
            st.observer = Some(observer);
        }

        // Observe existing apps
        observe_running_apps(&state);

        info!("Tile is running. Press Ctrl+Opt+Arrow keys to tile windows.");

        app.run();
    }
}
