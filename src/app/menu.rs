use objc2::sel;
use objc2_app_kit::{NSMenu, NSMenuItem, NSStatusBar};
use objc2_foundation::{ns_string, MainThreadMarker, NSString};

/// Set up the status bar menu.
pub(crate) fn setup_status_bar(mtm: MainThreadMarker) {
    let status_bar = NSStatusBar::systemStatusBar();
    let item = status_bar.statusItemWithLength(-1.0); // NSVariableStatusItemLength

    if let Some(button) = item.button(mtm) {
        button.setTitle(ns_string!("\u{229e}")); // ⊞ symbol
    }

    let menu = NSMenu::new(mtm);

    // About item
    let about_title = NSString::from_str("About Tile");
    let about_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &about_title,
            None,
            ns_string!(""),
        )
    };
    menu.addItem(&about_item);

    // Settings item — launches tile-settings binary
    let settings_title = NSString::from_str("Settings...");
    let settings_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &settings_title,
            None,
            ns_string!(","),
        )
    };
    menu.addItem(&settings_item);

    // Separator
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Quit item
    let quit_title = NSString::from_str("Quit Tile");
    let quit_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &quit_title,
            Some(sel!(terminate:)),
            ns_string!("q"),
        )
    };
    menu.addItem(&quit_item);

    item.setMenu(Some(&menu));

    // Keep the status item alive
    std::mem::forget(item);
}
