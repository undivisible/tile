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

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rect {
        Rect::new(0.0, 25.0, 1920.0, 1055.0)
    }

    // ---------------------------------------------------------------
    // Rect tests
    // ---------------------------------------------------------------

    #[test]
    fn test_rect_center() {
        let r = Rect::new(100.0, 200.0, 400.0, 300.0);
        let (cx, cy) = r.center();
        assert_eq!(cx, 300.0);
        assert_eq!(cy, 350.0);
    }

    #[test]
    fn test_rect_center_at_origin() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let (cx, cy) = r.center();
        assert_eq!(cx, 50.0);
        assert_eq!(cy, 50.0);
    }

    #[test]
    fn test_rect_center_zero_size() {
        let r = Rect::zero();
        let (cx, cy) = r.center();
        assert_eq!(cx, 0.0);
        assert_eq!(cy, 0.0);
    }

    #[test]
    fn test_rect_contains_point_inside() {
        let r = Rect::new(10.0, 20.0, 100.0, 200.0);
        assert!(r.contains_point(50.0, 100.0));
    }

    #[test]
    fn test_rect_contains_point_on_edges() {
        let r = Rect::new(10.0, 20.0, 100.0, 200.0);
        // Top-left corner
        assert!(r.contains_point(10.0, 20.0));
        // Bottom-right corner
        assert!(r.contains_point(110.0, 220.0));
        // Left edge
        assert!(r.contains_point(10.0, 100.0));
        // Right edge
        assert!(r.contains_point(110.0, 100.0));
        // Top edge
        assert!(r.contains_point(50.0, 20.0));
        // Bottom edge
        assert!(r.contains_point(50.0, 220.0));
    }

    #[test]
    fn test_rect_contains_point_outside() {
        let r = Rect::new(10.0, 20.0, 100.0, 200.0);
        assert!(!r.contains_point(9.0, 100.0));   // left of rect
        assert!(!r.contains_point(111.0, 100.0));  // right of rect
        assert!(!r.contains_point(50.0, 19.0));    // above rect
        assert!(!r.contains_point(50.0, 221.0));   // below rect
    }

    #[test]
    fn test_rect_contains_point_zero_rect() {
        let r = Rect::zero();
        assert!(r.contains_point(0.0, 0.0));
        assert!(!r.contains_point(0.1, 0.0));
    }

    #[test]
    fn test_rect_inset() {
        let r = Rect::new(10.0, 20.0, 200.0, 100.0);
        let inset = r.inset(5.0);
        assert_eq!(inset.x, 15.0);
        assert_eq!(inset.y, 25.0);
        assert_eq!(inset.width, 190.0);
        assert_eq!(inset.height, 90.0);
    }

    #[test]
    fn test_rect_inset_zero() {
        let r = Rect::new(10.0, 20.0, 200.0, 100.0);
        let inset = r.inset(0.0);
        assert_eq!(inset.x, 10.0);
        assert_eq!(inset.y, 20.0);
        assert_eq!(inset.width, 200.0);
        assert_eq!(inset.height, 100.0);
    }

    #[test]
    fn test_rect_inset_negative() {
        let r = Rect::new(10.0, 20.0, 200.0, 100.0);
        let inset = r.inset(-5.0);
        assert_eq!(inset.x, 5.0);
        assert_eq!(inset.y, 15.0);
        assert_eq!(inset.width, 210.0);
        assert_eq!(inset.height, 110.0);
    }

    #[test]
    fn test_rect_inset_larger_than_rect() {
        let r = Rect::new(10.0, 20.0, 20.0, 10.0);
        let inset = r.inset(15.0);
        // Width and height clamped to 0
        assert_eq!(inset.width, 0.0);
        assert_eq!(inset.height, 0.0);
    }

    #[test]
    fn test_rect_zero() {
        let r = Rect::zero();
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 0.0);
        assert_eq!(r.width, 0.0);
        assert_eq!(r.height, 0.0);
    }

    // ---------------------------------------------------------------
    // TileAction::compute_frame tests
    // ---------------------------------------------------------------

    #[test]
    fn test_compute_frame_left_half() {
        let s = screen();
        let r = TileAction::LeftHalf.compute_frame(s).unwrap();
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 25.0);
        assert_eq!(r.width, 960.0);
        assert_eq!(r.height, 1055.0);
    }

    #[test]
    fn test_compute_frame_right_half() {
        let s = screen();
        let r = TileAction::RightHalf.compute_frame(s).unwrap();
        assert_eq!(r.x, 960.0);
        assert_eq!(r.y, 25.0);
        assert_eq!(r.width, 960.0);
        assert_eq!(r.height, 1055.0);
    }

    #[test]
    fn test_compute_frame_top_half() {
        let s = screen();
        let r = TileAction::TopHalf.compute_frame(s).unwrap();
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 25.0);
        assert_eq!(r.width, 1920.0);
        assert_eq!(r.height, 527.5);
    }

    #[test]
    fn test_compute_frame_bottom_half() {
        let s = screen();
        let r = TileAction::BottomHalf.compute_frame(s).unwrap();
        assert_eq!(r.x, 0.0);
        assert!((r.y - 552.5).abs() < 0.1);
        assert_eq!(r.width, 1920.0);
        assert_eq!(r.height, 527.5);
    }

    #[test]
    fn test_compute_frame_thirds() {
        let s = screen();
        let third_w = 1920.0 / 3.0;

        let left = TileAction::LeftThird.compute_frame(s).unwrap();
        assert_eq!(left.x, 0.0);
        assert!((left.width - third_w).abs() < 0.01);

        let center = TileAction::CenterThird.compute_frame(s).unwrap();
        assert!((center.x - third_w).abs() < 0.01);
        assert!((center.width - third_w).abs() < 0.01);

        let right = TileAction::RightThird.compute_frame(s).unwrap();
        assert!((right.x - 2.0 * third_w).abs() < 0.01);
        assert!((right.width - third_w).abs() < 0.01);
    }

    #[test]
    fn test_compute_frame_two_thirds() {
        let s = screen();
        let two_third_w = 2.0 * 1920.0 / 3.0;

        let left = TileAction::LeftTwoThirds.compute_frame(s).unwrap();
        assert_eq!(left.x, 0.0);
        assert!((left.width - two_third_w).abs() < 0.01);

        let center = TileAction::CenterTwoThirds.compute_frame(s).unwrap();
        assert!((center.x - 1920.0 / 6.0).abs() < 0.01);
        assert!((center.width - two_third_w).abs() < 0.01);

        let right = TileAction::RightTwoThirds.compute_frame(s).unwrap();
        assert!((right.x - 1920.0 / 3.0).abs() < 0.01);
        assert!((right.width - two_third_w).abs() < 0.01);
    }

    #[test]
    fn test_compute_frame_quarters() {
        let s = screen();
        let hw = 960.0;
        let hh = 1055.0 / 2.0;

        let tl = TileAction::TopLeftQuarter.compute_frame(s).unwrap();
        assert_eq!(tl.x, 0.0);
        assert_eq!(tl.y, 25.0);
        assert_eq!(tl.width, hw);
        assert_eq!(tl.height, hh);

        let tr = TileAction::TopRightQuarter.compute_frame(s).unwrap();
        assert_eq!(tr.x, hw);
        assert_eq!(tr.y, 25.0);
        assert_eq!(tr.width, hw);
        assert_eq!(tr.height, hh);

        let bl = TileAction::BottomLeftQuarter.compute_frame(s).unwrap();
        assert_eq!(bl.x, 0.0);
        assert!((bl.y - (25.0 + hh)).abs() < 0.01);
        assert_eq!(bl.width, hw);
        assert_eq!(bl.height, hh);

        let br = TileAction::BottomRightQuarter.compute_frame(s).unwrap();
        assert_eq!(br.x, hw);
        assert!((br.y - (25.0 + hh)).abs() < 0.01);
        assert_eq!(br.width, hw);
        assert_eq!(br.height, hh);
    }

    #[test]
    fn test_compute_frame_maximize() {
        let s = screen();
        let r = TileAction::Maximize.compute_frame(s).unwrap();
        assert_eq!(r, s);
    }

    #[test]
    fn test_compute_frame_center() {
        let s = screen();
        let r = TileAction::Center.compute_frame(s).unwrap();
        let cw = 1920.0 * 0.6;
        let ch = 1055.0 * 0.6;
        assert!((r.width - cw).abs() < 0.01);
        assert!((r.height - ch).abs() < 0.01);
        // Should be centered
        assert!((r.x - (0.0 + (1920.0 - cw) / 2.0)).abs() < 0.01);
        assert!((r.y - (25.0 + (1055.0 - ch) / 2.0)).abs() < 0.01);
    }

    #[test]
    fn test_compute_frame_restore_returns_none() {
        let s = screen();
        assert!(TileAction::Restore.compute_frame(s).is_none());
    }

    #[test]
    fn test_compute_frame_movement_actions_return_none() {
        let s = screen();
        assert!(TileAction::MovePaneLeft.compute_frame(s).is_none());
        assert!(TileAction::MovePaneRight.compute_frame(s).is_none());
        assert!(TileAction::MovePaneUp.compute_frame(s).is_none());
        assert!(TileAction::MovePaneDown.compute_frame(s).is_none());
        assert!(TileAction::SwapPaneLeft.compute_frame(s).is_none());
        assert!(TileAction::SwapPaneRight.compute_frame(s).is_none());
        assert!(TileAction::SwapPaneUp.compute_frame(s).is_none());
        assert!(TileAction::SwapPaneDown.compute_frame(s).is_none());
        assert!(TileAction::EqualizeAll.compute_frame(s).is_none());
        assert!(TileAction::ToggleZoom.compute_frame(s).is_none());
    }

    #[test]
    fn test_compute_frame_halves_cover_full_screen() {
        let s = screen();
        let left = TileAction::LeftHalf.compute_frame(s).unwrap();
        let right = TileAction::RightHalf.compute_frame(s).unwrap();
        // Left + right should cover full width
        assert!((left.width + right.width - s.width).abs() < 0.01);
    }

    // ---------------------------------------------------------------
    // cycle_group tests
    // ---------------------------------------------------------------

    #[test]
    fn test_cycle_group_left() {
        let group = TileAction::LeftHalf.cycle_group().unwrap();
        assert_eq!(group.len(), 3);
        assert_eq!(group[0], TileAction::LeftHalf);
        assert_eq!(group[1], TileAction::LeftTwoThirds);
        assert_eq!(group[2], TileAction::LeftThird);
    }

    #[test]
    fn test_cycle_group_right() {
        let group = TileAction::RightHalf.cycle_group().unwrap();
        assert_eq!(group.len(), 3);
        assert_eq!(group[0], TileAction::RightHalf);
        assert_eq!(group[1], TileAction::RightTwoThirds);
        assert_eq!(group[2], TileAction::RightThird);
    }

    #[test]
    fn test_cycle_group_top_bottom_repeat() {
        let top = TileAction::TopHalf.cycle_group().unwrap();
        assert_eq!(top.len(), 3);
        assert!(top.iter().all(|a| *a == TileAction::TopHalf));

        let bottom = TileAction::BottomHalf.cycle_group().unwrap();
        assert_eq!(bottom.len(), 3);
        assert!(bottom.iter().all(|a| *a == TileAction::BottomHalf));
    }

    #[test]
    fn test_cycle_group_none_for_non_cycling() {
        assert!(TileAction::Maximize.cycle_group().is_none());
        assert!(TileAction::Center.cycle_group().is_none());
        assert!(TileAction::TopLeftQuarter.cycle_group().is_none());
        assert!(TileAction::LeftThird.cycle_group().is_none());
        assert!(TileAction::MovePaneLeft.cycle_group().is_none());
    }

    // ---------------------------------------------------------------
    // Orientation tests
    // ---------------------------------------------------------------

    #[test]
    fn test_orientation_toggle() {
        assert_eq!(Orientation::Horizontal.toggle(), Orientation::Vertical);
        assert_eq!(Orientation::Vertical.toggle(), Orientation::Horizontal);
    }

    #[test]
    fn test_orientation_double_toggle() {
        let o = Orientation::Horizontal;
        assert_eq!(o.toggle().toggle(), o);
    }

    // ---------------------------------------------------------------
    // GapConfig default
    // ---------------------------------------------------------------

    #[test]
    fn test_gap_config_default() {
        let g = GapConfig::default();
        assert_eq!(g.outer, 8.0);
        assert_eq!(g.inner, 8.0);
    }

    // ---------------------------------------------------------------
    // NodeId / WindowId uniqueness
    // ---------------------------------------------------------------

    #[test]
    fn test_node_id_unique() {
        let a = NodeId::next();
        let b = NodeId::next();
        let c = NodeId::next();
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }

    #[test]
    fn test_window_id_unique() {
        let a = WindowId::next();
        let b = WindowId::next();
        assert_ne!(a, b);
    }
}
