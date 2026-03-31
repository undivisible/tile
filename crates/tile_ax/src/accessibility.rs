//! Raw Accessibility API FFI bindings.
//!
//! The objc2 ecosystem does not yet wrap ApplicationServices / HIServices,
//! so we link directly against the framework and declare the C functions.

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use std::ptr;

// AXError codes
pub const K_AX_ERROR_SUCCESS: i32 = 0;
pub const K_AX_ERROR_FAILURE: i32 = -25200;
pub const K_AX_ERROR_ATTRIBUTE_UNSUPPORTED: i32 = -25205;
pub const K_AX_ERROR_NO_VALUE: i32 = -25212;

// AX notification constants
pub const K_AX_WINDOW_CREATED_NOTIFICATION: &str = "AXWindowCreated";
pub const K_AX_WINDOW_MOVED_NOTIFICATION: &str = "AXWindowMoved";
pub const K_AX_WINDOW_RESIZED_NOTIFICATION: &str = "AXWindowResized";
pub const K_AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION: &str = "AXFocusedWindowChanged";
pub const K_AX_UI_ELEMENT_DESTROYED_NOTIFICATION: &str = "AXUIElementDestroyed";
pub const K_AX_APPLICATION_ACTIVATED_NOTIFICATION: &str = "AXApplicationActivated";
pub const K_AX_APPLICATION_DEACTIVATED_NOTIFICATION: &str = "AXApplicationDeactivated";

// AX attribute constants
pub const K_AX_WINDOWS_ATTRIBUTE: &str = "AXWindows";
pub const K_AX_FOCUSED_WINDOW_ATTRIBUTE: &str = "AXFocusedWindow";
pub const K_AX_POSITION_ATTRIBUTE: &str = "AXPosition";
pub const K_AX_SIZE_ATTRIBUTE: &str = "AXSize";
pub const K_AX_TITLE_ATTRIBUTE: &str = "AXTitle";
pub const K_AX_ROLE_ATTRIBUTE: &str = "AXRole";
pub const K_AX_SUBROLE_ATTRIBUTE: &str = "AXSubrole";
pub const K_AX_MINIMIZED_ATTRIBUTE: &str = "AXMinimized";
pub const K_AX_MAIN_ATTRIBUTE: &str = "AXMain";
pub const K_AX_FOCUSED_ATTRIBUTE: &str = "AXFocused";

// AX role values
pub const K_AX_WINDOW_ROLE: &str = "AXWindow";
pub const K_AX_STANDARD_WINDOW_SUBROLE: &str = "AXStandardWindow";

// Accessibility trust prompt key
pub const K_AX_TRUSTED_CHECK_OPTION_PROMPT: &str =
    "AXTrustedCheckOptionPrompt";

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    pub fn AXUIElementCreateSystemWide() -> CFTypeRef;
    pub fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef, // CFStringRef
        value: *mut CFTypeRef,
    ) -> i32;
    pub fn AXUIElementSetAttributeValue(
        element: CFTypeRef,
        attribute: CFTypeRef, // CFStringRef
        value: CFTypeRef,
    ) -> i32;
    pub fn AXUIElementPerformAction(element: CFTypeRef, action: CFTypeRef) -> i32;
    pub fn AXIsProcessTrusted() -> bool;
    pub fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> bool;
    pub fn AXValueCreate(value_type: u32, value: *const std::ffi::c_void) -> CFTypeRef;
    pub fn AXValueGetValue(
        value: CFTypeRef,
        value_type: u32,
        value_out: *mut std::ffi::c_void,
    ) -> bool;
    pub fn AXObserverCreate(
        application: i32,
        callback: AXObserverCallback,
        observer: *mut CFTypeRef,
    ) -> i32;
    pub fn AXObserverAddNotification(
        observer: CFTypeRef,
        element: CFTypeRef,
        notification: CFTypeRef,
        refcon: *mut std::ffi::c_void,
    ) -> i32;
    pub fn AXObserverRemoveNotification(
        observer: CFTypeRef,
        element: CFTypeRef,
        notification: CFTypeRef,
    ) -> i32;
    pub fn AXObserverGetRunLoopSource(observer: CFTypeRef) -> CFTypeRef;
}

// AXValue types
pub const K_AX_VALUE_TYPE_CGPOINT: u32 = 1;
pub const K_AX_VALUE_TYPE_CGSIZE: u32 = 2;
pub const K_AX_VALUE_TYPE_CGRECT: u32 = 3;

/// AXObserver callback type.
pub type AXObserverCallback = extern "C" fn(
    observer: CFTypeRef,
    element: CFTypeRef,
    notification: CFTypeRef,
    refcon: *mut std::ffi::c_void,
);

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub fn CFRunLoopGetCurrent() -> CFTypeRef;
    pub fn CFRunLoopAddSource(rl: CFTypeRef, source: CFTypeRef, mode: CFTypeRef);
    pub fn CFRunLoopRemoveSource(rl: CFTypeRef, source: CFTypeRef, mode: CFTypeRef);
}

