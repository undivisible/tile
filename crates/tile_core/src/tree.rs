use crate::types::*;
use serde::{Deserialize, Serialize};

/// A binary split tree for tiling layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Node {
    Split {
        orientation: Orientation,
        ratio: f32,
        first: Box<Node>,
        second: Box<Node>,
        id: NodeId,
    },
    Pane {
        tabs: Vec<ManagedWindow>,
        active: usize,
        id: NodeId,
        /// If true, this pane is currently zoomed (maximized).
        #[serde(default)]
        zoomed: bool,
    },
}

impl Node {
    /// Create a new empty pane.
    pub fn new_pane() -> Self {
        Node::Pane {
            tabs: Vec::new(),
            active: 0,
            id: NodeId::next(),
            zoomed: false,
        }
    }

    /// Create a new pane with one window.
    pub fn new_pane_with(window: ManagedWindow) -> Self {
        Node::Pane {
            tabs: vec![window],
            active: 0,
            id: NodeId::next(),
            zoomed: false,
        }
    }

    /// Get the node ID.
    pub fn id(&self) -> NodeId {
        match self {
            Node::Split { id, .. } => *id,
            Node::Pane { id, .. } => *id,
        }
    }

    /// Find a node by ID.
    pub fn find(&self, target: NodeId) -> Option<&Node> {
        if self.id() == target {
            return Some(self);
        }
        match self {
            Node::Split {
                first, second, ..
            } => first.find(target).or_else(|| second.find(target)),
            Node::Pane { .. } => None,
        }
    }

    /// Find a mutable reference to a node by ID.
    pub fn find_mut(&mut self, target: NodeId) -> Option<&mut Node> {
        if self.id() == target {
            return Some(self);
        }
        match self {
            Node::Split {
                first, second, ..
            } => {
                if let Some(n) = first.find_mut(target) {
                    Some(n)
                } else {
                    second.find_mut(target)
                }
            }
            Node::Pane { .. } => None,
        }
    }

    /// Find the pane containing a given window.
    pub fn find_pane_with_window(&self, window_id: WindowId) -> Option<NodeId> {
        match self {
            Node::Pane { tabs, id, .. } => {
                if tabs.iter().any(|w| w.id == window_id) {
                    Some(*id)
                } else {
                    None
                }
            }
            Node::Split {
                first, second, ..
            } => first
                .find_pane_with_window(window_id)
                .or_else(|| second.find_pane_with_window(window_id)),
        }
    }

    /// Find pane by AX pid and approximate frame match.
    pub fn find_pane_by_pid(&self, pid: i32) -> Option<NodeId> {
        match self {
            Node::Pane { tabs, id, .. } => {
                if tabs.iter().any(|w| w.pid == pid) {
                    Some(*id)
                } else {
                    None
                }
            }
            Node::Split {
                first, second, ..
            } => first
                .find_pane_by_pid(pid)
                .or_else(|| second.find_pane_by_pid(pid)),
        }
    }

    /// Get all pane node IDs in order (left-to-right, top-to-bottom).
    pub fn pane_ids(&self) -> Vec<NodeId> {
        match self {
            Node::Pane { id, .. } => vec![*id],
            Node::Split {
                first, second, ..
            } => {
                let mut ids = first.pane_ids();
                ids.extend(second.pane_ids());
                ids
            }
        }
    }

    /// Get all managed windows.
    pub fn all_windows(&self) -> Vec<&ManagedWindow> {
        match self {
            Node::Pane { tabs, .. } => tabs.iter().collect(),
            Node::Split {
                first, second, ..
            } => {
                let mut wins = first.all_windows();
                wins.extend(second.all_windows());
                wins
            }
        }
    }

    /// Count panes.
    pub fn pane_count(&self) -> usize {
        match self {
            Node::Pane { .. } => 1,
            Node::Split {
                first, second, ..
            } => first.pane_count() + second.pane_count(),
        }
    }

    /// Count total windows across all panes.
    pub fn window_count(&self) -> usize {
        match self {
            Node::Pane { tabs, .. } => tabs.len(),
            Node::Split {
                first, second, ..
            } => first.window_count() + second.window_count(),
        }
    }

