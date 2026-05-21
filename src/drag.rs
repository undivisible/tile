//! Drag-to-tile monitor.
//!
//! **Plain drag** (no modifier)
//!   If the cursor starts within 8 px of a BSP split divider, enters split-resize
//!   mode and moves both panes simultaneously. Otherwise passes through silently.
//!
//! **Ctrl+Cmd drag** — VSCode-style drop zones
//!   Shows a blue overlay while dragging:
//!   - Over an existing window: L/R/T/B quadrant drop zones just like VSCode editor groups.
//!   - Near a screen edge: full-edge snap zones (left half, right half, etc.).
//!   On drop:
//!   - Pane quadrant zone (BSP mode): splits that pane in the tree, adds the window,
//!     and relayouts the entire grid so every window is sized to its slot.
//!   - Pane quadrant zone (Snap mode): resizes the dragged window to the slot rect.
//!   - Screen edge zone: resizes the dragged window to the edge rect.
//!   - Beside a window (proximity snap): resizes both windows to equal halves.
//!
//! **Opt+Cmd drag** — legacy single-split resize.

mod mod_target;

use crate::app::TilingMode;
use crate::app::{lock_state, AppState, PendingSplitResize};
use log::info;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags};
use objc2_foundation::MainThreadMarker;
use std::sync::{Arc, Mutex};
use tile_core::layout::{detect_snap_zone, snap_zone_rect, SnapZone};
use tile_core::{AppInfo, ManagedWindow, Node, Orientation, Rect, SnapSide};

use mod_target::find_mod_drag_target;
pub(crate) use mod_target::PendingModDrag;

/// Pixels from a split divider centre that counts as a hit.
const SPLIT_HIT_THRESHOLD: f64 = 8.0;

/// Monitors mouse events for drag-to-tile behaviour.
pub struct DragMonitor {
    _mouse_down_monitor: Option<Retained<AnyObject>>,
    _mouse_moved_monitor: Option<Retained<AnyObject>>,
    _mouse_up_monitor: Option<Retained<AnyObject>>,
}

impl DragMonitor {
    pub fn new(mtm: MainThreadMarker, state: Arc<Mutex<AppState>>) -> Self {
        let state_down = state.clone();
        let state_drag = state.clone();
        let state_up = state.clone();

        let down_monitor = {
            let mask = NSEventMask::LeftMouseDown;
            let handler = block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
                let loc = unsafe { event.as_ref() }.locationInWindow();
                handle_mouse_down(&state_down, loc.x, loc.y);
            });
            NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &handler)
        };

        let drag_monitor = {
            let mask = NSEventMask::LeftMouseDragged;
            let handler = block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
                let event = unsafe { event.as_ref() };
                let loc = event.locationInWindow();
                let flags = event.modifierFlags();
                let ctrl_cmd = NSEventModifierFlags::Control.union(NSEventModifierFlags::Command);
                let opt_cmd = NSEventModifierFlags::Option.union(NSEventModifierFlags::Command);
                if flags.contains(opt_cmd) {
                    handle_legacy_resize_drag(&state_drag, loc.x, loc.y);
                } else if flags.contains(ctrl_cmd) {
                    handle_snap_drag(&state_drag, loc.x, loc.y, mtm);
                } else {
                    handle_split_drag(&state_drag, loc.x, loc.y);
                }
            });
            NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &handler)
        };

        let up_monitor = {
            let mask = NSEventMask::LeftMouseUp;
            let handler = block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
                let event = unsafe { event.as_ref() };
                let flags = event.modifierFlags();
                let ctrl_cmd = NSEventModifierFlags::Control.union(NSEventModifierFlags::Command);
                let opt_cmd = NSEventModifierFlags::Option.union(NSEventModifierFlags::Command);
                if flags.contains(opt_cmd) {
                    handle_legacy_resize_end(&state_up);
                } else if flags.contains(ctrl_cmd) {
                    let loc = event.locationInWindow();
                    handle_snap_end(&state_up, loc.x, loc.y);
                } else {
                    handle_drag_end(&state_up);
                }
            });
            NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &handler)
        };

        Self {
            _mouse_down_monitor: down_monitor,
            _mouse_moved_monitor: drag_monitor,
            _mouse_up_monitor: up_monitor,
        }
    }
}