// CFRunLoop mode
extern "C" {
    pub static kCFRunLoopDefaultMode: CFTypeRef;
}

/// Check if the current process has accessibility permissions.
pub fn check_accessibility_permission() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Request accessibility permissions with a system prompt dialog.
pub fn request_accessibility_permission() -> bool {
    let key = CFString::new(K_AX_TRUSTED_CHECK_OPTION_PROMPT);
    let dict = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
    unsafe { AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef() as CFTypeRef) }
}

/// Helper: get a string attribute from an AX element.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn ax_get_string_attribute(element: CFTypeRef, attribute: &str) -> Option<String> {
    unsafe {
        let attr = CFString::new(attribute);
        let mut value: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(
            element,
            attr.as_concrete_TypeRef() as CFTypeRef,
            &mut value,
        );
        if err != K_AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }
        // Try to interpret as CFString
        let cf_str = CFString::wrap_under_create_rule(value as *const _);
        Some(cf_str.to_string())
    }
}

/// Helper: get a boolean attribute from an AX element.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn ax_get_bool_attribute(element: CFTypeRef, attribute: &str) -> Option<bool> {
    unsafe {
        let attr = CFString::new(attribute);
        let mut value: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(
            element,
            attr.as_concrete_TypeRef() as CFTypeRef,
            &mut value,
        );
        if err != K_AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }
        let cf_bool = CFBoolean::wrap_under_create_rule(value as *const _);
        Some(cf_bool == CFBoolean::true_value())
    }
}

/// Helper: get the position (CGPoint) of an AX element.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn ax_get_position(element: CFTypeRef) -> Option<(f64, f64)> {
    unsafe {
        let attr = CFString::new(K_AX_POSITION_ATTRIBUTE);
        let mut value: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(
            element,
            attr.as_concrete_TypeRef() as CFTypeRef,
            &mut value,
        );
        if err != K_AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }
        let mut point = core_graphics::geometry::CGPoint::new(0.0, 0.0);
        let ok = AXValueGetValue(
            value,
            K_AX_VALUE_TYPE_CGPOINT,
            &mut point as *mut _ as *mut std::ffi::c_void,
        );
        CFRelease(value);
        if ok {
            Some((point.x, point.y))
        } else {
            None
        }
    }
}

/// Helper: get the size (CGSize) of an AX element.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn ax_get_size(element: CFTypeRef) -> Option<(f64, f64)> {
    unsafe {
        let attr = CFString::new(K_AX_SIZE_ATTRIBUTE);
        let mut value: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(
            element,
            attr.as_concrete_TypeRef() as CFTypeRef,
            &mut value,
        );
        if err != K_AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }
        let mut size = core_graphics::geometry::CGSize::new(0.0, 0.0);
        let ok = AXValueGetValue(
            value,
            K_AX_VALUE_TYPE_CGSIZE,
            &mut size as *mut _ as *mut std::ffi::c_void,
        );
        CFRelease(value);
        if ok {
            Some((size.width, size.height))
        } else {
            None
        }
    }
}

/// Helper: set the position of an AX element.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn ax_set_position(element: CFTypeRef, x: f64, y: f64) -> bool {
    unsafe {
        let point = core_graphics::geometry::CGPoint::new(x, y);
        let value = AXValueCreate(
            K_AX_VALUE_TYPE_CGPOINT,
            &point as *const _ as *const std::ffi::c_void,
        );
        if value.is_null() {
            return false;
        }
        let attr = CFString::new(K_AX_POSITION_ATTRIBUTE);
        let err =
            AXUIElementSetAttributeValue(element, attr.as_concrete_TypeRef() as CFTypeRef, value);
        CFRelease(value);
        err == K_AX_ERROR_SUCCESS
    }
}

/// Helper: set the size of an AX element.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn ax_set_size(element: CFTypeRef, w: f64, h: f64) -> bool {
    unsafe {
        let size = core_graphics::geometry::CGSize::new(w, h);
        let value = AXValueCreate(
            K_AX_VALUE_TYPE_CGSIZE,
            &size as *const _ as *const std::ffi::c_void,
        );
        if value.is_null() {
            return false;
        }
        let attr = CFString::new(K_AX_SIZE_ATTRIBUTE);
        let err =
            AXUIElementSetAttributeValue(element, attr.as_concrete_TypeRef() as CFTypeRef, value);
        CFRelease(value);
        err == K_AX_ERROR_SUCCESS
    }
}

/// Perform an action on an AX element.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn ax_perform_action(element: CFTypeRef, action: &str) -> bool {
    unsafe {
        let action_str = CFString::new(action);
        let err =
            AXUIElementPerformAction(element, action_str.as_concrete_TypeRef() as CFTypeRef);
        err == K_AX_ERROR_SUCCESS
    }
}
