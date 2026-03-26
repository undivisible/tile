//! Window management via the Accessibility API.

use crate::accessibility::*;
use core_foundation::base::{CFRelease, CFRetain, CFTypeRef, TCFType};
use core_foundation::string::CFString;
use objc2_app_kit::NSWorkspace;
use std::ptr;
use tile_core::{AXWindowRef, AppInfo, Rect, WindowInfo};

/// Get info about the frontmost application.
pub fn get_frontmost_app() -> Option<AppInfo> {
    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;
    let name = app.localizedName()?.to_string();
    let pid = app.processIdentifier();
    let bundle = app.bundleIdentifier().map(|s| s.to_string());
    Some(AppInfo {
        pid,
        name,
        bundle_id: bundle,
    })
}

/// Get the focused window of the frontmost application.
///
/// The returned `CFTypeRef` is an owned reference from `AXUIElementCopyAttributeValue`.
/// The caller **must** call `release_frontmost_window` (or `CFRelease`) on the
/// `CFTypeRef` when it is no longer needed.
pub fn get_frontmost_window() -> Option<(CFTypeRef, AXWindowRef, AppInfo)> {
    let app_info = get_frontmost_app()?;
    unsafe {
        let app_element = AXUIElementCreateApplication(app_info.pid);
        if app_element.is_null() {
            return None;
        }
        let attr = CFString::new(K_AX_FOCUSED_WINDOW_ATTRIBUTE);
        let mut window: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(
            app_element,
            attr.as_concrete_TypeRef() as CFTypeRef,
            &mut window,
        );
        if err != K_AX_ERROR_SUCCESS || window.is_null() {
            CFRelease(app_element);
            return None;
        }
        let ax_ref = AXWindowRef::new(app_info.pid, 0, window as usize);
        CFRelease(app_element);
        Some((window, ax_ref, app_info))
    }
}

/// Release the window element returned by `get_frontmost_window`.
pub fn release_frontmost_window(element: CFTypeRef) {
    if !element.is_null() {
        unsafe {
            CFRelease(element);
        }
    }
}

/// Get the frame of a window via its AX element.
pub fn get_window_frame_raw(element: CFTypeRef) -> Option<Rect> {
    let (x, y) = ax_get_position(element)?;
    let (w, h) = ax_get_size(element)?;
    Some(Rect::new(x, y, w, h))
}

/// Get the frame of a window from its AXWindowRef.
pub fn get_window_frame(ax_ref: &AXWindowRef) -> Option<Rect> {
    unsafe {
        let app_element = AXUIElementCreateApplication(ax_ref.pid);
        if app_element.is_null() {
            return None;
        }
        let windows = get_ax_windows(app_element);
        CFRelease(app_element);

        let result = if ax_ref.window_index < windows.len() {
            get_window_frame_raw(windows[ax_ref.window_index])
        } else {
            None
        };
        release_ax_windows(&windows);
        result
    }
}

/// Set the frame of a window (position + size).
pub fn set_window_frame_raw(element: CFTypeRef, frame: Rect) {
    // Set position first, then size (some apps need this order)
    ax_set_position(element, frame.x, frame.y);
    ax_set_size(element, frame.width, frame.height);
    // Set position again in case the window adjusted
    ax_set_position(element, frame.x, frame.y);
}

/// Set the frame of a window from its AXWindowRef.
pub fn set_window_frame(ax_ref: &AXWindowRef, frame: Rect) {
    unsafe {
        let app_element = AXUIElementCreateApplication(ax_ref.pid);
        if app_element.is_null() {
            return;
        }
        let windows = get_ax_windows(app_element);
        if ax_ref.window_index < windows.len() {
            set_window_frame_raw(windows[ax_ref.window_index], frame);
        }
        release_ax_windows(&windows);
        CFRelease(app_element);
    }
}

