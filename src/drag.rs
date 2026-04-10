//! Drag-to-snap monitor.
//!
//! Three drag modes:
//!
//! 1. **Plain drag** — no overlay, no interference. If the cursor starts on a BSP
//!    split divider, enters split-resize mode and resizes both panes together.
//!
//! 2. **Opt+Ctrl drag** — shows a snap-zone overlay and, on mouse-up, snaps the
//!    window beside another or into a screen drop zone. Both windows are resized.
//!
//! 3. **Opt+Cmd drag** — legacy shared-resize path for the active split.

mod mod_target;

use crate::app::{lock_state, AppState};
use crate::app::TilingMode;
use crate::app::PendingSplitResize;
use log::info;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags};
use objc2_foundation::MainThreadMarker;
use std::sync::{Arc, Mutex};
use tile_core::layout::{detect_snap_zone, snap_zone_rect};
use tile_core::{Rect, TileTree};

pub(crate) use mod_target::PendingModDrag;
use mod_target::find_mod_drag_target;

/// Pixels from a split divider centre that counts as a hit.
const SPLIT_HIT_THRESHOLD: f64 = 8.0;

/// Monitors mouse events to detect window dragging and show snap zones.
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

        // ── mouse-down: detect split-border grabs ──────────────────────────
        let down_monitor = {
            let mask = NSEventMask::LeftMouseDown;
            let handler = block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
                let event = unsafe { event.as_ref() };
                let loc = event.locationInWindow();
                handle_mouse_down(&state_down, loc.x, loc.y);
            });
            NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &handler)
        };

        // ── drag: split resize (plain) or snap preview (Opt+Ctrl) ──────────
        let drag_monitor = {
            let mask = NSEventMask::LeftMouseDragged;
            let handler = block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
                let event = unsafe { event.as_ref() };
                let loc = event.locationInWindow();
                let flags = event.modifierFlags();

                let opt_ctrl = NSEventModifierFlags::Option.union(NSEventModifierFlags::Control);
                let opt_cmd  = NSEventModifierFlags::Option.union(NSEventModifierFlags::Command);

                if flags.contains(opt_cmd) {
                    handle_legacy_resize_drag(&state_drag, loc.x, loc.y);
                } else if flags.contains(opt_ctrl) {
                    handle_snap_drag(&state_drag, loc.x, loc.y, mtm);
                } else {
                    handle_split_drag(&state_drag, loc.x, loc.y);
                }
            });
            NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &handler)
        };

        // ── mouse-up: apply snap or clear resize ───────────────────────────
        let up_monitor = {
            let mask = NSEventMask::LeftMouseUp;
            let handler = block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
                let event = unsafe { event.as_ref() };
                let flags = event.modifierFlags();
                let opt_ctrl = NSEventModifierFlags::Option.union(NSEventModifierFlags::Control);
                let opt_cmd  = NSEventModifierFlags::Option.union(NSEventModifierFlags::Command);

                if flags.contains(opt_cmd) {
                    handle_legacy_resize_end(&state_up);
                } else if flags.contains(opt_ctrl) {
                    handle_snap_end(&state_up, event.locationInWindow().x, event.locationInWindow().y);
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

// ── mouse-down: check for split-border grab ────────────────────────────────

fn handle_mouse_down(state: &Arc<Mutex<AppState>>, x: f64, y: f64) {
    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };

    let mut st = lock_state(state);
    if st.tiling_mode != TilingMode::Bsp {
        return;
    }

    let screen_height = screen.y + screen.height;
    let ax_y = screen_height - y; // AppKit → AX coordinate

    let lines = st.tree.split_lines(screen);
    for line in lines {
        let hit = if line.is_horizontal {
            // Vertical divider — cursor must be near x = line.position and within y span
            (x - line.position).abs() < SPLIT_HIT_THRESHOLD
                && ax_y >= line.span_start
                && ax_y <= line.span_end
        } else {
            // Horizontal divider — cursor must be near y = line.position and within x span
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
    // Cursor not on any divider
    st.pending_split_resize = None;
}

// ── plain drag: split-border resize ───────────────────────────────────────

fn handle_split_drag(state: &Arc<Mutex<AppState>>, x: f64, y: f64) {
    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };

    let mut st = lock_state(state);
    let pending = match st.pending_split_resize {
        Some(p) => p,
        None => return,
    };

    let (px, py) = pending.last_cursor;
    let delta = if pending.is_horizontal {
        // Horizontal split (side-by-side): delta from horizontal mouse movement
        (x - px) / screen.width
    } else {
        // Vertical split (top/bottom): note AppKit y is inverted vs AX y
        // Dragging down in AppKit = decreasing y = increasing AX y = growing top pane
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
    st.overlay.hide();
}

// ── Opt+Ctrl drag: snap-zone preview ──────────────────────────────────────

fn handle_snap_drag(state: &Arc<Mutex<AppState>>, x: f64, y: f64, mtm: MainThreadMarker) {
    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };

    let screen_height = screen.y + screen.height;
    let ax_y = screen_height - y;

    let windows = tile_ax::list_visible_windows();
    let mut st = lock_state(state);

    // First try: snap beside / stack onto a specific window
    if let Some(target) = find_mod_drag_target(x, ax_y, &windows) {
        let overlay_rect = match &target {
            PendingModDrag::SnapBeside { target_frame, side } => {
                let preview_w = screen.width / 2.0;
                let preview_h = target_frame.height;
                TileTree::snap_window_beside(
                    *target_frame,
                    Rect::new(0.0, 0.0, preview_w, preview_h),
                    *side,
                    screen,
                )
            }
            PendingModDrag::StackOnto { target_frame } => target_frame.inset(20.0),
        };
        st.overlay.show(overlay_rect, mtm);
        st.pending_mod_drag = Some(target);
        return;
    }

    // Fall back: screen drop-zone overlay
    let pane_rects = st.tree.compute_layout(screen);
    if let Some(zone) = detect_snap_zone(x, ax_y, screen, &pane_rects) {
        let overlay_rect = snap_zone_rect(&zone, screen, &pane_rects);
        st.overlay.show(overlay_rect, mtm);
    } else {
        st.overlay.hide();
    }
    st.pending_mod_drag = None;
}

/// On mouse-up with Opt+Ctrl, apply snap and resize both windows.
fn handle_snap_end(state: &Arc<Mutex<AppState>>, _x: f64, _y: f64) {
    let mut st = lock_state(state);
    st.overlay.hide();

    let pending = st.pending_mod_drag.take();
    if let Some(target) = pending {
        let window_info = match tile_ax::get_frontmost_window() {
            Some(info) => info,
            None => return,
        };
        let (raw_element, _ax_ref, app_info) = window_info;

        match target {
            PendingModDrag::SnapBeside { target_frame, side } => {
                let screen = match tile_ax::get_usable_screen_frame(0) {
                    Some(s) => s,
                    None => {
                        tile_ax::release_frontmost_window(raw_element);
                        return;
                    }
                };

                // Both windows get equal halves of the screen area occupied by the target.
                // The "area" is the full available width beside the target.
                let half_w = screen.width / 2.0;
                let (source_frame, target_new_frame) = match side {
                    tile_core::SnapSide::Left => {
                        let src = Rect::new(screen.x, target_frame.y, half_w, target_frame.height);
                        let tgt = Rect::new(screen.x + half_w, target_frame.y, half_w, target_frame.height);
                        (src, tgt)
                    }
                    tile_core::SnapSide::Right => {
                        let tgt = Rect::new(screen.x, target_frame.y, half_w, target_frame.height);
                        let src = Rect::new(screen.x + half_w, target_frame.y, half_w, target_frame.height);
                        (src, tgt)
                    }
                };

                // Resize the dragged window
                tile_ax::set_window_frame_raw(raw_element, source_frame);

                // Find and resize the target window by matching its current frame
                let all_windows = tile_ax::list_visible_windows();
                for win in &all_windows {
                    if frames_approx_equal(win.frame, target_frame) && win.pid != app_info.pid {
                        tile_ax::set_window_frame(&win.ax_ref, target_new_frame);
                        break;
                    }
                }

                info!("Snapped {} beside target on {:?}", app_info.name, side);
            }
            PendingModDrag::StackOnto { target_frame } => {
                tile_ax::set_window_frame_raw(raw_element, target_frame);
                info!("Stacked {} onto target window", app_info.name);
            }
        }

        tile_ax::release_frontmost_window(raw_element);
    }
}

fn frames_approx_equal(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < 4.0
        && (a.y - b.y).abs() < 4.0
        && (a.width - b.width).abs() < 4.0
        && (a.height - b.height).abs() < 4.0
}

// ── BSP layout application ─────────────────────────────────────────────────

pub(crate) fn apply_bsp_layout(layout: &[(tile_core::NodeId, Rect)], tree: &tile_core::TileTree) {
    for (pane_id, rect) in layout {
        if let Some(tile_core::Node::Pane { tabs, active, .. }) = tree.root.find(*pane_id) {
            if let Some(window) = tabs.get(*active) {
                tile_ax::set_window_frame(&window.ax_ref, *rect);
            }
        }
    }
}

// ── Legacy Opt+Cmd shared-resize (kept for compatibility) ─────────────────

fn handle_legacy_resize_drag(state: &Arc<Mutex<AppState>>, x: f64, y: f64) {
    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };
    let mut st = lock_state(state);
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
        let dx = (x - px) as f32;
        let dy = (y - py) as f32;
        let delta = (dx + dy) * 0.0015;
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
}
