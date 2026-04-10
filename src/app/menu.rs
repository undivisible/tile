use log::{error, warn};
use objc2::define_class;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSMenu, NSMenuItem, NSStatusBar};
use objc2_foundation::{ns_string, MainThreadMarker as FoundationMainThreadMarker, NSObject, NSString};

fn launch_tile_panel(panel: &str) {
    let current_exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => {
            error!("Could not determine Tile executable path: {}", err);
            return;
        }
    };

    let exe_dir = match current_exe.parent() {
        Some(dir) => dir,
        None => {
            error!("Tile executable has no parent directory");
            return;
        }
    };

    let candidates = [
        exe_dir.join("tile-settings"),
        exe_dir.join("tile-settings.app/Contents/MacOS/tile-settings"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            match std::process::Command::new(&candidate).arg(panel).spawn() {
                Ok(_) => return,
                Err(err) => {
                    error!("Failed to launch {} via {}: {}", panel, candidate.display(), err);
                    return;
                }
            }
        }
    }

    warn!("Could not find tile-settings binary next to {}", current_exe.display());
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    struct TileMenuHandler;

    impl TileMenuHandler {
        #[unsafe(method(openAbout:))]
        fn open_about(&self, _sender: Option<&AnyObject>) {
            launch_tile_panel("about");
        }

        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            launch_tile_panel("settings");
        }
    }
);

impl TileMenuHandler {
    fn new(mtm: FoundationMainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { objc2::msg_send![super(this), init] }
    }
}

/// Set up the status bar menu.
pub(crate) fn setup_status_bar(mtm: MainThreadMarker) {
    let status_bar = NSStatusBar::systemStatusBar();
    let item = status_bar.statusItemWithLength(-1.0); // NSVariableStatusItemLength
    let handler = TileMenuHandler::new(mtm);

    if let Some(button) = item.button(mtm) {
        button.setTitle(ns_string!("\u{229e}")); // ⊞ symbol
    }

    let menu = NSMenu::new(mtm);

    let about_title = NSString::from_str("About Tile");
    let about_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &about_title,
            Some(objc2::sel!(openAbout:)),
            ns_string!(""),
        )
    };
    unsafe {
        about_item.setTarget(Some(&*handler));
    }
    menu.addItem(&about_item);

    let settings_title = NSString::from_str("Settings...");
    let settings_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &settings_title,
            Some(objc2::sel!(openSettings:)),
            ns_string!(","),
        )
    };
    unsafe {
        settings_item.setTarget(Some(&*handler));
    }
    menu.addItem(&settings_item);

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    let quit_title = NSString::from_str("Quit Tile");
    let quit_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &quit_title,
            Some(objc2::sel!(terminate:)),
            ns_string!("q"),
        )
    };
    menu.addItem(&quit_item);

    item.setMenu(Some(&menu));

    std::mem::forget(handler);
    std::mem::forget(item);
}
