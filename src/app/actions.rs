use std::sync::{Arc, Mutex};
use std::time::Instant;

use log::info;
use tile_core::{Direction, Node, Rect, SnapSide, TileAction, TileTree};

use super::state::{lock_state, AppState};
use super::window_search::find_nearest_window;

/// Handle a tile action from hotkey press.
pub(crate) fn handle_action(state: &Arc<Mutex<AppState>>, action: TileAction) {
    let window_info = match tile_ax::get_frontmost_window() {
        Some(info) => info,
        None => {
            log::warn!("No frontmost window to tile");
            return;
        }
    };

    let (raw_element, _ax_ref, app_info) = window_info;

    // Use an inner function so we always release raw_element regardless of
    // which code path we take.
    handle_action_inner(state, action, raw_element, app_info);

    // raw_element is an owned CFTypeRef from CopyAttributeValue — release it.
    tile_ax::release_frontmost_window(raw_element);
}

/// Inner implementation of handle_action; the caller releases `raw_element`.
fn handle_action_inner(
    state: &Arc<Mutex<AppState>>,
    action: TileAction,
    raw_element: *const std::ffi::c_void,
    app_info: tile_core::AppInfo,
) {
    let current_frame = match tile_ax::get_window_frame_raw(raw_element) {
        Some(f) => f,
        None => {
            log::warn!("Could not get window frame");
            return;
        }
    };

    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => {
            log::warn!("Could not get screen frame");
            return;
        }
    };

    let mut st = lock_state(state);

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
