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
    /// empty pane becomes `second`.
    ///
    /// Returns `(first_id, second_id)` — the IDs of the two resulting panes.
    /// `first` retains the original content; `second` is empty.
    pub fn split_pane(
        &mut self,
        pane_id: NodeId,
        orientation: Orientation,
        ratio: f32,
    ) -> Option<(NodeId, NodeId)> {
        if self.id() == pane_id {
            if let Node::Pane { tabs, zoomed, .. } = self {
                let existing_tabs = std::mem::take(tabs);
                let existing_zoomed = *zoomed;
                let first_id = NodeId::next();
                let second_id = NodeId::next();
                let first = Node::Pane {
                    tabs: existing_tabs,
                    active: 0,
                    id: first_id,
                    zoomed: existing_zoomed,
                };
                let second = Node::Pane {
                    tabs: Vec::new(),
                    active: 0,
                    id: second_id,
                    zoomed: false,
                };
                *self = Node::Split {
                    orientation,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                    id: NodeId::next(),
                };
                return Some((first_id, second_id));
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

    pub fn first_split_id(&self) -> Option<NodeId> {
        match self {
            Node::Split {
                id,
                ..
            } => Some(*id),
            Node::Pane { .. } => None,
        }
    }
}

/// A boundary line between two BSP panes, used for hit-testing split-resize drags.
#[derive(Debug, Clone, Copy)]
pub struct SplitLine {
    pub split_id: NodeId,
    /// Horizontal split = side-by-side panes = vertical divider line (drag left/right).
    pub is_horizontal: bool,
    /// Position along the split axis: x for vertical divider, y for horizontal divider.
    pub position: f64,
    /// Start of the span perpendicular to the axis (y-start for vertical, x-start for horizontal).
    pub span_start: f64,
    /// End of the span.
    pub span_end: f64,
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
                    if let Some((_first, second)) =
                        self.root.split_pane(pane_id, Orientation::Horizontal, 0.5)
                    {
                        self.root.stack_window(second, window);
                        self.focused_pane = Some(second);
                        return second;
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
            if let Some((_first, second)) = self
                .root
                .split_pane(first_pane, Orientation::Horizontal, 0.5)
            {
                self.root.stack_window(second, window);
                self.focused_pane = Some(second);
                return second;
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

    /// Return all split divider lines in the current layout, for drag hit-testing.
    pub fn split_lines(&self, screen: Rect) -> Vec<SplitLine> {
        if let Some(_) = self.root.has_zoomed_pane() {
            return Vec::new();
        }
        let outer = screen.inset(self.gaps.outer);
        let mut lines = Vec::new();
        Self::collect_split_lines(&self.root, outer, self.gaps.inner, &mut lines);
        lines
    }

    fn collect_split_lines(node: &Node, rect: Rect, gap: f64, out: &mut Vec<SplitLine>) {
        if let Node::Split { orientation, ratio, first, second, id } = node {
            let r = *ratio as f64;
            let half_gap = gap / 2.0;
            match orientation {
                Orientation::Horizontal => {
                    let divider_x = rect.x + rect.width * r;
                    out.push(SplitLine {
                        split_id: *id,
                        is_horizontal: true,
                        position: divider_x,
                        span_start: rect.y,
                        span_end: rect.y + rect.height,
                    });
                    let first_rect = Rect::new(rect.x, rect.y, rect.width * r - half_gap, rect.height);
                    let second_rect = Rect::new(divider_x + half_gap, rect.y, rect.width * (1.0 - r) - half_gap, rect.height);
                    Self::collect_split_lines(first, first_rect, gap, out);
                    Self::collect_split_lines(second, second_rect, gap, out);
                }
                Orientation::Vertical => {
                    let divider_y = rect.y + rect.height * r;
                    out.push(SplitLine {
                        split_id: *id,
                        is_horizontal: false,
                        position: divider_y,
                        span_start: rect.x,
                        span_end: rect.x + rect.width,
                    });
                    let first_rect = Rect::new(rect.x, rect.y, rect.width, rect.height * r - half_gap);
                    let second_rect = Rect::new(rect.x, divider_y + half_gap, rect.width, rect.height * (1.0 - r) - half_gap);
                    Self::collect_split_lines(first, first_rect, gap, out);
                    Self::collect_split_lines(second, second_rect, gap, out);
                }
            }
        }
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

    fn test_window_with_pid(name: &str, pid: i32) -> ManagedWindow {
        ManagedWindow::new(
            AXWindowRef::new(pid, 0, 0),
            pid,
            name.to_string(),
            "TestApp".to_string(),
            Rect::new(0.0, 0.0, 800.0, 600.0),
        )
    }

    fn screen() -> Rect {
        Rect::new(0.0, 0.0, 1920.0, 1080.0)
    }

    // ---------------------------------------------------------------
    // add_window: 1 through 5 windows
    // ---------------------------------------------------------------

    #[test]
    fn test_add_one_window() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        assert_eq!(tree.root.pane_count(), 1);
        assert_eq!(tree.root.window_count(), 1);
    }

    #[test]
    fn test_add_two_windows() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));
        assert_eq!(tree.root.pane_count(), 2);
        assert_eq!(tree.root.window_count(), 2);
    }

    #[test]
    fn test_add_three_windows() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));
        tree.add_window(test_window("win3"));
        assert_eq!(tree.root.pane_count(), 3);
        assert_eq!(tree.root.window_count(), 3);
    }

    #[test]
    fn test_add_four_windows() {
        let mut tree = TileTree::new();
        for i in 1..=4 {
            tree.add_window(test_window(&format!("win{}", i)));
        }
        assert_eq!(tree.root.pane_count(), 4);
        assert_eq!(tree.root.window_count(), 4);
    }

    #[test]
    fn test_add_five_windows() {
        let mut tree = TileTree::new();
        for i in 1..=5 {
            tree.add_window(test_window(&format!("win{}", i)));
        }
        assert_eq!(tree.root.pane_count(), 5);
        assert_eq!(tree.root.window_count(), 5);
    }

    #[test]
    fn test_add_windows_creates_splits() {
        let mut tree = TileTree::new();

        tree.add_window(test_window("win1"));
        assert_eq!(tree.root.pane_count(), 1);
        assert_eq!(tree.root.window_count(), 1);

        tree.add_window(test_window("win2"));
        assert_eq!(tree.root.pane_count(), 2);
        assert_eq!(tree.root.window_count(), 2);

        let layout = tree.compute_layout(screen());
        assert_eq!(layout.len(), 2);
    }

    // ---------------------------------------------------------------
    // remove_window
    // ---------------------------------------------------------------

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
    fn test_remove_second_window() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        let w2 = test_window("win2");
        let w2_id = w2.id;
        tree.add_window(w2);

        assert_eq!(tree.root.pane_count(), 2);
        let removed = tree.remove_window(w2_id);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().title, "win2");
        assert_eq!(tree.root.pane_count(), 1);
        assert_eq!(tree.root.window_count(), 1);
    }

    #[test]
    fn test_remove_middle_of_three() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        let w2 = test_window("win2");
        let w2_id = w2.id;
        tree.add_window(w2);
        tree.add_window(test_window("win3"));

        assert_eq!(tree.root.pane_count(), 3);
        tree.remove_window(w2_id);
        assert_eq!(tree.root.pane_count(), 2);
        assert_eq!(tree.root.window_count(), 2);
    }

    #[test]
    fn test_remove_nonexistent_window() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        let fake_id = WindowId::next();
        let result = tree.remove_window(fake_id);
        assert!(result.is_none());
        assert_eq!(tree.root.window_count(), 1);
    }

    #[test]
    fn test_remove_all_windows() {
        let mut tree = TileTree::new();
        let w1 = test_window("win1");
        let w1_id = w1.id;
        let w2 = test_window("win2");
        let w2_id = w2.id;
        tree.add_window(w1);
        tree.add_window(w2);

        tree.remove_window(w1_id);
        tree.remove_window(w2_id);
        assert_eq!(tree.root.window_count(), 0);
        assert_eq!(tree.root.pane_count(), 1);
    }

    // ---------------------------------------------------------------
    // split_pane
    // ---------------------------------------------------------------

    #[test]
    fn test_split_pane_creates_two_children() {
        let mut node = Node::new_pane_with(test_window("win1"));
        let pane_id = node.id();
        let new_id = node.split_pane(pane_id, Orientation::Horizontal, 0.5);
        assert!(new_id.is_some());
        assert_eq!(node.pane_count(), 2);
        assert_eq!(node.window_count(), 1);
    }

    #[test]
    fn test_split_pane_vertical() {
        let mut node = Node::new_pane_with(test_window("win1"));
        let pane_id = node.id();
        let split = node.split_pane(pane_id, Orientation::Vertical, 0.5);
        assert!(split.is_some());
        assert_eq!(node.pane_count(), 2);
        if let Node::Split { orientation, .. } = &node {
            assert_eq!(*orientation, Orientation::Vertical);
        } else {
            panic!("Expected split node");
        }
    }

    #[test]
    fn test_split_pane_nonexistent() {
        let mut node = Node::new_pane();
        let fake_id = NodeId::next();
        let result = node.split_pane(fake_id, Orientation::Horizontal, 0.5);
        assert!(result.is_none());
    }

    #[test]
    fn test_split_nested() {
        let mut node = Node::new_pane_with(test_window("win1"));
        let pane_id = node.id();
        let (_first, second) = node.split_pane(pane_id, Orientation::Horizontal, 0.5).unwrap();
        let nested = node.split_pane(second, Orientation::Vertical, 0.5);
        assert!(nested.is_some());
        assert_eq!(node.pane_count(), 3);
    }

    #[test]
    fn test_split_pane_preserves_existing_content() {
        let mut node = Node::new_pane_with(test_window("original"));
        let pane_id = node.id();
        node.split_pane(pane_id, Orientation::Horizontal, 0.5);
        // The original window should still be in the tree
        assert_eq!(node.window_count(), 1);
        let all = node.all_windows();
        assert_eq!(all[0].title, "original");
    }

    #[test]
    fn test_split_pane_custom_ratio() {
        let mut node = Node::new_pane_with(test_window("win1"));
        let pane_id = node.id();
        node.split_pane(pane_id, Orientation::Horizontal, 0.7);
        if let Node::Split { ratio, .. } = &node {
            assert!((*ratio - 0.7).abs() < 0.001);
        } else {
            panic!("Expected split node");
        }
    }

    // ---------------------------------------------------------------
    // stack_window
    // ---------------------------------------------------------------

    #[test]
    fn test_stack_window_adds_tab() {
        let mut node = Node::new_pane_with(test_window("win1"));
        let pane_id = node.id();
        let success = node.stack_window(pane_id, test_window("win2"));
        assert!(success);
        assert_eq!(node.window_count(), 2);
        if let Node::Pane { tabs, active, .. } = &node {
            assert_eq!(tabs.len(), 2);
            assert_eq!(*active, 1);
        } else {
            panic!("Expected pane node");
        }
    }

    #[test]
    fn test_stack_window_wrong_pane() {
        let mut node = Node::new_pane_with(test_window("win1"));
        let fake_id = NodeId::next();
        let success = node.stack_window(fake_id, test_window("win2"));
        assert!(!success);
        assert_eq!(node.window_count(), 1);
    }

    #[test]
    fn test_stack_multiple_tabs() {
        let mut node = Node::new_pane();
        let pane_id = node.id();
        for i in 0..5 {
            node.stack_window(pane_id, test_window(&format!("tab{}", i)));
        }
        assert_eq!(node.window_count(), 5);
        assert_eq!(node.pane_count(), 1);
    }

    #[test]
    fn test_stack_window_in_nested_split() {
        let mut node = Node::new_pane_with(test_window("win1"));
        let pane_id = node.id();
        let (_first, second) = node.split_pane(pane_id, Orientation::Horizontal, 0.5).unwrap();
        let success = node.stack_window(second, test_window("stacked"));
        assert!(success);
        assert_eq!(node.window_count(), 2);
    }

    // ---------------------------------------------------------------
    // navigate_focus
    // ---------------------------------------------------------------

    #[test]
    fn test_navigate_focus_left_right() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("left"));
        tree.add_window(test_window("right"));

        let layout = tree.compute_layout(screen());
        assert_eq!(layout.len(), 2);

        let right_pane = tree.focused_pane.unwrap();

        tree.navigate_focus(Direction::Left, screen());
        let left_pane = tree.focused_pane.unwrap();
        assert_ne!(left_pane, right_pane);

        tree.navigate_focus(Direction::Right, screen());
        assert_eq!(tree.focused_pane.unwrap(), right_pane);
    }

    #[test]
    fn test_navigate_focus_up_down() {
        let mut tree = TileTree::new();
        let w1 = test_window("top");
        tree.add_window(w1);

        let focused = tree.focused_pane.unwrap();
        if let Some((_first, second)) = tree.root.split_pane(focused, Orientation::Vertical, 0.5) {
            tree.root.stack_window(second, test_window("bottom"));
            tree.focused_pane = Some(second);
        }

        let bottom_pane = tree.focused_pane.unwrap();
        tree.navigate_focus(Direction::Up, screen());
        let top_pane = tree.focused_pane.unwrap();
        assert_ne!(top_pane, bottom_pane);

        tree.navigate_focus(Direction::Down, screen());
        assert_eq!(tree.focused_pane.unwrap(), bottom_pane);
    }

    #[test]
    fn test_navigate_focus_single_pane_no_change() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("only"));
        let pane = tree.focused_pane.unwrap();
        tree.navigate_focus(Direction::Left, screen());
        assert_eq!(tree.focused_pane.unwrap(), pane);
    }

    #[test]
    fn test_navigate_focus_empty_tree() {
        let mut tree = TileTree::new();
        tree.navigate_focus(Direction::Left, screen());
        // Should not panic
    }

    #[test]
    fn test_navigate_focus_all_four_directions() {
        let mut tree = TileTree::new();
        tree.gaps = GapConfig { outer: 0.0, inner: 0.0 };
        tree.add_window(test_window("tl"));
        tree.add_window(test_window("tr"));

        let pane_ids = tree.root.pane_ids();
        let tl_pane = pane_ids[0];
        if let Some((_first, bl_id)) = tree.root.split_pane(tl_pane, Orientation::Vertical, 0.5) {
            tree.root.stack_window(bl_id, test_window("bl"));
        }

        let pane_ids = tree.root.pane_ids();
        let tr_pane = pane_ids[1];
        if let Some((_first, br_id)) = tree.root.split_pane(tr_pane, Orientation::Vertical, 0.5) {
            tree.root.stack_window(br_id, test_window("br"));
        }

        let layout = tree.compute_layout(screen());
        assert_eq!(layout.len(), 4);

        // Focus top-left
        tree.focused_pane = Some(layout[0].0);

        tree.navigate_focus(Direction::Right, screen());
        assert_ne!(tree.focused_pane, Some(layout[0].0));

        tree.navigate_focus(Direction::Down, screen());
        assert!(tree.focused_pane.is_some());

        tree.navigate_focus(Direction::Left, screen());
        assert!(tree.focused_pane.is_some());

        tree.navigate_focus(Direction::Up, screen());
        assert!(tree.focused_pane.is_some());
    }

    // ---------------------------------------------------------------
    // swap_panes
    // ---------------------------------------------------------------

    #[test]
    fn test_swap_panes_contents() {
        let mut tree = TileTree::new();
        tree.add_window(test_window_with_pid("left_win", 100));
        tree.add_window(test_window_with_pid("right_win", 200));

        let pane_ids = tree.root.pane_ids();
        let (a, b) = (pane_ids[0], pane_ids[1]);

        let a_title = if let Some(Node::Pane { tabs, .. }) = tree.root.find(a) {
            tabs[0].title.clone()
        } else {
            panic!("Expected pane");
        };
        let b_title = if let Some(Node::Pane { tabs, .. }) = tree.root.find(b) {
            tabs[0].title.clone()
        } else {
            panic!("Expected pane");
        };

        tree.swap_panes(a, b);

        if let Some(Node::Pane { tabs, .. }) = tree.root.find(a) {
            assert_eq!(tabs[0].title, b_title);
        }
        if let Some(Node::Pane { tabs, .. }) = tree.root.find(b) {
            assert_eq!(tabs[0].title, a_title);
        }
    }

    #[test]
    fn test_swap_panes_same_id() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        let pane_ids = tree.root.pane_ids();
        let a = pane_ids[0];
        tree.swap_panes(a, a);
        if let Some(Node::Pane { tabs, .. }) = tree.root.find(a) {
            assert_eq!(tabs[0].title, "win1");
        }
    }

    #[test]
    fn test_swap_panes_double_swap_restores() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win_a"));
        tree.add_window(test_window("win_b"));
        let pane_ids = tree.root.pane_ids();
        let (a, b) = (pane_ids[0], pane_ids[1]);

        tree.swap_panes(a, b);
        tree.swap_panes(a, b);

        // After double swap, should be back to original
        if let Some(Node::Pane { tabs, .. }) = tree.root.find(a) {
            assert_eq!(tabs[0].title, "win_a");
        }
        if let Some(Node::Pane { tabs, .. }) = tree.root.find(b) {
            assert_eq!(tabs[0].title, "win_b");
        }
    }

    // ---------------------------------------------------------------
    // compute_layout
    // ---------------------------------------------------------------

    #[test]
    fn test_layout_single_pane_fills_screen() {
        let mut tree = TileTree::new();
        tree.gaps = GapConfig { outer: 0.0, inner: 0.0 };
        tree.add_window(test_window("win1"));

        let layout = tree.compute_layout(screen());
        assert_eq!(layout.len(), 1);
        let (_, r) = &layout[0];
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 0.0);
        assert_eq!(r.width, 1920.0);
        assert_eq!(r.height, 1080.0);
    }

    #[test]
    fn test_layout_single_pane_with_gaps() {
        let mut tree = TileTree::new();
        tree.gaps = GapConfig { outer: 10.0, inner: 8.0 };
        tree.add_window(test_window("win1"));

        let layout = tree.compute_layout(screen());
        assert_eq!(layout.len(), 1);
        let (_, r) = &layout[0];
        assert_eq!(r.x, 10.0);
        assert_eq!(r.y, 10.0);
        assert_eq!(r.width, 1900.0);
        assert_eq!(r.height, 1060.0);
    }

    #[test]
    fn test_layout_two_horizontal_panes_split_width() {
        let mut tree = TileTree::new();
        tree.gaps = GapConfig { outer: 0.0, inner: 0.0 };
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        let layout = tree.compute_layout(screen());
        assert_eq!(layout.len(), 2);

        let (_, left) = &layout[0];
        let (_, right) = &layout[1];
        assert!((left.width - 960.0).abs() < 1.0);
        assert!((right.width - 960.0).abs() < 1.0);
        assert_eq!(left.height, 1080.0);
        assert_eq!(right.height, 1080.0);
    }

    #[test]
    fn test_layout_two_vertical_panes_split_height() {
        let mut tree = TileTree::new();
        tree.gaps = GapConfig { outer: 0.0, inner: 0.0 };

        let w1 = test_window("top");
        tree.add_window(w1);
        let focused = tree.focused_pane.unwrap();
        if let Some((_first, new_id)) = tree.root.split_pane(focused, Orientation::Vertical, 0.5) {
            tree.root.stack_window(new_id, test_window("bottom"));
        }

        let layout = tree.compute_layout(screen());
        assert_eq!(layout.len(), 2);
        let (_, top) = &layout[0];
        let (_, bottom) = &layout[1];
        assert!((top.height - 540.0).abs() < 1.0);
        assert!((bottom.height - 540.0).abs() < 1.0);
        assert_eq!(top.width, 1920.0);
        assert_eq!(bottom.width, 1920.0);
    }

    #[test]
    fn test_layout_nested_splits() {
        let mut tree = TileTree::new();
        tree.gaps = GapConfig { outer: 0.0, inner: 0.0 };
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));
        tree.add_window(test_window("win3"));

        let layout = tree.compute_layout(screen());
        assert_eq!(layout.len(), 3);
        for (_, r) in &layout {
            assert!(r.width > 0.0);
            assert!(r.height > 0.0);
        }
    }

    #[test]
    fn test_layout_zoomed_pane_fills_screen() {
        let mut tree = TileTree::new();
        tree.gaps = GapConfig { outer: 0.0, inner: 0.0 };
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        let pane_ids = tree.root.pane_ids();
        let zoom_pane = pane_ids[0];
        tree.root.toggle_zoom(zoom_pane);

        let layout = tree.compute_layout(screen());
        assert_eq!(layout.len(), 1);
        assert_eq!(layout[0].0, zoom_pane);
        assert_eq!(layout[0].1.width, 1920.0);
        assert_eq!(layout[0].1.height, 1080.0);
    }

    #[test]
    fn test_layout_with_inner_gaps() {
        let mut tree = TileTree::new();
        tree.gaps = GapConfig { outer: 0.0, inner: 20.0 };
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        let layout = tree.compute_layout(screen());
        assert_eq!(layout.len(), 2);
        let (_, left) = &layout[0];
        let (_, right) = &layout[1];

        // With inner gap of 20, half_gap = 10
        // left width = 1920 * 0.5 - 10 = 950
        assert!((left.width - 950.0).abs() < 1.0);
        assert!((right.width - 950.0).abs() < 1.0);
        // Gap between them
        let left_right_edge = left.x + left.width;
        let right_left_edge = right.x;
        assert!((right_left_edge - left_right_edge - 20.0).abs() < 1.0);
    }

    #[test]
    fn test_layout_uneven_ratio() {
        let mut tree = TileTree::new();
        tree.gaps = GapConfig { outer: 0.0, inner: 0.0 };
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        // Set ratio to 0.7 (70% for first pane)
        if let Node::Split { ratio, .. } = &mut tree.root {
            *ratio = 0.7;
        }

        let layout = tree.compute_layout(screen());
        let (_, left) = &layout[0];
        let (_, right) = &layout[1];
        assert!((left.width - 1344.0).abs() < 1.0); // 1920 * 0.7
        assert!((right.width - 576.0).abs() < 1.0); // 1920 * 0.3
    }

    // ---------------------------------------------------------------
    // equalize_all
    // ---------------------------------------------------------------

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
    fn test_equalize_nested() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));
        tree.add_window(test_window("win3"));

        fn set_ratios(node: &mut Node, val: f32) {
            if let Node::Split { ratio, first, second, .. } = node {
                *ratio = val;
                set_ratios(first, val);
                set_ratios(second, val);
            }
        }
        set_ratios(&mut tree.root, 0.7);

        tree.root.equalize_all();

        fn check_ratios(node: &Node) {
            if let Node::Split { ratio, first, second, .. } = node {
                assert_eq!(*ratio, 0.5);
                check_ratios(first);
                check_ratios(second);
            }
        }
        check_ratios(&tree.root);
    }

    #[test]
    fn test_equalize_single_pane_noop() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.root.equalize_all();
        assert_eq!(tree.root.pane_count(), 1);
    }

    // ---------------------------------------------------------------
    // toggle_zoom
    // ---------------------------------------------------------------

    #[test]
    fn test_toggle_zoom() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        let pane_ids = tree.root.pane_ids();
        let pane = pane_ids[0];

        assert!(tree.root.has_zoomed_pane().is_none());
        tree.root.toggle_zoom(pane);
        assert_eq!(tree.root.has_zoomed_pane(), Some(pane));
        tree.root.toggle_zoom(pane);
        assert!(tree.root.has_zoomed_pane().is_none());
    }

    #[test]
    fn test_toggle_zoom_nonexistent_pane() {
        let mut node = Node::new_pane();
        let fake = NodeId::next();
        assert!(!node.toggle_zoom(fake));
    }

    #[test]
    fn test_zoom_affects_layout() {
        let mut tree = TileTree::new();
        tree.gaps = GapConfig { outer: 0.0, inner: 0.0 };
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));
        tree.add_window(test_window("win3"));

        // Before zoom: 3 panes
        let layout_before = tree.compute_layout(screen());
        assert_eq!(layout_before.len(), 3);

        // Zoom one pane
        let pane_ids = tree.root.pane_ids();
        tree.root.toggle_zoom(pane_ids[1]);

        // After zoom: only 1 pane visible
        let layout_after = tree.compute_layout(screen());
        assert_eq!(layout_after.len(), 1);
        assert_eq!(layout_after[0].0, pane_ids[1]);

        // Unzoom: back to 3
        tree.root.toggle_zoom(pane_ids[1]);
        let layout_restored = tree.compute_layout(screen());
        assert_eq!(layout_restored.len(), 3);
    }

    // ---------------------------------------------------------------
    // rotate_tree
    // ---------------------------------------------------------------

    #[test]
    fn test_rotate_tree() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        if let Node::Split { orientation, .. } = &tree.root {
            assert_eq!(*orientation, Orientation::Horizontal);
        }

        tree.root.rotate_tree();
        if let Node::Split { orientation, .. } = &tree.root {
            assert_eq!(*orientation, Orientation::Vertical);
        }

        tree.root.rotate_tree();
        if let Node::Split { orientation, .. } = &tree.root {
            assert_eq!(*orientation, Orientation::Horizontal);
        }
    }

    #[test]
    fn test_rotate_tree_nested() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        let focused = tree.focused_pane.unwrap();
        if let Some((_first, new_id)) = tree.root.split_pane(focused, Orientation::Vertical, 0.5) {
            tree.root.stack_window(new_id, test_window("win3"));
        }

        tree.root.rotate_tree();

        fn count_orientations(node: &Node, h: &mut usize, v: &mut usize) {
            if let Node::Split { orientation, first, second, .. } = node {
                match orientation {
                    Orientation::Horizontal => *h += 1,
                    Orientation::Vertical => *v += 1,
                }
                count_orientations(first, h, v);
                count_orientations(second, h, v);
            }
        }
        let (mut h, mut v) = (0, 0);
        count_orientations(&tree.root, &mut h, &mut v);
        // Root H->V, nested V->H
        assert_eq!(v, 1);
        assert_eq!(h, 1);
    }

    #[test]
    fn test_rotate_single_pane_noop() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.root.rotate_tree();
        assert_eq!(tree.root.pane_count(), 1);
    }

    // ---------------------------------------------------------------
    // resize_split
    // ---------------------------------------------------------------

    #[test]
    fn test_resize_split() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        let split_id = tree.root.id();
        assert!(tree.root.resize_split(split_id, 0.1));

        if let Node::Split { ratio, .. } = &tree.root {
            assert!((*ratio - 0.6).abs() < 0.001);
        }
    }

    #[test]
    fn test_resize_split_clamps_min() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        let split_id = tree.root.id();
        tree.root.resize_split(split_id, -0.9);

        if let Node::Split { ratio, .. } = &tree.root {
            assert!((*ratio - 0.1).abs() < 0.001);
        }
    }

    #[test]
    fn test_resize_split_clamps_max() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        let split_id = tree.root.id();
        tree.root.resize_split(split_id, 0.9);

        if let Node::Split { ratio, .. } = &tree.root {
            assert!((*ratio - 0.9).abs() < 0.001);
        }
    }

    #[test]
    fn test_resize_split_nonexistent() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        let fake = NodeId::next();
        assert!(!tree.root.resize_split(fake, 0.1));
    }

    #[test]
    fn test_resize_split_incremental() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));

        let split_id = tree.root.id();
        // Multiple small resizes
        tree.root.resize_split(split_id, 0.05);
        tree.root.resize_split(split_id, 0.05);
        tree.root.resize_split(split_id, 0.05);

        if let Node::Split { ratio, .. } = &tree.root {
            assert!((*ratio - 0.65).abs() < 0.01);
        }
    }

    // ---------------------------------------------------------------
    // serialize/deserialize roundtrip
    // ---------------------------------------------------------------

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
    fn test_serialize_roundtrip_complex() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));
        tree.add_window(test_window("win3"));

        let pane_ids = tree.root.pane_ids();
        tree.root.stack_window(pane_ids[0], test_window("tab_on_first"));

        if let Node::Split { ratio, .. } = &mut tree.root {
            *ratio = 0.7;
        }

        let json = tree.root.serialize();
        let restored = Node::deserialize(&json).unwrap();
        assert_eq!(restored.pane_count(), 3);
        assert_eq!(restored.window_count(), 4);

        if let Node::Split { ratio, .. } = &restored {
            assert!((ratio - 0.7).abs() < 0.001);
        }
    }

    #[test]
    fn test_deserialize_invalid_json() {
        let result = Node::deserialize("not valid json {{{");
        assert!(result.is_none());
    }

    #[test]
    fn test_serialize_single_pane() {
        let node = Node::new_pane_with(test_window("solo"));
        let json = node.serialize();
        let restored = Node::deserialize(&json).unwrap();
        assert_eq!(restored.pane_count(), 1);
        assert_eq!(restored.window_count(), 1);
    }

    #[test]
    fn test_serialize_empty_pane() {
        let node = Node::new_pane();
        let json = node.serialize();
        let restored = Node::deserialize(&json).unwrap();
        assert_eq!(restored.pane_count(), 1);
        assert_eq!(restored.window_count(), 0);
    }

    #[test]
    fn test_serialize_preserves_titles() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("Alpha"));
        tree.add_window(test_window("Beta"));

        let json = tree.root.serialize();
        let restored = Node::deserialize(&json).unwrap();
        let titles: Vec<_> = restored.all_windows().iter().map(|w| w.title.clone()).collect();
        assert!(titles.contains(&"Alpha".to_string()));
        assert!(titles.contains(&"Beta".to_string()));
    }

    // ---------------------------------------------------------------
    // cleanup
    // ---------------------------------------------------------------

    #[test]
    fn test_cleanup_removes_empty_panes() {
        let mut node = Node::new_pane_with(test_window("win1"));
        let pane_id = node.id();
        node.split_pane(pane_id, Orientation::Horizontal, 0.5);
        assert_eq!(node.pane_count(), 2);

        node.cleanup();
        assert_eq!(node.pane_count(), 1);
        assert_eq!(node.window_count(), 1);
    }

    #[test]
    fn test_cleanup_collapses_single_child() {
        let mut tree = TileTree::new();
        let w1 = test_window("win1");
        let w1_id = w1.id;
        tree.add_window(w1);
        tree.add_window(test_window("win2"));
        tree.add_window(test_window("win3"));

        assert_eq!(tree.root.pane_count(), 3);
        tree.remove_window(w1_id);
        assert_eq!(tree.root.pane_count(), 2);
    }

    #[test]
    fn test_cleanup_no_effect_on_full_tree() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));
        let count_before = tree.root.pane_count();
        tree.root.cleanup();
        assert_eq!(tree.root.pane_count(), count_before);
    }

    #[test]
    fn test_cleanup_empty_first_child() {
        let mut node = Node::new_pane();
        let pane_id = node.id();
        let (_first, new_id) = node.split_pane(pane_id, Orientation::Horizontal, 0.5).unwrap();
        node.stack_window(new_id, test_window("in_second"));

        assert_eq!(node.pane_count(), 2);
        node.cleanup();
        assert_eq!(node.pane_count(), 1);
        assert_eq!(node.window_count(), 1);
    }

    // ---------------------------------------------------------------
    // Node helpers
    // ---------------------------------------------------------------

    #[test]
    fn test_find_pane_with_window() {
        let mut tree = TileTree::new();
        let w = test_window("findme");
        let wid = w.id;
        tree.add_window(w);
        let found = tree.root.find_pane_with_window(wid);
        assert!(found.is_some());
    }

    #[test]
    fn test_find_pane_with_window_not_found() {
        let tree = TileTree::new();
        let found = tree.root.find_pane_with_window(WindowId::next());
        assert!(found.is_none());
    }

    #[test]
    fn test_find_pane_by_pid() {
        let mut tree = TileTree::new();
        tree.add_window(test_window_with_pid("app1", 42));
        let found = tree.root.find_pane_by_pid(42);
        assert!(found.is_some());
    }

    #[test]
    fn test_find_pane_by_pid_not_found() {
        let tree = TileTree::new();
        assert!(tree.root.find_pane_by_pid(9999).is_none());
    }

    #[test]
    fn test_pane_ids_order() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("a"));
        tree.add_window(test_window("b"));
        tree.add_window(test_window("c"));
        let ids = tree.root.pane_ids();
        assert_eq!(ids.len(), 3);
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j]);
            }
        }
    }

    #[test]
    fn test_all_windows() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("a"));
        tree.add_window(test_window("b"));
        let pane_ids = tree.root.pane_ids();
        tree.root.stack_window(pane_ids[0], test_window("c"));

        let all = tree.root.all_windows();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_default_tree_is_empty_pane() {
        let tree = TileTree::default();
        assert_eq!(tree.root.pane_count(), 1);
        assert_eq!(tree.root.window_count(), 0);
        assert!(tree.focused_pane.is_none());
    }

    #[test]
    fn test_find_node_by_id() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));
        let root_id = tree.root.id();
        assert!(tree.root.find(root_id).is_some());
    }

    #[test]
    fn test_find_mut_node_by_id() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        tree.add_window(test_window("win2"));
        let pane_ids = tree.root.pane_ids();
        assert!(tree.root.find_mut(pane_ids[0]).is_some());
    }

    // ---------------------------------------------------------------
    // cycle_tab (existing tests preserved + extras)
    // ---------------------------------------------------------------

    #[test]
    fn test_cycle_tab() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        let pane_id = tree.focused_pane.unwrap();

        tree.root.stack_window(pane_id, test_window("win2"));
        tree.root.stack_window(pane_id, test_window("win3"));

        if let Some(Node::Pane { active, .. }) = tree.root.find(pane_id) {
            assert_eq!(*active, 2);
        }

        let result = tree.cycle_tab(pane_id, true);
        assert_eq!(result, Some(0));

        let result = tree.cycle_tab(pane_id, true);
        assert_eq!(result, Some(1));

        let result = tree.cycle_tab(pane_id, false);
        assert_eq!(result, Some(0));

        let result = tree.cycle_tab(pane_id, false);
        assert_eq!(result, Some(2));
    }

    #[test]
    fn test_cycle_tab_single_window() {
        let mut tree = TileTree::new();
        tree.add_window(test_window("win1"));
        let pane_id = tree.focused_pane.unwrap();

        assert_eq!(tree.cycle_tab(pane_id, true), None);
        assert_eq!(tree.cycle_tab(pane_id, false), None);
    }

    #[test]
    fn test_cycle_tab_nonexistent_pane() {
        let mut tree = TileTree::new();
        let fake = NodeId::next();
        assert!(tree.cycle_tab(fake, true).is_none());
    }

    // ---------------------------------------------------------------
    // snap_window_beside (existing tests preserved + extras)
    // ---------------------------------------------------------------

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

        assert_eq!(result.x, 900.0);
        assert_eq!(result.y, 50.0);
        assert_eq!(result.width, 500.0);
        assert_eq!(result.height, 600.0);
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

        assert_eq!(result.x, 100.0);
        assert_eq!(result.y, 50.0);
        assert_eq!(result.width, 400.0);
        assert_eq!(result.height, 600.0);
    }

    #[test]
    fn test_snap_window_beside_clamps_to_screen() {
        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let target = Rect::new(100.0, 50.0, 800.0, 600.0);
        let source = Rect::new(0.0, 0.0, 500.0, 400.0);

        let result = TileTree::snap_window_beside(
            target,
            source,
            crate::types::SnapSide::Left,
            screen,
        );
        assert_eq!(result.x, 0.0);
    }

    #[test]
    fn test_snap_window_beside_clamps_width_to_half_screen() {
        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let target = Rect::new(100.0, 50.0, 800.0, 600.0);
        // Source wider than half screen
        let source = Rect::new(0.0, 0.0, 1200.0, 400.0);

        let result = TileTree::snap_window_beside(
            target,
            source,
            crate::types::SnapSide::Right,
            screen,
        );
        // Width should be clamped to half screen = 960
        assert_eq!(result.width, 960.0);
    }
}
