use tile_core::{Rect, WindowInfo};

/// Find the nearest visible window to the given frame, excluding the specified PID.
pub(crate) fn find_nearest_window(
    from: Rect,
    windows: &[WindowInfo],
    exclude_pid: i32,
) -> Option<WindowInfo> {
    let (cx, cy) = from.center();
    windows
        .iter()
        .filter(|w| w.pid != exclude_pid && !w.is_minimized)
        .min_by(|a, b| {
            let (ax, ay) = a.frame.center();
            let (bx, by) = b.frame.center();
            let da = (ax - cx).powi(2) + (ay - cy).powi(2);
            let db = (bx - cx).powi(2) + (by - cy).powi(2);
            da.partial_cmp(&db).unwrap()
        })
        .cloned()
}