    /// Split a pane into two. The existing pane content goes to `first`, and a new
    /// empty pane becomes `second`. Returns the new pane's NodeId.
    pub fn split_pane(
        &mut self,
        pane_id: NodeId,
        orientation: Orientation,
        ratio: f32,
    ) -> Option<NodeId> {
        if self.id() == pane_id {
            if let Node::Pane { tabs, zoomed, .. } = self {
                let existing_tabs = std::mem::take(tabs);
                let existing_zoomed = *zoomed;
                let new_pane_id = NodeId::next();
                let first = Node::Pane {
                    tabs: existing_tabs,
                    active: 0,
                    id: NodeId::next(),
                    zoomed: existing_zoomed,
                };
                let second = Node::Pane {
                    tabs: Vec::new(),
                    active: 0,
                    id: new_pane_id,
                    zoomed: false,
                };
                *self = Node::Split {
                    orientation,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                    id: NodeId::next(),
                };
                return Some(new_pane_id);
            }
            return None;
        }
        match self {
            Node::Split {
                first, second, ..
            } => first
                .split_pane(pane_id, orientation, ratio)
                .or_else(|| second.split_pane(pane_id, orientation, ratio)),
            Node::Pane { .. } => None,
        }
    }

    /// Add a window as a tab to the specified pane.
    pub fn stack_window(&mut self, pane_id: NodeId, window: ManagedWindow) -> bool {
        match self {
            Node::Pane { tabs, active, id, .. } if *id == pane_id => {
                tabs.push(window);
                *active = tabs.len() - 1;
                true
            }
            Node::Split {
                first, second, ..
            } => {
                if first.stack_window(pane_id, window.clone()) {
                    true
                } else {
                    second.stack_window(pane_id, window)
                }
            }
            _ => false,
        }
    }

    /// Remove a window by ID. If the pane becomes empty, the caller should clean up.
    /// Returns the removed window if found.
    pub fn remove_window(&mut self, window_id: WindowId) -> Option<ManagedWindow> {
        match self {
            Node::Pane { tabs, active, .. } => {
                if let Some(pos) = tabs.iter().position(|w| w.id == window_id) {
                    let win = tabs.remove(pos);
                    if *active >= tabs.len() && !tabs.is_empty() {
                        *active = tabs.len() - 1;
                    }
                    Some(win)
                } else {
                    None
                }
            }
            Node::Split {
                first, second, ..
            } => first
                .remove_window(window_id)
                .or_else(|| second.remove_window(window_id)),
        }
    }

    /// Remove empty panes and collapse single-child splits.
    pub fn cleanup(&mut self) {
        if let Node::Split {
            first, second, ..
        } = self
        {
            first.cleanup();
            second.cleanup();

            // If first is an empty pane, replace self with second
            if matches!(first.as_ref(), Node::Pane { tabs, .. } if tabs.is_empty()) {
                *self = *second.clone();
                return;
            }
            // If second is an empty pane, replace self with first
            if matches!(second.as_ref(), Node::Pane { tabs, .. } if tabs.is_empty()) {
                *self = *first.clone();
            }
        }
    }

    /// Equalize all split ratios to 0.5.
    pub fn equalize_all(&mut self) {
        if let Node::Split {
            ratio,
            first,
            second,
            ..
        } = self
        {
            *ratio = 0.5;
            first.equalize_all();
            second.equalize_all();
        }
    }

    /// Toggle the zoom state of a pane.
    pub fn toggle_zoom(&mut self, pane_id: NodeId) -> bool {
        match self {
            Node::Pane { id, zoomed, .. } if *id == pane_id => {
                *zoomed = !*zoomed;
                true
            }
            Node::Split {
                first, second, ..
            } => first.toggle_zoom(pane_id) || second.toggle_zoom(pane_id),
            _ => false,
        }
    }

    /// Check if any pane is zoomed.
    pub fn has_zoomed_pane(&self) -> Option<NodeId> {
        match self {
            Node::Pane { id, zoomed, .. } if *zoomed => Some(*id),
            Node::Split {
                first, second, ..
            } => first.has_zoomed_pane().or_else(|| second.has_zoomed_pane()),
            _ => None,
        }
    }

    /// Rotate the tree: swap all orientations.
    pub fn rotate_tree(&mut self) {
        if let Node::Split {
            orientation,
            first,
            second,
            ..
        } = self
        {
            *orientation = orientation.toggle();
            first.rotate_tree();
            second.rotate_tree();
        }
    }

