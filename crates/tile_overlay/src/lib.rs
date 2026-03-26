//! Snap zone overlay windows using AppKit via objc2.
//!
//! When a user drags a window near a screen edge or another managed pane,
//! we show a semi-transparent overlay indicating where the window will be placed.

use core_foundation::base::TCFType as _;
use log::debug;
use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSScreen, NSView, NSWindow, NSWindowStyleMask};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};
use tile_core::Rect;

/// Set a CGColorRef property on a CALayer using a typed function pointer cast.
/// We can't use objc2's msg_send! because it rejects CGColorRef (opaque C type).
/// We also can't declare objc_msgSend as variadic — ARM64 uses different calling
/// conventions for variadic vs non-variadic functions.
unsafe fn layer_set_cgcolor(layer: &objc2::runtime::AnyObject, sel: objc2::runtime::Sel, color: core_graphics::sys::CGColorRef) {
    // Cast objc_msgSend to the exact signature CALayer expects: (id, SEL, CGColorRef) -> void
    type SetColorFn = unsafe extern "C" fn(*const objc2::runtime::AnyObject, objc2::runtime::Sel, core_graphics::sys::CGColorRef);
    let msg_send: SetColorFn = std::mem::transmute(objc2::ffi::objc_msgSend as *const ());
    msg_send(layer as *const _, sel, color);
}

/// Set an f64 property on a CALayer.
unsafe fn layer_set_f64(layer: &objc2::runtime::AnyObject, sel: objc2::runtime::Sel, value: f64) {
    type SetF64Fn = unsafe extern "C" fn(*const objc2::runtime::AnyObject, objc2::runtime::Sel, f64);
    let msg_send: SetF64Fn = std::mem::transmute(objc2::ffi::objc_msgSend as *const ());
    msg_send(layer as *const _, sel, value);
}

/// Configuration for overlay appearance.
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    pub color: (f64, f64, f64, f64),         // RGBA
    pub corner_radius: f64,
    pub border_width: f64,
    pub border_color: (f64, f64, f64, f64),  // RGBA
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            color: (0.2, 0.5, 1.0, 0.2),
            border_color: (0.2, 0.5, 1.0, 0.6),
            corner_radius: 10.0,
            border_width: 2.0,
        }
    }
}

/// Manages snap zone overlay windows.
pub struct OverlayManager {
    window: Option<Retained<NSWindow>>,
    config: OverlayConfig,
}

impl OverlayManager {
    pub fn new(config: OverlayConfig) -> Self {
        Self {
            window: None,
            config,
        }
    }

    /// Show the overlay at the given screen-space rectangle (top-left origin).
    pub fn show(&mut self, rect: Rect, mtm: MainThreadMarker) {
        let screen_height = get_main_screen_height(mtm);
        let ns_rect = NSRect::new(
            NSPoint::new(rect.x, screen_height - rect.y - rect.height),
            NSSize::new(rect.width, rect.height),
        );

        let window = self.get_or_create_window(mtm);
        window.setFrame_display(ns_rect, true);

        if let Some(content_view) = window.contentView() {
            content_view.setNeedsDisplay(true);
        }
        window.orderFront(None);
    }

    /// Hide the overlay. Must be called from the main thread (NSEvent global
    /// monitors always deliver on the main thread, so this is safe from drag callbacks).
    pub fn hide(&mut self) {
        if let Some(ref window) = self.window {
            window.orderOut(None);
        }
    }

    /// Check if the overlay is currently visible.
    pub fn is_visible(&self) -> bool {
        self.window
            .as_ref()
            .map(|w| w.isVisible())
            .unwrap_or(false)
    }

    fn get_or_create_window(&mut self, mtm: MainThreadMarker) -> &Retained<NSWindow> {
        if self.window.is_none() {
            let window = create_overlay_window(&self.config, mtm);
            self.window = Some(window);
            debug!("Created overlay window");
        }
        self.window.as_ref().unwrap()
    }
}

impl Drop for OverlayManager {
    fn drop(&mut self) {
        self.hide();
    }
}

/// Create a transparent overlay NSWindow.
fn create_overlay_window(config: &OverlayConfig, mtm: MainThreadMarker) -> Retained<NSWindow> {
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(100.0, 100.0));
    let style = NSWindowStyleMask::Borderless;

    let window = unsafe { NSWindow::initWithContentRect_styleMask_backing_defer(
        mtm.alloc::<NSWindow>(),
        frame,
        style,
        objc2_app_kit::NSBackingStoreType::Buffered,
        false,
    ) };

    // Configure for overlay use
    window.setLevel(1001); // Above screen saver level
    window.setOpaque(false);
    window.setHasShadow(false);
    window.setIgnoresMouseEvents(true);
    window.setCollectionBehavior(
        objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllSpaces
            | objc2_app_kit::NSWindowCollectionBehavior::Stationary,
    );

    let clear = NSColor::clearColor();
    window.setBackgroundColor(Some(&clear));

    // Create a layer-backed content view for the overlay
    let view = create_overlay_view(config, frame, mtm);
    window.setContentView(Some(&view));

    window
}

/// Create an NSView that renders the overlay via CALayer properties.
fn create_overlay_view(config: &OverlayConfig, frame: NSRect, mtm: MainThreadMarker) -> Retained<NSView> {
    let view = NSView::initWithFrame(mtm.alloc::<NSView>(), frame);
    view.setWantsLayer(true);

    if let Some(layer) = view.layer() {
        let bg_color = core_graphics::color::CGColor::rgb(
            config.color.0,
            config.color.1,
            config.color.2,
            config.color.3,
        );
        unsafe {
            layer_set_cgcolor(&layer, objc2::sel!(setBackgroundColor:), bg_color.as_concrete_TypeRef());
            layer_set_f64(&layer, objc2::sel!(setCornerRadius:), config.corner_radius);
        }

        let border_color = core_graphics::color::CGColor::rgb(
            config.border_color.0,
            config.border_color.1,
            config.border_color.2,
            config.border_color.3,
        );
        unsafe {
            layer_set_cgcolor(&layer, objc2::sel!(setBorderColor:), border_color.as_concrete_TypeRef());
            layer_set_f64(&layer, objc2::sel!(setBorderWidth:), config.border_width);
        }
    }

    view
}

/// Get the height of the main screen (for coordinate conversion).
fn get_main_screen_height(mtm: MainThreadMarker) -> f64 {
    let screens = NSScreen::screens(mtm);
    if screens.count() == 0 {
        return 1080.0;
    }
    screens.objectAtIndex(0).frame().size.height
}
