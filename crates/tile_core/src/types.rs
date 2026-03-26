use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

/// Unique identifier for a tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl NodeId {
    pub fn next() -> Self {
        Self(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Unique identifier for a managed window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

impl WindowId {
    pub fn next() -> Self {
        Self(NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// An opaque reference to an AX window element.
/// Stores the pid + window index for re-acquisition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AXWindowRef {
    pub pid: i32,
    /// Index of the window within the app's window list when first captured.
    pub window_index: usize,
    /// The raw AXUIElementRef pointer value, not serialized (reconstructed at runtime).
    #[serde(skip)]
    pub raw: usize,
}

impl AXWindowRef {
    pub fn new(pid: i32, window_index: usize, raw: usize) -> Self {
        Self {
            pid,
            window_index,
            raw,
        }
    }
}

/// A simple rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn contains_point(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    /// Inset the rect by the given amount on all sides.
    pub fn inset(&self, amount: f64) -> Self {
        Self {
            x: self.x + amount,
            y: self.y + amount,
            width: (self.width - 2.0 * amount).max(0.0),
            height: (self.height - 2.0 * amount).max(0.0),
        }
    }
}

/// Split orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    Horizontal, // side by side (left | right)
    Vertical,   // stacked (top / bottom)
}

impl Orientation {
    pub fn toggle(&self) -> Self {
        match self {
            Orientation::Horizontal => Orientation::Vertical,
            Orientation::Vertical => Orientation::Horizontal,
        }
    }
}

/// Cardinal direction for navigation and swapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// A managed window stored in a pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedWindow {
    pub id: WindowId,
    pub ax_ref: AXWindowRef,
    pub pid: i32,
    pub title: String,
    pub app_name: String,
    pub original_frame: Rect,
}

impl ManagedWindow {
    pub fn new(
        ax_ref: AXWindowRef,
        pid: i32,
        title: String,
        app_name: String,
        original_frame: Rect,
    ) -> Self {
        Self {
            id: WindowId::next(),
            ax_ref,
            pid,
            title,
            app_name,
            original_frame,
        }
    }
}

/// Info about a visible window (not necessarily managed).
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub pid: i32,
    pub title: String,
    pub app_name: String,
    pub frame: Rect,
    pub ax_ref: AXWindowRef,
    pub is_minimized: bool,
}

/// Info about a running application.
#[derive(Debug, Clone)]
pub struct AppInfo {
    pub pid: i32,
    pub name: String,
    pub bundle_id: Option<String>,
}

/// A tiling action that can be triggered by a hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileAction {
    // Halves
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,

    // Thirds
    LeftThird,
    CenterThird,
    RightThird,

    // Two-thirds
    LeftTwoThirds,
    CenterTwoThirds,
    RightTwoThirds,

    // Quarters
    TopLeftQuarter,
    TopRightQuarter,
    BottomLeftQuarter,
    BottomRightQuarter,

    // Special
    Maximize,
    Center,
    Restore,
    EqualizeAll,
    ToggleZoom,

    // Tiling movement
    MovePaneLeft,
    MovePaneRight,
    MovePaneUp,
    MovePaneDown,

    // Swap
    SwapPaneLeft,
    SwapPaneRight,
    SwapPaneUp,
    SwapPaneDown,

    // Stacking / scroll-to-switch
    StackNext,
    StackPrev,

    // Snap beside nearest window (floating, not BSP)
    SnapToNearest,
}

impl TileAction {
    /// Compute the target frame for a simple region action on the given screen.
    pub fn compute_frame(&self, screen: Rect) -> Option<Rect> {
        let x = screen.x;
        let y = screen.y;
        let w = screen.width;
        let h = screen.height;

        match self {
            // Halves
            TileAction::LeftHalf => Some(Rect::new(x, y, w / 2.0, h)),
            TileAction::RightHalf => Some(Rect::new(x + w / 2.0, y, w / 2.0, h)),
            TileAction::TopHalf => Some(Rect::new(x, y, w, h / 2.0)),
            TileAction::BottomHalf => Some(Rect::new(x, y + h / 2.0, w, h / 2.0)),

            // Thirds
            TileAction::LeftThird => Some(Rect::new(x, y, w / 3.0, h)),
            TileAction::CenterThird => Some(Rect::new(x + w / 3.0, y, w / 3.0, h)),
            TileAction::RightThird => Some(Rect::new(x + 2.0 * w / 3.0, y, w / 3.0, h)),

            // Two-thirds
            TileAction::LeftTwoThirds => Some(Rect::new(x, y, 2.0 * w / 3.0, h)),
            TileAction::CenterTwoThirds => Some(Rect::new(x + w / 6.0, y, 2.0 * w / 3.0, h)),
            TileAction::RightTwoThirds => Some(Rect::new(x + w / 3.0, y, 2.0 * w / 3.0, h)),

            // Quarters
            TileAction::TopLeftQuarter => Some(Rect::new(x, y, w / 2.0, h / 2.0)),
            TileAction::TopRightQuarter => Some(Rect::new(x + w / 2.0, y, w / 2.0, h / 2.0)),
            TileAction::BottomLeftQuarter => Some(Rect::new(x, y + h / 2.0, w / 2.0, h / 2.0)),
            TileAction::BottomRightQuarter => {
                Some(Rect::new(x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0))
            }

            // Maximize
            TileAction::Maximize => Some(screen),

            // Center — 60% of screen, centered
            TileAction::Center => {
                let cw = w * 0.6;
                let ch = h * 0.6;
                Some(Rect::new(x + (w - cw) / 2.0, y + (h - ch) / 2.0, cw, ch))
            }

            // These don't produce a simple frame
            _ => None,
        }
    }

    /// Returns the cycle group for size cycling (same shortcut pressed rapidly).
    /// Returns None if this action doesn't participate in cycling.
    pub fn cycle_group(&self) -> Option<&[TileAction]> {
        use TileAction::*;
        match self {
            LeftHalf => Some(&[LeftHalf, LeftTwoThirds, LeftThird]),
            RightHalf => Some(&[RightHalf, RightTwoThirds, RightThird]),
            TopHalf => Some(&[TopHalf, TopHalf, TopHalf]), // no cycling for top/bottom
            BottomHalf => Some(&[BottomHalf, BottomHalf, BottomHalf]),
            _ => None,
        }
    }
}

/// Which side to snap a window beside another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapSide {
    Left,
    Right,
}

/// Gap configuration for tiling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GapConfig {
    pub outer: f64,
    pub inner: f64,
}

impl Default for GapConfig {
    fn default() -> Self {
        Self {
            outer: 8.0,
            inner: 8.0,
        }
    }
}
