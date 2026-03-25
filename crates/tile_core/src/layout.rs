use crate::types::*;

/// Snap zone detection for drag-to-tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapZone {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    TopLeftQuarter,
    TopRightQuarter,
    BottomLeftQuarter,
    BottomRightQuarter,
    Maximize,
    /// Split an existing pane on a given side.
    SplitLeft(NodeId),
    SplitRight(NodeId),
    SplitTop(NodeId),
    SplitBottom(NodeId),
    /// Stack onto an existing pane (center of pane).
    Stack(NodeId),
}

/// Threshold in pixels for edge detection.
const EDGE_THRESHOLD: f64 = 40.0;
/// Corner detection radius.
const CORNER_THRESHOLD: f64 = 80.0;

/// Detect which snap zone the cursor is in, given screen bounds.
pub fn detect_snap_zone(
    cursor_x: f64,
    cursor_y: f64,
    screen: Rect,
    pane_rects: &[(NodeId, Rect)],
) -> Option<SnapZone> {
    // First check if cursor is near a managed pane
    for (pane_id, rect) in pane_rects {
        if rect.contains_point(cursor_x, cursor_y) {
            let rx = (cursor_x - rect.x) / rect.width;
            let ry = (cursor_y - rect.y) / rect.height;

            // Center region → stack
            if rx > 0.25 && rx < 0.75 && ry > 0.25 && ry < 0.75 {
                return Some(SnapZone::Stack(*pane_id));
            }
            // Edge regions → split
            if rx < 0.25 {
                return Some(SnapZone::SplitLeft(*pane_id));
            }
            if rx > 0.75 {
                return Some(SnapZone::SplitRight(*pane_id));
            }
            if ry < 0.25 {
                return Some(SnapZone::SplitTop(*pane_id));
            }
            if ry > 0.75 {
                return Some(SnapZone::SplitBottom(*pane_id));
            }
        }
    }

    // Screen edge detection
    let near_left = cursor_x - screen.x < EDGE_THRESHOLD;
    let near_right = (screen.x + screen.width) - cursor_x < EDGE_THRESHOLD;
    let near_top = cursor_y - screen.y < EDGE_THRESHOLD;
    let near_bottom = (screen.y + screen.height) - cursor_y < EDGE_THRESHOLD;

    // Corners
    let near_left_edge = cursor_x - screen.x < CORNER_THRESHOLD;
    let near_right_edge = (screen.x + screen.width) - cursor_x < CORNER_THRESHOLD;
    let _near_top_edge = cursor_y - screen.y < CORNER_THRESHOLD;
    let _near_bottom_edge = (screen.y + screen.height) - cursor_y < CORNER_THRESHOLD;

    if near_top && near_left_edge {
        return Some(SnapZone::TopLeftQuarter);
    }
    if near_top && near_right_edge {
        return Some(SnapZone::TopRightQuarter);
    }
    if near_bottom && near_left_edge {
        return Some(SnapZone::BottomLeftQuarter);
    }
    if near_bottom && near_right_edge {
        return Some(SnapZone::BottomRightQuarter);
    }

    // Top center → maximize
    if near_top {
        return Some(SnapZone::Maximize);
    }

    // Edges
    if near_left {
        return Some(SnapZone::LeftHalf);
    }
    if near_right {
        return Some(SnapZone::RightHalf);
    }
    if near_bottom {
        return Some(SnapZone::BottomHalf);
    }

    None
}

/// Compute the overlay rectangle for a snap zone.
pub fn snap_zone_rect(zone: &SnapZone, screen: Rect, pane_rects: &[(NodeId, Rect)]) -> Rect {
    let x = screen.x;
    let y = screen.y;
    let w = screen.width;
    let h = screen.height;

    match zone {
        SnapZone::LeftHalf => Rect::new(x, y, w / 2.0, h),
        SnapZone::RightHalf => Rect::new(x + w / 2.0, y, w / 2.0, h),
        SnapZone::TopHalf => Rect::new(x, y, w, h / 2.0),
        SnapZone::BottomHalf => Rect::new(x, y + h / 2.0, w, h / 2.0),
        SnapZone::TopLeftQuarter => Rect::new(x, y, w / 2.0, h / 2.0),
        SnapZone::TopRightQuarter => Rect::new(x + w / 2.0, y, w / 2.0, h / 2.0),
        SnapZone::BottomLeftQuarter => Rect::new(x, y + h / 2.0, w / 2.0, h / 2.0),
        SnapZone::BottomRightQuarter => Rect::new(x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0),
        SnapZone::Maximize => screen,
        SnapZone::SplitLeft(pane_id) => {
            if let Some((_, r)) = pane_rects.iter().find(|(id, _)| id == pane_id) {
                Rect::new(r.x, r.y, r.width / 2.0, r.height)
            } else {
                Rect::zero()
            }
        }
        SnapZone::SplitRight(pane_id) => {
            if let Some((_, r)) = pane_rects.iter().find(|(id, _)| id == pane_id) {
                Rect::new(r.x + r.width / 2.0, r.y, r.width / 2.0, r.height)
            } else {
                Rect::zero()
            }
        }
        SnapZone::SplitTop(pane_id) => {
            if let Some((_, r)) = pane_rects.iter().find(|(id, _)| id == pane_id) {
                Rect::new(r.x, r.y, r.width, r.height / 2.0)
            } else {
                Rect::zero()
            }
        }
        SnapZone::SplitBottom(pane_id) => {
            if let Some((_, r)) = pane_rects.iter().find(|(id, _)| id == pane_id) {
                Rect::new(r.x, r.y + r.height / 2.0, r.width, r.height / 2.0)
            } else {
                Rect::zero()
            }
        }
        SnapZone::Stack(pane_id) => {
            if let Some((_, r)) = pane_rects.iter().find(|(id, _)| id == pane_id) {
                // Show a slightly inset overlay for stack
                r.inset(20.0)
            } else {
                Rect::zero()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_zone_edges() {
        let screen = Rect::new(0.0, 25.0, 1920.0, 1055.0);

        // Left edge
        assert_eq!(
            detect_snap_zone(5.0, 500.0, screen, &[]),
            Some(SnapZone::LeftHalf)
        );

        // Right edge
        assert_eq!(
            detect_snap_zone(1915.0, 500.0, screen, &[]),
            Some(SnapZone::RightHalf)
        );

        // Top center → maximize
        assert_eq!(
            detect_snap_zone(960.0, 30.0, screen, &[]),
            Some(SnapZone::Maximize)
        );

        // Top-left corner
        assert_eq!(
            detect_snap_zone(5.0, 30.0, screen, &[]),
            Some(SnapZone::TopLeftQuarter)
        );
    }

    #[test]
    fn test_snap_zone_pane() {
        let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let pane_id = NodeId::next();
        let panes = vec![(pane_id, Rect::new(0.0, 0.0, 960.0, 1080.0))];

        // Center of pane → stack
        assert_eq!(
            detect_snap_zone(480.0, 540.0, screen, &panes),
            Some(SnapZone::Stack(pane_id))
        );

        // Left edge of pane → split left
        assert_eq!(
            detect_snap_zone(50.0, 540.0, screen, &panes),
            Some(SnapZone::SplitLeft(pane_id))
        );
    }
}
