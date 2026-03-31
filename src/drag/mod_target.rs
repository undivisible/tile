use tile_core::{Rect, SnapSide, WindowInfo};

/// The result of an Opt+Ctrl drag detection.
#[derive(Debug, Clone)]
pub(crate) enum PendingModDrag {
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

/// Find a target window under the cursor for Opt+Ctrl drag.
/// Returns SnapBeside if cursor is at the edge, StackOnto if at center.
pub(crate) fn find_mod_drag_target(
    cursor_x: f64,
    cursor_y: f64,
    windows: &[WindowInfo],
) -> Option<PendingModDrag> {
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
                return Some(PendingModDrag::StackOnto {
                    target_frame: frame,
                });
            }

            // Left edge → snap to left of target
            if rx < 0.25 {
                return Some(PendingModDrag::SnapBeside {
                    target_frame: frame,
                    side: SnapSide::Left,
                });
            }
            // Right edge → snap to right of target
            if rx > 0.75 {
                return Some(PendingModDrag::SnapBeside {
                    target_frame: frame,
                    side: SnapSide::Right,
                });
            }

            // Top/bottom edges also snap beside (default to right)
            return Some(PendingModDrag::SnapBeside {
                target_frame: frame,
                side: SnapSide::Right,
            });
        }
    }

    // Check proximity: if no window is directly under cursor, look for nearby ones
    let proximity_threshold = 80.0;
    let mut closest: Option<(f64, &WindowInfo, SnapSide)> = None;

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

        if dist < proximity_threshold && (closest.is_none() || dist < closest.unwrap().0) {
            closest = Some((dist, win, side));
        }
    }

    closest.map(|(_, win, side)| PendingModDrag::SnapBeside {
        target_frame: win.frame,
        side,
    })
}
