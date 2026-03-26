//! Main application state and event loop.

use log::{debug, error, info, warn};
use objc2::sel;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuItem, NSStatusBar,
};
use objc2_foundation::{ns_string, MainThreadMarker, NSString};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;
use tile_ax::WindowObserverManager;
use tile_core::{Direction, Node, Rect, SnapSide, TileAction, TileTree};
use tile_hotkeys::{HotkeyManager, ScrollMonitor};
use tile_overlay::{OverlayConfig, OverlayManager};

use crate::drag::{DragMonitor, PendingModDrag};

/// Lock the AppState mutex, recovering from poison if necessary.
pub(crate) fn lock_state(state: &Mutex<AppState>) -> MutexGuard<'_, AppState> {
    match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!("State mutex poisoned, recovering");
            poisoned.into_inner()
        }
    }
}

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
        setup_status_bar(mtm, state.clone());

        // Set up hotkeys (callback runs on main thread via Carbon)
        let state_for_hotkeys = state.clone();
        match HotkeyManager::new(Box::new(move |action| {
            handle_action(&state_for_hotkeys, action);
        })) {
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

/// Shared application state.
pub struct AppState {
    pub tree: TileTree,
    pub overlay: OverlayManager,
    pub observer: Option<WindowObserverManager>,
    pub last_action: Option<(TileAction, Instant)>,
    pub cycle_index: usize,
    pub original_frames: Vec<(i32, Rect)>,
    pub needs_relayout: bool,
    /// Pending Opt+Ctrl drag target (snap-beside or stack-onto).
    pub pending_mod_drag: Option<PendingModDrag>,
}

// SAFETY: AppState is only accessed from the main thread (via Mutex).
// The non-Send types (OverlayManager contains NSWindow, WindowObserverManager
// contains CFTypeRef) are all created and used on the main thread.
unsafe impl Send for AppState {}

impl AppState {
    fn new() -> Self {
        Self {
            tree: TileTree::new(),
            overlay: OverlayManager::new(OverlayConfig::default()),
            observer: None,
            last_action: None,
            cycle_index: 0,
            original_frames: Vec::new(),
            needs_relayout: false,
            pending_mod_drag: None,
        }
    }
}

/// Handle a tile action from hotkey press.
fn handle_action(state: &Arc<Mutex<AppState>>, action: TileAction) {
    debug!("Handling action: {:?}", action);

    let window_info = match tile_ax::get_frontmost_window() {
        Some(info) => info,
        None => {
            warn!("No frontmost window to tile");
            return;
        }
    };

    let (raw_element, _ax_ref, app_info) = window_info;

    let current_frame = match tile_ax::get_window_frame_raw(raw_element) {
        Some(f) => f,
        None => {
            warn!("Could not get window frame");
            return;
        }
    };

    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => {
            warn!("Could not get screen frame");
            return;
        }
    };

    let mut st = lock_state(&state);

    // Handle special actions
    match action {
        TileAction::Restore => {
            if let Some(pos) = st
                .original_frames
                .iter()
                .position(|(pid, _)| *pid == app_info.pid)
            {
                let (_, frame) = st.original_frames.remove(pos);
                tile_ax::set_window_frame_raw(raw_element, frame);
                info!("Restored window for {}", app_info.name);
            }
            return;
        }
        TileAction::EqualizeAll => {
            st.tree.root.equalize_all();
            relayout(&st.tree, screen);
            return;
        }
        TileAction::ToggleZoom => {
            if let Some(pane_id) = st.tree.focused_pane {
                st.tree.root.toggle_zoom(pane_id);
                relayout(&st.tree, screen);
            }
            return;
        }
        TileAction::MovePaneLeft
        | TileAction::MovePaneRight
        | TileAction::MovePaneUp
        | TileAction::MovePaneDown => {
            let dir = match action {
                TileAction::MovePaneLeft => Direction::Left,
                TileAction::MovePaneRight => Direction::Right,
                TileAction::MovePaneUp => Direction::Up,
                TileAction::MovePaneDown => Direction::Down,
                _ => unreachable!(),
            };
            st.tree.navigate_focus(dir, screen);
            return;
        }
        TileAction::SwapPaneLeft
        | TileAction::SwapPaneRight
        | TileAction::SwapPaneUp
        | TileAction::SwapPaneDown => {
            let dir = match action {
                TileAction::SwapPaneLeft => Direction::Left,
                TileAction::SwapPaneRight => Direction::Right,
                TileAction::SwapPaneUp => Direction::Up,
                TileAction::SwapPaneDown => Direction::Down,
                _ => unreachable!(),
            };
            let current_pane = st.tree.focused_pane;
            st.tree.navigate_focus(dir, screen);
            let target_pane = st.tree.focused_pane;
            if let (Some(a), Some(b)) = (current_pane, target_pane) {
                if a != b {
                    st.tree.swap_panes(a, b);
                    relayout(&st.tree, screen);
                }
            }
            return;
        }
        TileAction::StackNext | TileAction::StackPrev => {
            let forward = matches!(action, TileAction::StackNext);
            if let Some(pane_id) = st.tree.focused_pane {
                if let Some(new_active) = st.tree.cycle_tab(pane_id, forward) {
                    // Raise the newly active tab's window and push others behind
                    if let Some(Node::Pane { tabs, .. }) = st.tree.root.find(pane_id) {
                        // Focus the active tab
                        if let Some(active_win) = tabs.get(new_active) {
                            tile_ax::focus_window(&active_win.ax_ref);
                            info!(
                                "Cycled to tab {} in pane: {} ({})",
                                new_active, active_win.title, active_win.app_name
                            );
                        }
                    }
                }
            }
            return;
        }
        TileAction::SnapToNearest => {
            // Find the nearest visible window and snap beside it
            let windows = tile_ax::list_visible_windows();
            if let Some(nearest) = find_nearest_window(current_frame, &windows, app_info.pid) {
                let side = if current_frame.center().0 < nearest.frame.center().0 {
                    SnapSide::Left
                } else {
                    SnapSide::Right
                };
                let snap_frame = TileTree::snap_window_beside(
                    nearest.frame,
                    current_frame,
                    side,
                    screen,
                );
                tile_ax::set_window_frame_raw(raw_element, snap_frame);
                info!(
                    "Snapped {} beside {} on {:?}",
                    app_info.name, nearest.app_name, side
                );
            }
            return;
        }
        _ => {}
    }

    // Size cycling
    let now = Instant::now();
    let target_action = if let Some(cycle_group) = action.cycle_group() {
        if let Some((last_act, last_time)) = st.last_action {
            if last_act == action && now.duration_since(last_time).as_millis() < 1000 {
                st.cycle_index = (st.cycle_index + 1) % cycle_group.len();
            } else {
                st.cycle_index = 0;
            }
        } else {
            st.cycle_index = 0;
        }
        cycle_group[st.cycle_index]
    } else {
        st.cycle_index = 0;
        action
    };

    st.last_action = Some((action, now));

    // Save original frame for restore
    if !st
        .original_frames
        .iter()
        .any(|(pid, _)| *pid == app_info.pid)
    {
        st.original_frames.push((app_info.pid, current_frame));
    }

    // Compute and apply target frame
    if let Some(target_frame) = target_action.compute_frame(screen) {
        tile_ax::set_window_frame_raw(raw_element, target_frame);
        info!(
            "Tiled {} ({}) to {:?} -> ({:.0}, {:.0}, {:.0}, {:.0})",
            app_info.name,
            app_info.pid,
            target_action,
            target_frame.x,
            target_frame.y,
            target_frame.width,
            target_frame.height
        );
    }
}

