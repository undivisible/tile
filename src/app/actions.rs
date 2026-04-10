use std::sync::{Arc, Mutex};
use std::time::Instant;

use log::info;
use tile_core::{AXWindowRef, Direction, ManagedWindow, Node, Rect, SnapSide, TileAction, TileTree};

use super::state::{lock_state, ActionSnapshot, AppState, MultiplexerRegion, TilingMode};
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
        TileAction::UndoLastAction => {
            if let Some(snapshot) = st.action_history.pop() {
                if snapshot.pid == app_info.pid {
                    tile_ax::set_window_frame_raw(raw_element, snapshot.frame);
                    info!("Undid last action for {}", app_info.name);
                }
            }
            return;
        }
        TileAction::ToggleMultiplexerMode => {
            st.tiling_mode = if st.tiling_mode == TilingMode::Snap {
                TilingMode::Bsp
            } else {
                TilingMode::Snap
            };
            info!("Tiling mode set to {:?}", st.tiling_mode);
            return;
        }
        TileAction::SetMultiplexerRegionFromFrontmost => {
            st.multiplexer.active_region = Some(MultiplexerRegion {
                rect: current_frame,
            });
            st.tiling_mode = TilingMode::Bsp;
            info!(
                "Multiplexer region set from frontmost window: ({:.0}, {:.0}, {:.0}, {:.0})",
                current_frame.x, current_frame.y, current_frame.width, current_frame.height
            );
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
        TileAction::MoveToNextDisplay | TileAction::MoveToPreviousDisplay => {
            let displays = tile_ax::all_usable_frames();
            if displays.len() < 2 {
                return;
            }
            let (cx, cy) = current_frame.center();
            let mut current_idx = 0usize;
            for (i, d) in displays.iter().enumerate() {
                if d.contains_point(cx, cy) {
                    current_idx = i;
                    break;
                }
            }
            let target_idx = if matches!(action, TileAction::MoveToNextDisplay) {
                (current_idx + 1) % displays.len()
            } else if current_idx == 0 {
                displays.len() - 1
            } else {
                current_idx - 1
            };
            let src = displays[current_idx];
            let dst = displays[target_idx];
            let rx = if src.width > 0.0 { (current_frame.x - src.x) / src.width } else { 0.0 };
            let ry = if src.height > 0.0 { (current_frame.y - src.y) / src.height } else { 0.0 };
            let rw = if src.width > 0.0 { current_frame.width / src.width } else { 0.5 };
            let rh = if src.height > 0.0 { current_frame.height / src.height } else { 0.5 };
            let mapped = Rect::new(
                dst.x + dst.width * rx,
                dst.y + dst.height * ry,
                dst.width * rw,
                dst.height * rh,
            );
            st.action_history.push(ActionSnapshot {
                pid: app_info.pid,
                frame: current_frame,
            });
            tile_ax::set_window_frame_raw(raw_element, mapped);
            info!("Moved {} to display {}", app_info.name, target_idx);
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

    // In BSP mode: add window to the tree if not already managed, then
    // relayout the whole grid (the hotkey action is ignored — the tree drives sizing).
    if st.tiling_mode == TilingMode::Bsp {
        if st.tree.root.find_pane_by_pid(app_info.pid).is_none() {
            let window = ManagedWindow::new(
                AXWindowRef::new(app_info.pid, 0, raw_element as usize),
                app_info.pid,
                app_info.name.clone(),
                app_info.name.clone(),
                current_frame,
            );
            st.tree.add_window(window);
        }
        let region = st.multiplexer.active_region.map(|r| r.rect).unwrap_or(screen);
        relayout(&st.tree, region);
        info!("BSP relayout triggered by hotkey for {}", app_info.name);
        return;
    }

    // Snap mode: compute frame from the requested action and apply it.
    let target_frame = match target_action.compute_frame(screen) {
        Some(f) => f,
        None => return,
    };
    st.action_history.push(ActionSnapshot {
        pid: app_info.pid,
        frame: current_frame,
    });
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

pub(crate) fn set_tiling_mode(state: &Arc<Mutex<AppState>>, mode: TilingMode) {
    let mut st = lock_state(state);
    st.tiling_mode = mode;
}

pub(crate) fn set_multiplexer_region(state: &Arc<Mutex<AppState>>, rect: Rect) {
    let mut st = lock_state(state);
    st.multiplexer.active_region = Some(MultiplexerRegion { rect });
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