    /// Resize a split by delta (positive = grow first, negative = grow second).
    pub fn resize_split(&mut self, split_id: NodeId, delta: f32) -> bool {
        match self {
            Node::Split {
                id,
                ratio,
                first,
                second,
                ..
            } => {
                if *id == split_id {
                    *ratio = (*ratio + delta).clamp(0.1, 0.9);
                    return true;
                }
                first.resize_split(split_id, delta) || second.resize_split(split_id, delta)
            }
            Node::Pane { .. } => false,
        }
    }

    /// Serialize to JSON.
    pub fn serialize(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Deserialize from JSON.
    pub fn deserialize(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

/// The tiling tree manager. Holds the root node and manages operations.
#[derive(Debug, Clone)]
pub struct TileTree {
    pub root: Node,
    pub gaps: GapConfig,
    /// The currently focused pane.
    pub focused_pane: Option<NodeId>,
}

impl TileTree {
    pub fn new() -> Self {
        Self {
            root: Node::new_pane(),
            gaps: GapConfig::default(),
            focused_pane: None,
        }
    }

    /// Add a window. If there's a focused pane with content, split it.
    /// If the focused pane is empty, add as first tab.
    pub fn add_window(&mut self, window: ManagedWindow) -> NodeId {
        // If we have a focused pane, try to put it there
        if let Some(pane_id) = self.focused_pane {
            if let Some(Node::Pane { tabs, .. }) = self.root.find(pane_id) {
                if tabs.is_empty() {
                    // Empty pane, add as first tab
                    self.root.stack_window(pane_id, window);
                    return pane_id;
                } else {
                    // Split the focused pane
                    if let Some(new_id) =
                        self.root.split_pane(pane_id, Orientation::Horizontal, 0.5)
                    {
                        self.root.stack_window(new_id, window);
                        self.focused_pane = Some(new_id);
                        return new_id;
                    }
                }
            }
        }

        // Find first empty pane, or the root pane
        let pane_ids = self.root.pane_ids();
        for &pid in &pane_ids {
            if let Some(Node::Pane { tabs, .. }) = self.root.find(pid) {
                if tabs.is_empty() {
                    self.root.stack_window(pid, window);
                    self.focused_pane = Some(pid);
                    return pid;
                }
            }
        }

        // No empty pane found, split the first pane
        if let Some(&first_pane) = pane_ids.first() {
            if let Some(new_id) = self
                .root
                .split_pane(first_pane, Orientation::Horizontal, 0.5)
            {
                self.root.stack_window(new_id, window);
                self.focused_pane = Some(new_id);
                return new_id;
            }
        }

        // Fallback: stack on root (shouldn't happen normally)
        let root_id = self.root.id();
        self.root.stack_window(root_id, window);
        root_id
    }

    /// Remove a window and clean up empty panes.
    pub fn remove_window(&mut self, window_id: WindowId) -> Option<ManagedWindow> {
        let win = self.root.remove_window(window_id);
        if win.is_some() {
            self.root.cleanup();
        }
        win
    }

    /// Navigate focus in a direction.
    pub fn navigate_focus(&mut self, direction: Direction, screen: Rect) {
        let layout = self.compute_layout(screen);
        let current = self.focused_pane;

        if layout.is_empty() {
            return;
        }

        let current_rect = current
            .and_then(|id| layout.iter().find(|(pid, _)| *pid == id))
            .map(|(_, r)| *r);

        let target = if let Some(cr) = current_rect {
            let (cx, cy) = cr.center();
            layout
                .iter()
                .filter(|(pid, _)| Some(*pid) != current)
                .filter(|(_, r)| match direction {
                    Direction::Left => r.center().0 < cx,
                    Direction::Right => r.center().0 > cx,
                    Direction::Up => r.center().1 < cy,
                    Direction::Down => r.center().1 > cy,
                })
                .min_by(|(_, a), (_, b)| {
                    let da = (a.center().0 - cx).powi(2) + (a.center().1 - cy).powi(2);
                    let db = (b.center().0 - cx).powi(2) + (b.center().1 - cy).powi(2);
                    da.partial_cmp(&db).unwrap()
                })
                .map(|(pid, _)| *pid)
        } else {
            layout.first().map(|(pid, _)| *pid)
        };

        if let Some(target_id) = target {
            self.focused_pane = Some(target_id);
        }
    }

    /// Compute layout frames for all panes.
    pub fn compute_layout(&self, screen: Rect) -> Vec<(NodeId, Rect)> {
        // If any pane is zoomed, only show that pane
        if let Some(zoomed_id) = self.root.has_zoomed_pane() {
            return vec![(zoomed_id, screen.inset(self.gaps.outer))];
        }

        let mut result = Vec::new();
        let outer = screen.inset(self.gaps.outer);
        Self::layout_node(&self.root, outer, self.gaps.inner, &mut result);
        result
    }

    fn layout_node(node: &Node, rect: Rect, gap: f64, result: &mut Vec<(NodeId, Rect)>) {
        match node {
            Node::Pane { id, .. } => {
                result.push((*id, rect));
            }
            Node::Split {
                orientation,
                ratio,
                first,
                second,
                ..
            } => {
                let r = *ratio as f64;
                let half_gap = gap / 2.0;
                match orientation {
                    Orientation::Horizontal => {
                        let first_w = rect.width * r - half_gap;
                        let second_w = rect.width * (1.0 - r) - half_gap;
                        let first_rect = Rect::new(rect.x, rect.y, first_w, rect.height);
                        let second_rect = Rect::new(
                            rect.x + rect.width * r + half_gap,
                            rect.y,
                            second_w,
                            rect.height,
                        );
                        Self::layout_node(first, first_rect, gap, result);
                        Self::layout_node(second, second_rect, gap, result);
                    }
                    Orientation::Vertical => {
                        let first_h = rect.height * r - half_gap;
                        let second_h = rect.height * (1.0 - r) - half_gap;
                        let first_rect = Rect::new(rect.x, rect.y, rect.width, first_h);
                        let second_rect = Rect::new(
                            rect.x,
                            rect.y + rect.height * r + half_gap,
                            rect.width,
                            second_h,
                        );
                        Self::layout_node(first, first_rect, gap, result);
                        Self::layout_node(second, second_rect, gap, result);
                    }
                }
            }
        }
    }

    /// Cycle the active tab in a stacked pane.
    /// `forward = true` means next tab, `false` means previous tab.
    /// Returns the index of the newly active tab, or None if the pane was not found
    /// or has fewer than 2 tabs.
    pub fn cycle_tab(&mut self, pane_id: NodeId, forward: bool) -> Option<usize> {
        if let Some(Node::Pane { tabs, active, id: _, .. }) = self.root.find_mut(pane_id) {
            if tabs.len() < 2 {
                return None;
            }
            if forward {
                *active = (*active + 1) % tabs.len();
            } else {
                *active = if *active == 0 {
                    tabs.len() - 1
                } else {
                    *active - 1
                };
            }
            Some(*active)
        } else {
            None
        }
    }

    /// Compute a frame for snapping a window beside a target window (floating, not BSP).
    /// The snapped window gets the same height and y as the target, and is placed on the
    /// given side with a width equal to its current width (clamped to half screen width).
    pub fn snap_window_beside(
        target_frame: Rect,
        source_frame: Rect,
        side: crate::types::SnapSide,
        screen: Rect,
    ) -> Rect {
        let snap_width = source_frame.width.min(screen.width / 2.0);
        let x = match side {
            crate::types::SnapSide::Left => (target_frame.x - snap_width).max(screen.x),
            crate::types::SnapSide::Right => {
                let x = target_frame.x + target_frame.width;
                x.min(screen.x + screen.width - snap_width)
            }
        };
        Rect::new(x, target_frame.y, snap_width, target_frame.height)
    }

    /// Swap two panes' contents.
    pub fn swap_panes(&mut self, a: NodeId, b: NodeId) {
        // Collect tabs from both panes
        let a_tabs = if let Some(Node::Pane { tabs, .. }) = self.root.find(a) {
            tabs.clone()
        } else {
            return;
        };
        let b_tabs = if let Some(Node::Pane { tabs, .. }) = self.root.find(b) {
            tabs.clone()
        } else {
            return;
        };

        if let Some(Node::Pane { tabs, .. }) = self.root.find_mut(a) {
            *tabs = b_tabs;
        }
        if let Some(Node::Pane { tabs, .. }) = self.root.find_mut(b) {
            *tabs = a_tabs;
        }
    }
}

impl Default for TileTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_window(name: &str) -> ManagedWindow {
        ManagedWindow::new(
            AXWindowRef::new(1, 0, 0),
            1,
            name.to_string(),
            "TestApp".to_string(),
            Rect::new(0.0, 0.0, 800.0, 600.0),
        )
    }

    #[test]
    fn test_add_windows_creates_splits() {
        let mut tree = TileTree::new();
        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);

        tree.add_window(test_window("win1"));
        assert_eq!(tree.root.pane_count(), 1);
        assert_eq!(tree.root.window_count(), 1);

        tree.add_window(test_window("win2"));
        assert_eq!(tree.root.pane_count(), 2);
        assert_eq!(tree.root.window_count(), 2);

        let layout = tree.compute_layout(screen);
        assert_eq!(layout.len(), 2);
    }

    #[test]
    fn test_remove_window_cleans_up() {
        let mut tree = TileTree::new();

        let w1 = test_window("win1");
        let w1_id = w1.id;
        tree.add_window(w1);
        tree.add_window(test_window("win2"));

        assert_eq!(tree.root.pane_count(), 2);
        tree.remove_window(w1_id);
        assert_eq!(tree.root.pane_count(), 1);
    }

    #[test]
    fn test_equalize() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        if let Node::Split { ratio, .. } = &mut tree.root {
            *ratio = 0.3;
        }
        tree.root.equalize_all();
        if let Node::Split { ratio, .. } = &tree.root {
            assert_eq!(*ratio, 0.5);
        }
    }

