//! Main application state and event loop.

mod actions;
mod features;
mod menu;
mod observe;
mod state;
mod window_search;

use crate::app::actions::handle_action;
use crate::app::observe::observe_running_apps;
use log::{debug, error, info};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::MainThreadMarker;
pub(crate) use state::TilingMode;
pub(crate) use state::{lock_state, AppState, PendingSplitResize};
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
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| "Must be called from the main thread".to_string())?;
        Ok(Self { mtm })
    }

    pub fn run(self) {
        let mtm = self.mtm;

        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

        // Create shared state
        let state = Arc::new(Mutex::new(AppState::new()));

        // Load config and apply tiling mode.
        let config = tile_settings::TileConfig::load();
        {
            let mut st = crate::app::state::lock_state(&state);
            st.config = config.clone();
            st.tiling_mode = match config.tiling_mode {
                tile_settings::TilingModeConfig::Bsp => TilingMode::Bsp,
                tile_settings::TilingModeConfig::Snap => TilingMode::Snap,
            };
            st.tree.gaps.outer = config.gap_outer;
            st.tree.gaps.inner = config.gap_inner;
        }

        // Set up status bar after config is loaded so menu callbacks can mutate it.
        menu::setup_status_bar(mtm, state.clone());
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
                match event {
                    tile_ax::WindowEvent::Destroyed { raw, .. } => {
                        let region = {
                            let st = lock_state(&state_for_observer);
                            st.multiplexer.active_region.map(|r| r.rect)
                        };

                        let maybe_tree = {
                            let mut st = lock_state(&state_for_observer);
                            if let Some(window_id) = st.tree.root.find_window_id_by_raw_window(raw) {
                                st.tree.remove_window(window_id);
                                Some(st.tree.clone())
                            } else {
                                None
                            }
                        };

                        if let Some(tree) = maybe_tree {
                            if lock_state(&state_for_observer).paused {
                                return;
                            }
                            let screen = region.unwrap_or_else(|| {
                                tile_ax::get_usable_screen_frame(0)
                                    .unwrap_or(tile_core::Rect::new(0.0, 0.0, 1920.0, 1080.0))
                            });
                            crate::app::actions::relayout(&tree, screen);
                        }
                    }
                    _ => {
                        let mut st = lock_state(&state_for_observer);
                        if !st.paused {
                            st.needs_relayout = true;
                        }
                    }
                }
            }));
            let mut st = lock_state(&state);
            st.observer = Some(observer);
        }

        // Observe existing apps
        observe_running_apps(&state);

        // If we launch in BSP mode, immediately tile everything that is already open.
        if matches!(config.tiling_mode, tile_settings::TilingModeConfig::Bsp)
            && !lock_state(&state).paused
        {
            let region = {
                let st = crate::app::state::lock_state(&state);
                st.multiplexer.active_region.map(|r| r.rect)
            };
            crate::app::actions::sync_bsp_layout(&state, region, None);
        }

        info!("Tile is running. Press Ctrl+Opt+Arrow keys to tile windows.");

        app.run();
    }
}
