//! Screen information via objc2-app-kit.

use objc2_app_kit::NSScreen;
use objc2_foundation::MainThreadMarker;
use tile_core::Rect;

/// Get the usable frame of a screen (respects menu bar and dock).
pub fn get_usable_screen_frame(screen_idx: usize) -> Option<Rect> {
    let mtm = MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    if screen_idx >= screens.count() {
        return None;
    }
    let screen = screens.objectAtIndex(screen_idx);
    let frame = screen.visibleFrame();
    let full = screen.frame();
    // NSScreen uses bottom-left origin; convert to top-left
    let y = full.size.height - frame.origin.y - frame.size.height;
    Some(Rect::new(
        frame.origin.x,
        y,
        frame.size.width,
        frame.size.height,
    ))
}

/// Get the full frame of a screen.
pub fn get_full_screen_frame(screen_idx: usize) -> Option<Rect> {
    let mtm = MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    if screen_idx >= screens.count() {
        return None;
    }
    let screen = screens.objectAtIndex(screen_idx);
    let frame = screen.frame();
    Some(Rect::new(
        frame.origin.x,
        frame.origin.y,
        frame.size.width,
        frame.size.height,
    ))
}

/// Get the number of connected screens.
pub fn screen_count() -> usize {
    let mtm = match MainThreadMarker::new() {
        Some(m) => m,
        None => return 0,
    };
    NSScreen::screens(mtm).count()
}

/// Get usable frames for all screens.
pub fn all_usable_frames() -> Vec<Rect> {
    (0..screen_count())
        .filter_map(get_usable_screen_frame)
        .collect()
}