// ── Mouse-down: check for split-border grab ───────────────────────────────

fn handle_mouse_down(state: &Arc<Mutex<AppState>>, x: f64, y: f64) {
    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };
    let mut st = lock_state(state);
    if st.paused {
        return;
    }
    st.pending_dragged_window_raw = tile_ax::get_frontmost_window().map(|(raw_element, _, _)| {
        let raw = raw_element as usize;
        tile_ax::release_frontmost_window(raw_element);
        raw
    });
    if st.tiling_mode != TilingMode::Bsp {
        return;
    }
    let ax_y = screen.y + screen.height - y;
    for line in st.tree.split_lines(screen) {
        let hit = if line.is_horizontal {
            (x - line.position).abs() < SPLIT_HIT_THRESHOLD
                && ax_y >= line.span_start
                && ax_y <= line.span_end
        } else {
            (ax_y - line.position).abs() < SPLIT_HIT_THRESHOLD
                && x >= line.span_start
                && x <= line.span_end
        };
        if hit {
            st.pending_split_resize = Some(PendingSplitResize {
                split_id: line.split_id,
                is_horizontal: line.is_horizontal,
                last_cursor: (x, y),
            });
            return;
        }
    }
    st.pending_split_resize = None;
}

// ── Plain drag: split-border resize only ─────────────────────────────────

fn handle_split_drag(state: &Arc<Mutex<AppState>>, x: f64, y: f64) {
    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };
    let mut st = lock_state(state);
    if st.paused {
        return;
    }
    let pending = match st.pending_split_resize {
        Some(p) => p,
        None => return,
    };
    let (px, py) = pending.last_cursor;
    let delta = if pending.is_horizontal {
        (x - px) / screen.width
    } else {
        (py - y) / screen.height
    } as f32;

    if st.tree.root.resize_split(pending.split_id, delta) {
        let layout = st.tree.compute_layout(screen);
        apply_bsp_layout(&layout, &st.tree);
    }
    if let Some(ref mut p) = st.pending_split_resize {
        p.last_cursor = (x, y);
    }
}

fn handle_drag_end(state: &Arc<Mutex<AppState>>) {
    let mut st = lock_state(state);
    st.pending_split_resize = None;
    st.pending_dragged_window_raw = None;
    st.overlay.hide();
}

// ── Ctrl+Cmd drag: VSCode-style drop zone preview ────────────────────────

fn handle_snap_drag(state: &Arc<Mutex<AppState>>, x: f64, y: f64, mtm: MainThreadMarker) {
    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };
    let ax_y = screen.y + screen.height - y;
    let windows = tile_ax::list_visible_windows();
    let mut st = lock_state(state);
    if st.paused {
        return;
    }
    if st.pending_dragged_window_raw.is_none() {
        st.pending_dragged_window_raw = match tile_ax::get_frontmost_window() {
            Some((raw_element, _, _)) => {
                let raw = raw_element as usize;
                tile_ax::release_frontmost_window(raw_element);
                Some(raw)
            }
            None => None,
        };
    }
    let dragged_window_raw = st.pending_dragged_window_raw;

    // Priority 1: window-proximity snap (snap-beside)
    if let Some(target) = find_mod_drag_target(x, ax_y, &windows, dragged_window_raw) {
        let overlay_rect = match &target {
            PendingModDrag::SnapBeside {
                target_frame, ..
            } => {
                let half_w = screen.width / 2.0;
                Rect::new(screen.x, target_frame.y, half_w, target_frame.height)
            }
            PendingModDrag::StackOnto { target_frame, .. } => target_frame.inset(20.0),
        };
        st.overlay.show(overlay_rect, mtm);
        st.pending_mod_drag = Some(target);
        st.pending_snap_zone = None;
        return;
    }
    st.pending_mod_drag = None;

    // Priority 2: pane-quadrant or screen-edge snap zone
    let pane_rects = st.tree.compute_layout(screen);
    if let Some(zone) = detect_snap_zone(x, ax_y, screen, &pane_rects) {
        let overlay_rect = snap_zone_rect(&zone, screen, &pane_rects);
        st.overlay.show(overlay_rect, mtm);
        st.pending_snap_zone = Some(zone);
    } else {
        st.overlay.hide();
        st.pending_snap_zone = None;
    }
}

