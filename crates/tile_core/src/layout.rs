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

    fn screen() -> Rect {
        Rect::new(0.0, 25.0, 1920.0, 1055.0)
    }

    // ---------------------------------------------------------------
    // detect_snap_zone: screen edges
    // ---------------------------------------------------------------

    #[test]
    fn test_snap_zone_left_edge() {
        assert_eq!(
            detect_snap_zone(5.0, 500.0, screen(), &[]),
            Some(SnapZone::LeftHalf)
        );
    }

    #[test]
    fn test_snap_zone_right_edge() {
        assert_eq!(
            detect_snap_zone(1915.0, 500.0, screen(), &[]),
            Some(SnapZone::RightHalf)
        );
    }

    #[test]
    fn test_snap_zone_top_center_maximize() {
        assert_eq!(
            detect_snap_zone(960.0, 30.0, screen(), &[]),
            Some(SnapZone::Maximize)
        );
    }

    #[test]
    fn test_snap_zone_bottom_edge() {
        let s = screen();
        assert_eq!(
            detect_snap_zone(960.0, s.y + s.height - 5.0, s, &[]),
            Some(SnapZone::BottomHalf)
        );
    }

    // ---------------------------------------------------------------
    // detect_snap_zone: all 4 corners
    // ---------------------------------------------------------------

    #[test]
    fn test_snap_zone_top_left_corner() {
        assert_eq!(
            detect_snap_zone(5.0, 30.0, screen(), &[]),
            Some(SnapZone::TopLeftQuarter)
        );
    }

    #[test]
    fn test_snap_zone_top_right_corner() {
        assert_eq!(
            detect_snap_zone(1915.0, 30.0, screen(), &[]),
            Some(SnapZone::TopRightQuarter)
        );
    }

    #[test]
    fn test_snap_zone_bottom_left_corner() {
        let s = screen();
        assert_eq!(
            detect_snap_zone(5.0, s.y + s.height - 5.0, s, &[]),
            Some(SnapZone::BottomLeftQuarter)
        );
    }

    #[test]
    fn test_snap_zone_bottom_right_corner() {
        let s = screen();
        assert_eq!(
            detect_snap_zone(1915.0, s.y + s.height - 5.0, s, &[]),
            Some(SnapZone::BottomRightQuarter)
        );
    }

    // ---------------------------------------------------------------
    // detect_snap_zone: pane-based zones
    // ---------------------------------------------------------------

    #[test]
    fn test_snap_zone_pane_stack() {
        let pane_id = NodeId::next();
        let panes = vec![(pane_id, Rect::new(0.0, 0.0, 960.0, 1080.0))];

        // Center of pane -> stack
        assert_eq!(
            detect_snap_zone(480.0, 540.0, screen(), &panes),
            Some(SnapZone::Stack(pane_id))
        );
    }

    #[test]
    fn test_snap_zone_pane_split_left() {
        let pane_id = NodeId::next();
        let panes = vec![(pane_id, Rect::new(0.0, 0.0, 960.0, 1080.0))];

        // Left edge of pane (< 0.25 of width)
        assert_eq!(
            detect_snap_zone(50.0, 540.0, screen(), &panes),
            Some(SnapZone::SplitLeft(pane_id))
        );
    }

    #[test]
    fn test_snap_zone_pane_split_right() {
        let pane_id = NodeId::next();
        let panes = vec![(pane_id, Rect::new(0.0, 0.0, 960.0, 1080.0))];

        // Right edge of pane (> 0.75 of width)
        assert_eq!(
            detect_snap_zone(900.0, 540.0, screen(), &panes),
            Some(SnapZone::SplitRight(pane_id))
        );
    }

    #[test]
    fn test_snap_zone_pane_split_top() {
        let pane_id = NodeId::next();
        let panes = vec![(pane_id, Rect::new(200.0, 200.0, 960.0, 800.0))];

        // Top edge of pane (< 0.25 of height), but horizontally in center
        assert_eq!(
            detect_snap_zone(680.0, 250.0, screen(), &panes),
            Some(SnapZone::SplitTop(pane_id))
        );
    }

    #[test]
    fn test_snap_zone_pane_split_bottom() {
        let pane_id = NodeId::next();
        let panes = vec![(pane_id, Rect::new(200.0, 200.0, 960.0, 800.0))];

        // Bottom edge of pane (> 0.75 of height), horizontally in center
        assert_eq!(
            detect_snap_zone(680.0, 950.0, screen(), &panes),
            Some(SnapZone::SplitBottom(pane_id))
        );
    }

    // ---------------------------------------------------------------
    // detect_snap_zone: no zone (middle of screen with no panes)
    // ---------------------------------------------------------------

    #[test]
    fn test_snap_zone_none_middle_of_screen() {
        // Well away from all edges, no panes
        assert_eq!(
            detect_snap_zone(960.0, 540.0, screen(), &[]),
            None
        );
    }

    #[test]
    fn test_snap_zone_none_just_outside_threshold() {
        let s = screen();
        // Just outside the 40px edge threshold
        assert_eq!(
            detect_snap_zone(s.x + 45.0, 540.0, s, &[]),
            None
        );
    }

    // ---------------------------------------------------------------
    // detect_snap_zone: multiple panes
    // ---------------------------------------------------------------

    #[test]
    fn test_snap_zone_with_multiple_panes() {
        let pane1 = NodeId::next();
        let pane2 = NodeId::next();
        let panes = vec![
            (pane1, Rect::new(0.0, 0.0, 960.0, 1080.0)),
            (pane2, Rect::new(960.0, 0.0, 960.0, 1080.0)),
        ];

        // Center of first pane
        assert_eq!(
            detect_snap_zone(480.0, 540.0, screen(), &panes),
            Some(SnapZone::Stack(pane1))
        );

        // Center of second pane
        assert_eq!(
            detect_snap_zone(1440.0, 540.0, screen(), &panes),
            Some(SnapZone::Stack(pane2))
        );
    }

    #[test]
    fn test_snap_zone_pane_takes_priority_over_screen_edge() {
        let pane_id = NodeId::next();
        // Pane sits at the very left edge of screen
        let panes = vec![(pane_id, Rect::new(0.0, 0.0, 960.0, 1080.0))];

        // At left edge of screen, but inside a pane -> pane zone takes priority
        let zone = detect_snap_zone(5.0, 540.0, screen(), &panes);
        assert_eq!(zone, Some(SnapZone::SplitLeft(pane_id)));
    }

    // ---------------------------------------------------------------
    // snap_zone_rect tests
    // ---------------------------------------------------------------

    #[test]
    fn test_snap_zone_rect_left_half() {
        let s = screen();
        let r = snap_zone_rect(&SnapZone::LeftHalf, s, &[]);
        assert_eq!(r.x, s.x);
        assert_eq!(r.y, s.y);
        assert_eq!(r.width, s.width / 2.0);
        assert_eq!(r.height, s.height);
    }

    #[test]
    fn test_snap_zone_rect_right_half() {
        let s = screen();
        let r = snap_zone_rect(&SnapZone::RightHalf, s, &[]);
        assert_eq!(r.x, s.x + s.width / 2.0);
        assert_eq!(r.width, s.width / 2.0);
    }

    #[test]
    fn test_snap_zone_rect_top_half() {
        let s = screen();
        let r = snap_zone_rect(&SnapZone::TopHalf, s, &[]);
        assert_eq!(r.y, s.y);
        assert_eq!(r.height, s.height / 2.0);
        assert_eq!(r.width, s.width);
    }

    #[test]
    fn test_snap_zone_rect_bottom_half() {
        let s = screen();
        let r = snap_zone_rect(&SnapZone::BottomHalf, s, &[]);
        assert_eq!(r.y, s.y + s.height / 2.0);
        assert_eq!(r.height, s.height / 2.0);
    }

    #[test]
    fn test_snap_zone_rect_quarters() {
        let s = screen();
        let hw = s.width / 2.0;
        let hh = s.height / 2.0;

        let tl = snap_zone_rect(&SnapZone::TopLeftQuarter, s, &[]);
        assert_eq!(tl, Rect::new(s.x, s.y, hw, hh));

        let tr = snap_zone_rect(&SnapZone::TopRightQuarter, s, &[]);
        assert_eq!(tr, Rect::new(s.x + hw, s.y, hw, hh));

        let bl = snap_zone_rect(&SnapZone::BottomLeftQuarter, s, &[]);
        assert_eq!(bl, Rect::new(s.x, s.y + hh, hw, hh));

        let br = snap_zone_rect(&SnapZone::BottomRightQuarter, s, &[]);
        assert_eq!(br, Rect::new(s.x + hw, s.y + hh, hw, hh));
    }

    #[test]
    fn test_snap_zone_rect_maximize() {
        let s = screen();
        let r = snap_zone_rect(&SnapZone::Maximize, s, &[]);
        assert_eq!(r, s);
    }

    #[test]
    fn test_snap_zone_rect_split_left() {
        let pane_id = NodeId::next();
        let pane_rect = Rect::new(100.0, 100.0, 800.0, 600.0);
        let panes = vec![(pane_id, pane_rect)];

        let r = snap_zone_rect(&SnapZone::SplitLeft(pane_id), screen(), &panes);
        assert_eq!(r.x, 100.0);
        assert_eq!(r.y, 100.0);
        assert_eq!(r.width, 400.0);
        assert_eq!(r.height, 600.0);
    }

    #[test]
    fn test_snap_zone_rect_split_right() {
        let pane_id = NodeId::next();
        let pane_rect = Rect::new(100.0, 100.0, 800.0, 600.0);
        let panes = vec![(pane_id, pane_rect)];

        let r = snap_zone_rect(&SnapZone::SplitRight(pane_id), screen(), &panes);
        assert_eq!(r.x, 500.0); // 100 + 800/2
        assert_eq!(r.width, 400.0);
    }

    #[test]
    fn test_snap_zone_rect_split_top() {
        let pane_id = NodeId::next();
        let pane_rect = Rect::new(100.0, 100.0, 800.0, 600.0);
        let panes = vec![(pane_id, pane_rect)];

        let r = snap_zone_rect(&SnapZone::SplitTop(pane_id), screen(), &panes);
        assert_eq!(r.y, 100.0);
        assert_eq!(r.height, 300.0);
        assert_eq!(r.width, 800.0);
    }

    #[test]
    fn test_snap_zone_rect_split_bottom() {
        let pane_id = NodeId::next();
        let pane_rect = Rect::new(100.0, 100.0, 800.0, 600.0);
        let panes = vec![(pane_id, pane_rect)];

        let r = snap_zone_rect(&SnapZone::SplitBottom(pane_id), screen(), &panes);
        assert_eq!(r.y, 400.0); // 100 + 600/2
        assert_eq!(r.height, 300.0);
    }

    #[test]
    fn test_snap_zone_rect_stack() {
        let pane_id = NodeId::next();
        let pane_rect = Rect::new(100.0, 100.0, 800.0, 600.0);
        let panes = vec![(pane_id, pane_rect)];

        let r = snap_zone_rect(&SnapZone::Stack(pane_id), screen(), &panes);
        // Should be inset by 20
        assert_eq!(r.x, 120.0);
        assert_eq!(r.y, 120.0);
        assert_eq!(r.width, 760.0);
        assert_eq!(r.height, 560.0);
    }

    #[test]
    fn test_snap_zone_rect_missing_pane_returns_zero() {
        let fake_id = NodeId::next();
        let r = snap_zone_rect(&SnapZone::SplitLeft(fake_id), screen(), &[]);
        assert_eq!(r, Rect::zero());
    }
}
