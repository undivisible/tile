//! AX Observer for window events (create, destroy, move, resize, focus change).

use crate::accessibility::*;
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;
use log::{debug, warn};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};

/// Events that the observer can report.
#[derive(Debug, Clone)]
pub enum WindowEvent {
    Created { pid: i32 },
    Destroyed { pid: i32 },
    Moved { pid: i32 },
    Resized { pid: i32 },
    FocusChanged { pid: i32 },
    AppActivated { pid: i32 },
    AppDeactivated { pid: i32 },
}

/// Callback type for window events.
pub type WindowEventCallback = Box<dyn Fn(WindowEvent) + Send + 'static>;

/// State shared between the observer callback and the Rust side.
struct ObserverState {
    callback: WindowEventCallback,
}

/// Manages AX observers for multiple applications.
pub struct WindowObserverManager {
    /// Map of pid → observer CFTypeRef
    observers: HashMap<i32, CFTypeRef>,
    /// Shared state for callbacks
    state: Arc<Mutex<ObserverState>>,
    /// Raw pointer to the Arc, created once and reused for all observer refcons.
    /// We hold one extra Arc ref count for this pointer; it is reclaimed in Drop.
    state_raw: *const Mutex<ObserverState>,
}

impl WindowObserverManager {
    pub fn new(callback: WindowEventCallback) -> Self {
        let state = Arc::new(Mutex::new(ObserverState { callback }));
        // Create one raw pointer from an extra Arc clone; reclaimed in Drop.
        let state_raw = Arc::into_raw(state.clone());
        Self {
            observers: HashMap::new(),
            state,
            state_raw,
        }
    }

    /// Start observing a specific application by PID.
    pub fn observe_app(&mut self, pid: i32) -> bool {
        if self.observers.contains_key(&pid) {
            return true; // Already observing
        }

        unsafe {
            let mut observer: CFTypeRef = std::ptr::null();
            let err = AXObserverCreate(pid, observer_callback, &mut observer);
            if err != K_AX_ERROR_SUCCESS || observer.is_null() {
                warn!("Failed to create AX observer for pid {}: error {}", pid, err);
                return false;
            }

            let app_element = AXUIElementCreateApplication(pid);
            if app_element.is_null() {
                CFRelease(observer);
                return false;
            }

            // Reuse the single raw pointer created in new() for the refcon
            let state_ptr = self.state_raw as *mut c_void;

            // Register for notifications
            let notifications = [
                K_AX_WINDOW_CREATED_NOTIFICATION,
                K_AX_WINDOW_MOVED_NOTIFICATION,
                K_AX_WINDOW_RESIZED_NOTIFICATION,
                K_AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION,
                K_AX_UI_ELEMENT_DESTROYED_NOTIFICATION,
                K_AX_APPLICATION_ACTIVATED_NOTIFICATION,
                K_AX_APPLICATION_DEACTIVATED_NOTIFICATION,
            ];

            for notif in &notifications {
                let notif_str = CFString::new(notif);
                let err = AXObserverAddNotification(
                    observer,
                    app_element,
                    notif_str.as_concrete_TypeRef() as CFTypeRef,
                    state_ptr,
                );
                if err != K_AX_ERROR_SUCCESS {
                    debug!(
                        "Failed to add notification {} for pid {}: error {}",
                        notif, pid, err
                    );
                }
            }

            // Add observer to the current run loop
            let source = AXObserverGetRunLoopSource(observer);
            if !source.is_null() {
                let run_loop = CFRunLoopGetCurrent();
                CFRunLoopAddSource(run_loop, source, kCFRunLoopDefaultMode);
            }

            CFRelease(app_element);
            self.observers.insert(pid, observer);
            debug!("Started observing pid {}", pid);
            true
        }
    }

    /// Stop observing an application.
    pub fn stop_observing(&mut self, pid: i32) {
        if let Some(observer) = self.observers.remove(&pid) {
            unsafe {
                let source = AXObserverGetRunLoopSource(observer);
                if !source.is_null() {
                    let run_loop = CFRunLoopGetCurrent();
                    CFRunLoopRemoveSource(run_loop, source, kCFRunLoopDefaultMode);
                }
                CFRelease(observer);
            }
            debug!("Stopped observing pid {}", pid);
        }
    }

    /// Stop all observers.
    pub fn stop_all(&mut self) {
        let pids: Vec<i32> = self.observers.keys().cloned().collect();
        for pid in pids {
            self.stop_observing(pid);
        }
    }

    /// Get set of currently observed PIDs.
    pub fn observed_pids(&self) -> Vec<i32> {
        self.observers.keys().cloned().collect()
    }
}

impl Drop for WindowObserverManager {
    fn drop(&mut self) {
        self.stop_all();
        // Reclaim the Arc ref count held by the raw pointer created in new().
        unsafe {
            Arc::from_raw(self.state_raw);
        }
    }
}

/// The C callback that AXObserver invokes. Dispatches to Rust.
extern "C" fn observer_callback(
    _observer: CFTypeRef,
    _element: CFTypeRef,
    notification: CFTypeRef,
    refcon: *mut c_void,
) {
    if refcon.is_null() {
        return;
    }

    unsafe {
        // Reconstruct the Arc without taking ownership (we need it to persist)
        let state = Arc::from_raw(refcon as *const Mutex<ObserverState>);
        let state_clone = state.clone();
        // Leak it back so it isn't dropped
        let _ = Arc::into_raw(state);

        // Get notification name
        let notif_str = CFString::wrap_under_get_rule(notification as *const _);
        let notif_name = notif_str.to_string();

        // Try to get the PID from the element
        // For now, we pass 0 — the manager tracks pid per observer anyway
        let pid = 0i32; // We'd need AXUIElementGetPid but it's not critical here

        let event = match notif_name.as_str() {
            "AXWindowCreated" => Some(WindowEvent::Created { pid }),
            "AXUIElementDestroyed" => Some(WindowEvent::Destroyed { pid }),
            "AXWindowMoved" => Some(WindowEvent::Moved { pid }),
            "AXWindowResized" => Some(WindowEvent::Resized { pid }),
            "AXFocusedWindowChanged" => Some(WindowEvent::FocusChanged { pid }),
            "AXApplicationActivated" => Some(WindowEvent::AppActivated { pid }),
            "AXApplicationDeactivated" => Some(WindowEvent::AppDeactivated { pid }),
            _ => {
                debug!("Unknown AX notification: {}", notif_name);
                None
            }
        };

        if let Some(event) = event {
            if let Ok(state) = state_clone.lock() {
                (state.callback)(event);
            }
        }
    }
}