// ── Ctrl+Cmd mouse-up: apply the drop ────────────────────────────────────

fn handle_snap_end(state: &Arc<Mutex<AppState>>, _x: f64, _y: f64) {
    // Grab pending state and hide overlay while holding the lock.
    let (pending_mod, pending_zone, tiling_mode) = {
        let mut st = lock_state(state);
        if st.paused {
            st.overlay.hide();
            st.pending_mod_drag = None;
            st.pending_snap_zone = None;
            st.pending_dragged_window_raw = None;
            return;
        }
        st.overlay.hide();
        (
            st.pending_mod_drag.take(),
            st.pending_snap_zone.take(),
            st.tiling_mode,
        )
    };

    // ── Path 1: proximity snap (snap-beside) ───────────────────────────────
    if let Some(target) = pending_mod {
        let source_raw = {
            let mut st = lock_state(state);
            st.pending_dragged_window_raw.take()
        };
        let source_raw = match source_raw {
            Some(raw) => raw,
            None => return,
        };
        let windows = tile_ax::list_visible_windows();
        let source_info = match windows.iter().find(|w| w.ax_ref.raw == source_raw) {
            Some(info) => info,
            None => return,
        };
        let app_info = AppInfo {
            pid: source_info.pid,
            name: source_info.app_name.clone(),
            bundle_id: None,
        };

        match target {
            PendingModDrag::SnapBeside {
                target_raw,
                target_frame,
                side,
            } => {
                let screen = match tile_ax::get_usable_screen_frame(0) {
                    Some(s) => s,
                    None => return,
                };
                let target_info = match windows.iter().find(|w| w.ax_ref.raw == target_raw) {
                    Some(info) => info,
                    None => return,
                };
                let target_pane = {
                    let mut st = lock_state(state);
                    if let Some(pane_id) = st.tree.root.find_pane_by_raw_window(target_raw) {
                        Some(pane_id)
                    } else {
                        let managed = ManagedWindow::new(
                            target_info.ax_ref.clone(),
                            target_info.pid,
                            target_info.app_name.clone(),
                            target_info.app_name.clone(),
                            target_info.frame,
                        );
                        Some(st.tree.add_window(managed))
                    }
                };

                if let Some(pane_id) = target_pane {
                    let orientation = if target_frame.width >= target_frame.height {
                        Orientation::Horizontal
                    } else {
                        Orientation::Vertical
                    };
                    let dragged_goes_second = matches!(side, SnapSide::Right);
                    let managed = ManagedWindow::new(
                        source_info.ax_ref.clone(),
                        source_info.pid,
                        source_info.app_name.clone(),
                        source_info.app_name.clone(),
                        source_info.frame,
                    );

                    let layout = {
                        let mut st = lock_state(state);
                        if let Some(window_id) =
                            st.tree.root.find_window_id_by_raw_window(source_raw)
                        {
                            st.tree.remove_window(window_id);
                        }
                        if let Some((first_id, second_id)) =
                            st.tree.root.split_pane(pane_id, orientation, 0.5)
                        {
                            let target_new_pane = if dragged_goes_second {
                                second_id
                            } else {
                                st.tree.swap_panes(first_id, second_id);
                                first_id
                            };
                            st.tree.root.stack_window(target_new_pane, managed);
                            st.tree.focused_pane = Some(target_new_pane);
                        }
                        st.tree.compute_layout(screen)
                    };

                    let st = lock_state(state);
                    apply_bsp_layout(&layout, &st.tree);
                    info!("Snapped {} beside target on {:?}", app_info.name, side);
                }
            }
            PendingModDrag::StackOnto {
                target_raw,
                target_frame,
            } => {
                let screen = match tile_ax::get_usable_screen_frame(0) {
                    Some(s) => s,
                    None => return,
                };
                let target_info = match windows.iter().find(|w| w.ax_ref.raw == target_raw) {
                    Some(info) => info,
                    None => return,
                };
                let mut st = lock_state(state);
                if let Some(window_id) = st.tree.root.find_window_id_by_raw_window(source_raw) {
                    st.tree.remove_window(window_id);
                }
                let pane_id = if let Some(pane_id) = st.tree.root.find_pane_by_raw_window(target_raw) {
                    pane_id
                } else {
                    let managed = ManagedWindow::new(
                        target_info.ax_ref.clone(),
                        target_info.pid,
                        target_info.app_name.clone(),
                        target_info.app_name.clone(),
                        target_info.frame,
                    );
                    st.tree.add_window(managed)
                };
                let managed = ManagedWindow::new(
                    source_info.ax_ref.clone(),
                    source_info.pid,
                    source_info.app_name.clone(),
                    source_info.app_name.clone(),
                    source_info.frame,
                );
                st.tree.root.stack_window(pane_id, managed);
                st.tree.focused_pane = Some(pane_id);
                let layout = st.tree.compute_layout(screen);
                drop(st);
                let st = lock_state(state);
                apply_bsp_layout(&layout, &st.tree);
                info!(
                    "Stacked {} into pane {:?} at ({:.0}, {:.0}, {:.0}, {:.0})",
                    app_info.name,
                    pane_id,
                    target_frame.x,
                    target_frame.y,
                    target_frame.width,
                    target_frame.height
                );
            }
        }
        return;
    }

    // ── Path 2: snap-zone drop ────────────────────────────────────────────
    if let Some(zone) = pending_zone {
        apply_snap_zone_drop(state, zone, tiling_mode);
    }
    let mut st = lock_state(state);
    st.pending_dragged_window_raw = None;
}