/// Apply the tiling tree layout to all managed windows.
fn relayout(tree: &TileTree, screen: Rect) {
    let layout = tree.compute_layout(screen);
    for (pane_id, rect) in &layout {
        if let Some(Node::Pane { tabs, active, .. }) = tree.root.find(*pane_id) {
            if let Some(window) = tabs.get(*active) {
                tile_ax::set_window_frame(&window.ax_ref, *rect);
            }
        }
    }
}

/// Set up the status bar menu.
fn setup_status_bar(mtm: MainThreadMarker, _state: Arc<Mutex<AppState>>) {
    let status_bar = NSStatusBar::systemStatusBar();
    let item = status_bar.statusItemWithLength(-1.0); // NSVariableStatusItemLength

    if let Some(button) = item.button(mtm) {
        button.setTitle(ns_string!("\u{229e}")); // ⊞ symbol
    }

    let menu = NSMenu::new(mtm);

    // About item
    let about_title = NSString::from_str("About Tile");
    let about_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &about_title,
            None,
            ns_string!(""),
        )
    };
    menu.addItem(&about_item);

    // Separator
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Quit item
    let quit_title = NSString::from_str("Quit Tile");
    let quit_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &quit_title,
            Some(sel!(terminate:)),
            ns_string!("q"),
        )
    };
    menu.addItem(&quit_item);

    item.setMenu(Some(&menu));

    // Keep the status item alive
    std::mem::forget(item);
}

/// Find the nearest visible window to the given frame, excluding the specified PID.
fn find_nearest_window(
    from: Rect,
    windows: &[tile_core::WindowInfo],
    exclude_pid: i32,
) -> Option<tile_core::WindowInfo> {
    let (cx, cy) = from.center();
    windows
        .iter()
        .filter(|w| w.pid != exclude_pid && !w.is_minimized)
        .min_by(|a, b| {
            let (ax, ay) = a.frame.center();
            let (bx, by) = b.frame.center();
            let da = (ax - cx).powi(2) + (ay - cy).powi(2);
            let db = (bx - cx).powi(2) + (by - cy).powi(2);
            da.partial_cmp(&db).unwrap()
        })
        .cloned()
}

/// Start observing all running regular applications.
fn observe_running_apps(state: &Arc<Mutex<AppState>>) {
    let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
    let apps = workspace.runningApplications();

    let mut st = lock_state(&state);
    let observer = match st.observer.as_mut() {
        Some(o) => o,
        None => return,
    };

    for app in apps.iter() {
        if app.activationPolicy() == objc2_app_kit::NSApplicationActivationPolicy::Regular {
            let pid = app.processIdentifier();
            observer.observe_app(pid);
        }
    }
}
