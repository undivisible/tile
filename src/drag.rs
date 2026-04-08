//! Drag-to-snap monitor.
//!
//! Watches for mouse drag events and shows overlay zones when a window
//! is being dragged near screen edges or managed panes.
//!
//! When Option+Control is held during a drag:
//! - Dragging near the edge of another window → snap beside it (match height)
//! - Dragging onto the center of another window → stack as a tab

mod mod_target;

use crate::app::{lock_state, AppState};
use crate::app::state::TilingMode;
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

/// Monitors mouse events to detect window dragging and show snap zones.
pub struct DragMonitor {
    _mouse_moved_monitor: Option<Retained<AnyObject>>,
    _mouse_up_monitor: Option<Retained<AnyObject>>,
}

impl DragMonitor {
    pub fn new(mtm: MainThreadMarker, state: Arc<Mutex<AppState>>) -> Self {
        let state_drag = state.clone();
        let state_up = state.clone();

        let drag_monitor = {
            let mask = NSEventMask::LeftMouseDragged;
            let handler = block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
                let event = unsafe { event.as_ref() };
                let location = event.locationInWindow();
                let flags = event.modifierFlags();

                let opt_ctrl =
                    NSEventModifierFlags::Option.union(NSEventModifierFlags::Control);
                let opt_cmd =
                    NSEventModifierFlags::Option.union(NSEventModifierFlags::Command);
                if flags.contains(opt_cmd) {
                    handle_shared_resize_drag(&state_drag, location.x, location.y);
                } else if flags.contains(opt_ctrl) {
                    handle_mod_drag_move(&state_drag, location.x, location.y, mtm);
                } else {
                    handle_drag_move(&state_drag, location.x, location.y, mtm);
                }
            });
            NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &handler)
        };

        let up_monitor = {
            let mask = NSEventMask::LeftMouseUp;
            let handler = block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
                let event = unsafe { event.as_ref() };
                let flags = event.modifierFlags();
                let opt_ctrl =
                    NSEventModifierFlags::Option.union(NSEventModifierFlags::Control);
                let opt_cmd =
                    NSEventModifierFlags::Option.union(NSEventModifierFlags::Command);
                if flags.contains(opt_cmd) {
                    handle_shared_resize_end(&state_up);
                } else if flags.contains(opt_ctrl) {
                    handle_mod_drag_end(&state_up, event.locationInWindow().x, event.locationInWindow().y);
                } else {
                    handle_drag_end(&state_up);
                }
            });
            NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &handler)
        };

        Self {
            _mouse_moved_monitor: drag_monitor,
            _mouse_up_monitor: up_monitor,
        }
    }
}

fn handle_drag_move(state: &Arc<Mutex<AppState>>, x: f64, y: f64, mtm: MainThreadMarker) {
    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };

    let mut st = lock_state(state);
    let pane_rects = st.tree.compute_layout(screen);
    let screen_height = screen.y + screen.height;
    let flipped_y = screen_height - y;

    if let Some(zone) = detect_snap_zone(x, flipped_y, screen, &pane_rects) {
        let overlay_rect = snap_zone_rect(&zone, screen, &pane_rects);
        st.overlay.show(overlay_rect, mtm);
    } else {
        st.overlay.hide();
    }
}

fn handle_drag_end(state: &Arc<Mutex<AppState>>) {
    let mut st = lock_state(state);
    st.overlay.hide();
}

/// When Opt+Ctrl is held during a drag, detect nearby windows and show
/// snap-beside or stack-onto overlay.
fn handle_mod_drag_move(
    state: &Arc<Mutex<AppState>>,
    x: f64,
    y: f64,
    mtm: MainThreadMarker,
) {
    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };

    let screen_height = screen.y + screen.height;
    // Convert AppKit bottom-left y to top-left y (AX coordinate space)
    let ax_y = screen_height - y;

    // Find nearby windows
    let windows = tile_ax::list_visible_windows();
    if let Some(target) = find_mod_drag_target(x, ax_y, &windows) {
        let overlay_rect = match &target {
            PendingModDrag::SnapBeside { target_frame, side } => {
                // Show a preview of where the window would snap
                // Use a reasonable default width (half of target width)
                let preview_width = target_frame.width;
                TileTree::snap_window_beside(
                    *target_frame,
                    Rect::new(0.0, 0.0, preview_width, target_frame.height),
                    *side,
                    screen,
                )
            }
            PendingModDrag::StackOnto { target_frame } => {
                // Show inset overlay on the target to indicate stacking
                target_frame.inset(20.0)
            }
        };

        let mut st = lock_state(state);
        st.overlay.show(overlay_rect, mtm);
        st.pending_mod_drag = Some(target);
    } else {
        let mut st = lock_state(state);
        st.overlay.hide();
        st.pending_mod_drag = None;
    }
}

/// On mouse-up with Opt+Ctrl held, apply the snap-beside or stack-onto action.
fn handle_mod_drag_end(state: &Arc<Mutex<AppState>>, _x: f64, _y: f64) {
    let mut st = lock_state(state);
    st.overlay.hide();

    let pending = st.pending_mod_drag.take();
    if let Some(target) = pending {
        // Get the frontmost window (the one being dragged)
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
                let current_frame = match tile_ax::get_window_frame_raw(raw_element) {
                    Some(f) => f,
                    None => {
                        tile_ax::release_frontmost_window(raw_element);
                        return;
                    }
                };
                let snap_frame = TileTree::snap_window_beside(
                    target_frame,
                    current_frame,
                    side,
                    screen,
                );
                tile_ax::set_window_frame_raw(raw_element, snap_frame);
                info!(
                    "Snapped {} beside target on {:?}",
                    app_info.name, side
                );
            }
            PendingModDrag::StackOnto { target_frame } => {
                // Move the dragged window to exactly match the target frame
                tile_ax::set_window_frame_raw(raw_element, target_frame);
                info!("Stacked {} onto target window", app_info.name);
            }
        }

        // Release the owned window element from get_frontmost_window
        tile_ax::release_frontmost_window(raw_element);
    }
}

fn handle_shared_resize_drag(state: &Arc<Mutex<AppState>>, x: f64, y: f64) {
    let screen = match tile_ax::get_usable_screen_frame(0) {
        Some(s) => s,
        None => return,
    };
    let mut st = lock_state(state);
    if st.tiling_mode != TilingMode::Multiplexer {
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
            for (pane_id, rect) in layout {
                if let Some(tile_core::Node::Pane { tabs, active, .. }) = st.tree.root.find(pane_id) {
                    if let Some(window) = tabs.get(*active) {
                        tile_ax::set_window_frame(&window.ax_ref, rect);
                    }
                }
            }
        }
    }
}

fn handle_shared_resize_end(state: &Arc<Mutex<AppState>>) {
    let mut st = lock_state(state);
    st.multiplexer.shared_resize.last_cursor = None;
    st.multiplexer.shared_resize.split_id = None;
}