/// Apply a snap-zone drop. Pane zones mutate the tree; edge zones still resize
/// the floating window slot.
fn apply_snap_zone_drop(state: &Arc<Mutex<AppState>>, zone: SnapZone, _tiling_mode: TilingMode) {
    let source_raw = {
        let mut st = lock_state(state);
        if st.paused {
            return;
        }
        st.pending_dragged_window_raw.take()
    };
    let source_raw = match source_raw {
        Some(raw) => raw,
        None => return,
    };
    let windows = tile_ax::list_visible_windows();
    let source_info = match windows.iter().find(|w| w.ax_ref.raw == source_raw) {
        Some(info) => info,
        None => return,
    };
    let app_info = AppInfo {
        pid: source_info.pid,
        name: source_info.app_name.clone(),
        bundle_id: None,
    };

    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };

    match zone {
        SnapZone::SplitLeft(pane_id)
        | SnapZone::SplitRight(pane_id)
        | SnapZone::SplitTop(pane_id)
        | SnapZone::SplitBottom(pane_id) => {
            let orientation = match zone {
                SnapZone::SplitLeft(_) | SnapZone::SplitRight(_) => Orientation::Horizontal,
                _ => Orientation::Vertical,
            };
            // Dragged window goes into the SECOND slot for Right/Bottom,
            // FIRST slot for Left/Top (requires swapping after split).
            let dragged_goes_second =
                matches!(zone, SnapZone::SplitRight(_) | SnapZone::SplitBottom(_));

            let managed = ManagedWindow::new(
                source_info.ax_ref.clone(),
                app_info.pid,
                app_info.name.clone(),
                app_info.name.clone(),
                source_info.frame,
            );

            let layout = {
                let mut st = lock_state(state);

                // Remove the exact dragged window from the tree if it's already managed.
                if let Some(window_id) = st.tree.root.find_window_id_by_raw_window(source_raw) {
                    st.tree.remove_window(window_id);
                }

                if let Some((first_id, second_id)) =
                    st.tree.root.split_pane(pane_id, orientation, 0.5)
                {
                    let target_pane = if dragged_goes_second {
                        second_id
                    } else {
                        // Swap so the first slot is free for the dragged window.
                        st.tree.swap_panes(first_id, second_id);
                        first_id
                    };
                    st.tree.root.stack_window(target_pane, managed);
                    st.tree.focused_pane = Some(target_pane);
                }

                st.tree.compute_layout(screen)
            };

            let st = lock_state(state);
            apply_bsp_layout(&layout, &st.tree);
            drop(st);

            info!("Tree drop: {} into {:?} pane {:?}", app_info.name, zone, pane_id);
        }

        // ── Stack onto an existing pane ────────────────────────────────────
        SnapZone::Stack(pane_id) => {
            let mut st = lock_state(state);
            if let Some(window_id) = st.tree.root.find_window_id_by_raw_window(source_raw) {
                st.tree.remove_window(window_id);
            }
            let managed = ManagedWindow::new(
                source_info.ax_ref.clone(),
                app_info.pid,
                app_info.name.clone(),
                app_info.name.clone(),
                source_info.frame,
            );
            if st.tree.root.stack_window(pane_id, managed) {
                st.tree.focused_pane = Some(pane_id);
                let pane_rects = st.tree.compute_layout(screen);
                let frame = snap_zone_rect(&zone, screen, &pane_rects);
                drop(st);
                tile_ax::set_window_frame(&source_info.ax_ref, frame);
                info!("Stack drop: {} onto pane {:?}", app_info.name, pane_id);
            }
        }

        // ── Screen-edge zones: resize window to the edge slot ─────────────
        _ => {
            let st = lock_state(state);
            let pane_rects = st.tree.compute_layout(screen);
            drop(st);
            let frame = snap_zone_rect(&zone, screen, &pane_rects);
            tile_ax::set_window_frame(&source_info.ax_ref, frame);
            info!("Edge drop: {} → {:?}", app_info.name, zone);
        }
    }
}

