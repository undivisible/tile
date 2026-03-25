//! Drag-to-snap monitor.
//!
//! Watches for mouse drag events and shows overlay zones when a window
//! is being dragged near screen edges or managed panes.

use crate::app::AppState;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask};
use objc2_foundation::MainThreadMarker;
use std::sync::{Arc, Mutex};
use tile_core::layout::{detect_snap_zone, snap_zone_rect};

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
                handle_drag_move(&state_drag, location.x, location.y, mtm);
            });
            NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &handler)
        };

        let up_monitor = {
            let mask = NSEventMask::LeftMouseUp;
            let handler = block2::RcBlock::new(move |_event: std::ptr::NonNull<NSEvent>| {
                handle_drag_end(&state_up);
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

    let mut st = state.lock().unwrap();
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
    let mut st = state.lock().unwrap();
    st.overlay.hide();
}
