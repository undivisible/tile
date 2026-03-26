//! Drag-to-snap monitor.
//!
//! Watches for mouse drag events and shows overlay zones when a window
//! is being dragged near screen edges or managed panes.
//!
//! When Option+Control is held during a drag:
//! - Dragging near the edge of another window → snap beside it (match height)
//! - Dragging onto the center of another window → stack as a tab

use crate::app::{lock_state, AppState};
use log::info;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags};
use objc2_foundation::MainThreadMarker;
use std::sync::{Arc, Mutex};
use tile_core::layout::{detect_snap_zone, snap_zone_rect};
use tile_core::{Rect, SnapSide, TileTree};

/// Monitors mouse events to detect window dragging and show snap zones.
pub struct DragMonitor {
    _mouse_moved_monitor: Option<Retained<AnyObject>>,
    _mouse_up_monitor: Option<Retained<AnyObject>>,
}

/// The result of an Opt+Ctrl drag detection (public so AppState can store it).
#[derive(Debug, Clone)]
pub enum PendingModDrag {
    /// Snap the dragged window beside the target window.
    SnapBeside {
        target_frame: Rect,
        side: SnapSide,
    },
    /// Stack the dragged window onto the target (same frame, as a tab).
    StackOnto {
        target_frame: Rect,
    },
}

/// Internal alias
type ModDragTarget = PendingModDrag;

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
                if flags.contains(opt_ctrl) {
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
                if flags.contains(opt_ctrl) {
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

    let mut st = lock_state(&state);
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
    let mut st = lock_state(&state);
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
            ModDragTarget::SnapBeside { target_frame, side } => {
                // Show a preview of where the window would snap
                // Use a reasonable default width (half of target width)
                let preview_width = target_frame.width;
                let snap_frame = TileTree::snap_window_beside(
                    *target_frame,
                    Rect::new(0.0, 0.0, preview_width, target_frame.height),
                    *side,
                    screen,
                );
                snap_frame
            }
            ModDragTarget::StackOnto { target_frame } => {
                // Show inset overlay on the target to indicate stacking
                target_frame.inset(20.0)
            }
        };

        let mut st = lock_state(&state);
        st.overlay.show(overlay_rect, mtm);
        st.pending_mod_drag = Some(target);
    } else {
        let mut st = lock_state(&state);
        st.overlay.hide();
        st.pending_mod_drag = None;
    }
}

/// On mouse-up with Opt+Ctrl held, apply the snap-beside or stack-onto action.
fn handle_mod_drag_end(state: &Arc<Mutex<AppState>>, _x: f64, _y: f64) {
    let mut st = lock_state(&state);
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
            ModDragTarget::SnapBeside { target_frame, side } => {
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
            ModDragTarget::StackOnto { target_frame } => {
                // Move the dragged window to exactly match the target frame
                tile_ax::set_window_frame_raw(raw_element, target_frame);
                info!("Stacked {} onto target window", app_info.name);
            }
        }

        // Release the owned window element from get_frontmost_window
        tile_ax::release_frontmost_window(raw_element);
    }
}

/// Find a target window under the cursor for Opt+Ctrl drag.
/// Returns SnapBeside if cursor is at the edge, StackOnto if at center.
fn find_mod_drag_target(
    cursor_x: f64,
    cursor_y: f64,
    windows: &[tile_core::WindowInfo],
) -> Option<ModDragTarget> {
    // Get the frontmost window's PID so we can exclude it
    let frontmost = tile_ax::get_frontmost_app().map(|a| a.pid);

    for win in windows {
        // Skip the window being dragged (same app, frontmost)
        if Some(win.pid) == frontmost {
            continue;
        }
        if win.is_minimized {
            continue;
        }

        let frame = win.frame;
        if frame.contains_point(cursor_x, cursor_y) {
            // Determine if cursor is at center or edge
            let rx = (cursor_x - frame.x) / frame.width;
            let ry = (cursor_y - frame.y) / frame.height;

            // Center region → stack
            if rx > 0.25 && rx < 0.75 && ry > 0.25 && ry < 0.75 {
                return Some(ModDragTarget::StackOnto {
                    target_frame: frame,
                });
            }

            // Left edge → snap to left of target
            if rx < 0.25 {
                return Some(ModDragTarget::SnapBeside {
                    target_frame: frame,
                    side: SnapSide::Left,
                });
            }
            // Right edge → snap to right of target
            if rx > 0.75 {
                return Some(ModDragTarget::SnapBeside {
                    target_frame: frame,
                    side: SnapSide::Right,
                });
            }

            // Top/bottom edges also snap beside (default to right)
            return Some(ModDragTarget::SnapBeside {
                target_frame: frame,
                side: SnapSide::Right,
            });
        }
    }

    // Check proximity: if no window is directly under cursor, look for nearby ones
    let proximity_threshold = 80.0;
    let mut closest: Option<(f64, &tile_core::WindowInfo, SnapSide)> = None;

    for win in windows {
        if Some(win.pid) == frontmost || win.is_minimized {
            continue;
        }
        let frame = win.frame;

        // Distance from cursor to edges
        let dist_left = (cursor_x - frame.x).abs();
        let dist_right = (cursor_x - (frame.x + frame.width)).abs();

        // Must be vertically aligned (within frame height range)
        if cursor_y < frame.y - proximity_threshold
            || cursor_y > frame.y + frame.height + proximity_threshold
        {
            continue;
        }

        let (dist, side) = if dist_left < dist_right {
            (dist_left, SnapSide::Left)
        } else {
            (dist_right, SnapSide::Right)
        };

        if dist < proximity_threshold {
            if closest.is_none() || dist < closest.unwrap().0 {
                closest = Some((dist, win, side));
            }
        }
    }

    closest.map(|(_, win, side)| ModDragTarget::SnapBeside {
        target_frame: win.frame,
        side,
    })
}
