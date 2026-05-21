use std::sync::{Arc, Mutex};

use log::info;
use tile_core::{Node, Rect};

use super::actions::relayout;
use super::state::{lock_state, AppState};

/// Toggle the global tiling pause flag.
pub(crate) fn toggle_pause(state: &Arc<Mutex<AppState>>) -> bool {
    let mut st = lock_state(state);
    st.paused = !st.paused;
    info!("Tiling paused: {}", st.paused);
    st.paused
}

/// Toggle monocle-style zoom on the focused pane.
pub(crate) fn toggle_monocle(state: &Arc<Mutex<AppState>>, screen: Rect) {
    let mut st = lock_state(state);
    if st.paused {
        return;
    }
    if let Some(pane_id) = st.tree.focused_pane {
        if st.tree.root.toggle_zoom(pane_id) {
            relayout(&st.tree, screen);
        }
    }
}

/// Rotate the layout tree so it reflows in the opposite orientation.
pub(crate) fn flip_layout(state: &Arc<Mutex<AppState>>, screen: Rect) {
    let mut st = lock_state(state);
    if st.paused {
        return;
    }
    st.tree.root.rotate_tree();
    relayout(&st.tree, screen);
}

/// Split the focused tabbed pane and put the active tab into its own pane.
pub(crate) fn unstack_focused(state: &Arc<Mutex<AppState>>, screen: Rect) {
    let mut st = lock_state(state);
    if st.paused {
        return;
    }
    let Some(pane_id) = st.tree.focused_pane else {
        return;
    };
    let window_id = match st.tree.root.find(pane_id) {
        Some(Node::Pane { tabs, active, .. }) => tabs.get(*active).map(|w| w.id),
        _ => None,
    };
    let Some(window_id) = window_id else {
        return;
    };
    if let Some(new_pane) = st.tree.unstack_window(pane_id, window_id) {
        st.tree.focused_pane = Some(new_pane);
        relayout(&st.tree, screen);
    }
}

/// Split the focused tabbed pane until every window has its own pane.
pub(crate) fn unstack_all_focused(state: &Arc<Mutex<AppState>>, screen: Rect) {
    let mut st = lock_state(state);
    if st.paused {
        return;
    }
    let Some(pane_id) = st.tree.focused_pane else {
        return;
    };
    if let Some(new_panes) = st.tree.unstack_all_windows(pane_id) {
        st.tree.focused_pane = new_panes.last().copied();
        relayout(&st.tree, screen);
    }
}