/// Focus (raise) a window.
pub fn focus_window(ax_ref: &AXWindowRef) {
    unsafe {
        let app_element = AXUIElementCreateApplication(ax_ref.pid);
        if app_element.is_null() {
            return;
        }
        let windows = get_ax_windows(app_element);
        if ax_ref.window_index < windows.len() {
            ax_perform_action(windows[ax_ref.window_index], "AXRaise");
        }
        release_ax_windows(&windows);
        CFRelease(app_element);

        // Also activate the application via NSRunningApplication
        let workspace = NSWorkspace::sharedWorkspace();
        let apps = workspace.runningApplications();
        for app in apps.iter() {
            if app.processIdentifier() == ax_ref.pid {
                let _ = app.activateWithOptions(
                    objc2_app_kit::NSApplicationActivationOptions::empty(),
                );
                break;
            }
        }
    }
}

/// List all visible windows across all applications.
pub fn list_visible_windows() -> Vec<WindowInfo> {
    let mut result = Vec::new();
    unsafe {
        let workspace = NSWorkspace::sharedWorkspace();
        let apps = workspace.runningApplications();

        for app in apps.iter() {
            // Skip background-only apps
            if app.activationPolicy()
                == objc2_app_kit::NSApplicationActivationPolicy::Prohibited
            {
                continue;
            }

            let pid = app.processIdentifier();
            let app_name = app
                .localizedName()
                .map(|s| s.to_string())
                .unwrap_or_default();

            if app_name.is_empty() {
                continue;
            }

            let app_element = AXUIElementCreateApplication(pid);
            if app_element.is_null() {
                continue;
            }

            let windows = get_ax_windows(app_element);
            for (idx, &win_element) in windows.iter().enumerate() {
                let role = ax_get_string_attribute(win_element, K_AX_ROLE_ATTRIBUTE);
                if role.as_deref() != Some(K_AX_WINDOW_ROLE) {
                    continue;
                }

                let subrole = ax_get_string_attribute(win_element, K_AX_SUBROLE_ATTRIBUTE);
                if subrole.as_deref() != Some(K_AX_STANDARD_WINDOW_SUBROLE) {
                    continue;
                }

                let is_minimized = ax_get_bool_attribute(win_element, K_AX_MINIMIZED_ATTRIBUTE)
                    .unwrap_or(false);

                let title = ax_get_string_attribute(win_element, K_AX_TITLE_ATTRIBUTE)
                    .unwrap_or_default();

                if let Some(frame) = get_window_frame_raw(win_element) {
                    result.push(WindowInfo {
                        pid,
                        title,
                        app_name: app_name.clone(),
                        frame,
                        ax_ref: AXWindowRef::new(pid, idx, win_element as usize),
                        is_minimized,
                    });
                }
            }

            release_ax_windows(&windows);
            CFRelease(app_element);
        }
    }
    result
}

/// Get the AX windows array for an app element. Returns raw CFTypeRef pointers.
unsafe fn get_ax_windows(app_element: CFTypeRef) -> Vec<CFTypeRef> {
    let attr = CFString::new(K_AX_WINDOWS_ATTRIBUTE);
    let mut value: CFTypeRef = ptr::null();
    let err = AXUIElementCopyAttributeValue(
        app_element,
        attr.as_concrete_TypeRef() as CFTypeRef,
        &mut value,
    );
    if err != K_AX_ERROR_SUCCESS || value.is_null() {
        return Vec::new();
    }

    // The value is a CFArray. We need to extract elements manually.
    // CFArrayGetCount + CFArrayGetValueAtIndex
    let count = core_foundation::array::CFArrayGetCount(value as *const _);
    let mut windows = Vec::new();
    for i in 0..count {
        let item = core_foundation::array::CFArrayGetValueAtIndex(value as *const _, i);
        if !item.is_null() {
            CFRetain(item as CFTypeRef);
            windows.push(item as CFTypeRef);
        }
    }
    // Each element has been individually retained, so we can safely release the array.
    CFRelease(value);
    windows
}

/// Release AX window element references returned by `get_ax_windows`.
/// Callers must invoke this when they are done using the window pointers.
pub fn release_ax_windows(windows: &[CFTypeRef]) {
    for &w in windows {
        unsafe {
            CFRelease(w);
        }
    }
}