// ── BSP layout application ────────────────────────────────────────────────

pub(crate) fn apply_bsp_layout(layout: &[(tile_core::NodeId, Rect)], tree: &tile_core::TileTree) {
    for (pane_id, rect) in layout {
        if let Some(Node::Pane { tabs, active, .. }) = tree.root.find(*pane_id) {
            if let Some(window) = tabs.get(*active) {
                tile_ax::set_window_frame(&window.ax_ref, *rect);
            }
        }
    }
}

// ── Legacy Opt+Cmd single-split resize ───────────────────────────────────

fn handle_legacy_resize_drag(state: &Arc<Mutex<AppState>>, x: f64, y: f64) {
    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };
    let mut st = lock_state(state);
    if st.paused {
        return;
    }
    if st.tiling_mode != TilingMode::Bsp {
        return;
    }
    let split_id = match st.tree.root.first_split_id() {
        Some(id) => id,
        None => return,
    };
    let prev = st.multiplexer.shared_resize.last_cursor;
    st.multiplexer.shared_resize.split_id = Some(split_id);
    st.multiplexer.shared_resize.last_cursor = Some((x, y));
    if let Some((px, py)) = prev {
        let delta = ((x - px) as f32 + (y - py) as f32) * 0.0015;
        if st.tree.root.resize_split(split_id, delta) {
            let layout = st.tree.compute_layout(screen);
            apply_bsp_layout(&layout, &st.tree);
        }
    }
}

fn handle_legacy_resize_end(state: &Arc<Mutex<AppState>>) {
    let mut st = lock_state(state);
    st.multiplexer.shared_resize.last_cursor = None;
    st.multiplexer.shared_resize.split_id = None;
    st.pending_dragged_window_raw = None;
}
