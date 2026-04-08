use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use log::warn;
use tile_ax::WindowObserverManager;
use tile_core::{Rect, TileAction, TileTree};
use tile_overlay::{OverlayConfig, OverlayManager};

use crate::drag::PendingModDrag;

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
    pub tiling_mode: TilingMode,
    pub multiplexer: MultiplexerState,
    pub action_history: Vec<ActionSnapshot>,
}

// SAFETY: AppState is only accessed from the main thread (via Mutex).
// The non-Send types (OverlayManager contains NSWindow, WindowObserverManager
// contains CFTypeRef) are all created and used on the main thread.
unsafe impl Send for AppState {}

impl AppState {
    pub(crate) fn new() -> Self {
        Self {
            tree: TileTree::new(),
            overlay: OverlayManager::new(OverlayConfig::default()),
            observer: None,
            last_action: None,
            cycle_index: 0,
            original_frames: Vec::new(),
            needs_relayout: false,
            pending_mod_drag: None,
            tiling_mode: TilingMode::Snap,
            multiplexer: MultiplexerState::default(),
            action_history: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilingMode {
    Snap,
    Multiplexer,
}

#[derive(Debug, Clone, Copy)]
pub struct MultiplexerRegion {
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SharedResizeState {
    pub split_id: Option<tile_core::NodeId>,
    pub last_cursor: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Default)]
pub struct MultiplexerState {
    pub active_region: Option<MultiplexerRegion>,
    pub shared_resize: SharedResizeState,
}

#[derive(Debug, Clone, Copy)]
pub struct ActionSnapshot {
    pub pid: i32,
    pub frame: Rect,
}