    #[test]
    fn test_serialize_roundtrip() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        let json = tree.root.serialize();
        let restored = Node::deserialize(&json).unwrap();
        assert_eq!(restored.pane_count(), 2);
        assert_eq!(restored.window_count(), 2);
    }

    #[test]
    fn test_cycle_tab() {
        let mut tree = TileTree::new();
        // Add first window — goes into root pane
        tree.add_window(test_window("win1"));
        let pane_id = tree.focused_pane.unwrap();

        // Stack a second window into the same pane
        tree.root.stack_window(pane_id, test_window("win2"));
        tree.root.stack_window(pane_id, test_window("win3"));

        // Active should be 2 (last stacked)
        if let Some(Node::Pane { active, .. }) = tree.root.find(pane_id) {
            assert_eq!(*active, 2);
        }

        // Cycle forward: 2 -> 0
        let result = tree.cycle_tab(pane_id, true);
        assert_eq!(result, Some(0));

        // Cycle forward: 0 -> 1
        let result = tree.cycle_tab(pane_id, true);
        assert_eq!(result, Some(1));

        // Cycle backward: 1 -> 0
        let result = tree.cycle_tab(pane_id, false);
        assert_eq!(result, Some(0));

        // Cycle backward: 0 -> 2 (wraps)
        let result = tree.cycle_tab(pane_id, false);
        assert_eq!(result, Some(2));
    }

    #[test]
    fn test_cycle_tab_single_window() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        let pane_id = tree.focused_pane.unwrap();

        // Should return None for single-window pane
        assert_eq!(tree.cycle_tab(pane_id, true), None);
        assert_eq!(tree.cycle_tab(pane_id, false), None);
    }

    #[test]
    fn test_snap_window_beside_right() {
        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let target = Rect::new(100.0, 50.0, 800.0, 600.0);
        let source = Rect::new(0.0, 0.0, 500.0, 400.0);

        let result = TileTree::snap_window_beside(
            target,
            source,
            crate::types::SnapSide::Right,
            screen,
        );

        // Should be placed to the right of target, matching target height
        assert_eq!(result.x, 900.0); // target.x + target.width
        assert_eq!(result.y, 50.0); // same y as target
        assert_eq!(result.width, 500.0); // source width preserved
        assert_eq!(result.height, 600.0); // target height
    }

    #[test]
    fn test_snap_window_beside_left() {
        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let target = Rect::new(500.0, 50.0, 800.0, 600.0);
        let source = Rect::new(0.0, 0.0, 400.0, 400.0);

        let result = TileTree::snap_window_beside(
            target,
            source,
            crate::types::SnapSide::Left,
            screen,
        );

        // Should be placed to the left of target
        assert_eq!(result.x, 100.0); // target.x - source.width
        assert_eq!(result.y, 50.0);
        assert_eq!(result.width, 400.0);
        assert_eq!(result.height, 600.0);
    }

    #[test]
    fn test_snap_window_beside_clamps_to_screen() {
        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let target = Rect::new(100.0, 50.0, 800.0, 600.0);
        let source = Rect::new(0.0, 0.0, 500.0, 400.0);

        // Snap left — would go to x=-400, should clamp to 0
        let result = TileTree::snap_window_beside(
            target,
            source,
            crate::types::SnapSide::Left,
            screen,
        );
        assert_eq!(result.x, 0.0); // clamped to screen.x
    }
}
