use tile_core::{Rect, SnapSide, WindowInfo};

/// The result of a Ctrl+Cmd drag detection.
#[derive(Debug, Clone)]
pub(crate) enum PendingModDrag {
    /// Snap the dragged window beside the target window.
    SnapBeside {
        target_raw: usize,
        target_frame: Rect,
        side: SnapSide,
    },
    /// Stack the dragged window onto the target window's pane.
    StackOnto { target_raw: usize, target_frame: Rect },
}

/// Find a target window under the cursor for Ctrl+Cmd drag.
/// Returns SnapBeside when the cursor is over or near another window.
pub(crate) fn find_mod_drag_target(
    cursor_x: f64,
    cursor_y: f64,
    windows: &[WindowInfo],
    dragged_window_raw: Option<usize>,
) -> Option<PendingModDrag> {
    for win in windows {
        // Skip the window being dragged.
        if dragged_window_raw == Some(win.ax_ref.raw) {
            continue;
        }
        if win.is_minimized {
            continue;
        }

        let frame = win.frame;
        if frame.contains_point(cursor_x, cursor_y) {
            let rx = (cursor_x - frame.x) / frame.width;
            let ry = (cursor_y - frame.y) / frame.height;
            if rx > 0.25 && rx < 0.75 && ry > 0.25 && ry < 0.75 {
                return Some(PendingModDrag::StackOnto {
                    target_raw: win.ax_ref.raw,
                    target_frame: frame,
                });
            }
            let side = if rx < 0.5 {
                SnapSide::Left
            } else {
                SnapSide::Right
            };
            return Some(PendingModDrag::SnapBeside {
                target_raw: win.ax_ref.raw,
                target_frame: frame,
                side,
            });
        }
    }

    // Check proximity: if no window is directly under cursor, look for nearby ones
    let proximity_threshold = 80.0;
    let mut closest: Option<(f64, &WindowInfo, SnapSide)> = None;

    for win in windows {
        if dragged_window_raw == Some(win.ax_ref.raw) || win.is_minimized {
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
        target_raw: win.ax_ref.raw,
        target_frame: win.frame,
        side,
    })
}
