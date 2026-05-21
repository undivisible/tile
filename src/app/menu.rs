use std::sync::{Arc, Mutex, OnceLock};
use std::{path::PathBuf, process::Command};

use log::warn;
use objc2::define_class;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSMenu, NSMenuItem, NSStatusBar};
use objc2_foundation::{
    ns_string, MainThreadMarker as FoundationMainThreadMarker, NSObject, NSString,
};
use tile_core::Rect;

use super::actions::sync_bsp_layout;
use super::features;
use super::state::{lock_state, AppState, TilingMode};

static APP_STATE: OnceLock<Arc<Mutex<AppState>>> = OnceLock::new();

fn app_state() -> Option<&'static Arc<Mutex<AppState>>> {
    APP_STATE.get()
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    struct TileMenuHandler;

    impl TileMenuHandler {
        #[unsafe(method(switchToSnapMode:))]
        fn switch_to_snap_mode(&self, _sender: Option<&AnyObject>) {
            let Some(state) = app_state() else {
                warn!("Menu state is unavailable");
                return;
            };
            let mut st = lock_state(state);
            st.tiling_mode = TilingMode::Snap;
            st.config.tiling_mode = tile_settings::TilingModeConfig::Snap;
            if let Err(err) = st.config.save() {
                warn!("Failed to save config after switching to Snap mode: {}", err);
            }
        }

        #[unsafe(method(switchToBspMode:))]
        fn switch_to_bsp_mode(&self, _sender: Option<&AnyObject>) {
            let Some(state) = app_state() else {
                warn!("Menu state is unavailable");
                return;
            };
            let region = {
                let mut st = lock_state(state);
                st.tiling_mode = TilingMode::Bsp;
                st.config.tiling_mode = tile_settings::TilingModeConfig::Bsp;
                if let Err(err) = st.config.save() {
                    warn!("Failed to save config after switching to BSP mode: {}", err);
                }
                st.multiplexer.active_region.map(|r| r.rect)
            };
            sync_bsp_layout(state, region, None);
        }

        #[unsafe(method(resetDefaults:))]
        fn reset_defaults(&self, _sender: Option<&AnyObject>) {
            let Some(state) = app_state() else {
                warn!("Menu state is unavailable");
                return;
            };
            let mut st = lock_state(state);
            st.config = tile_settings::TileConfig::default();
            st.tiling_mode = match st.config.tiling_mode {
                tile_settings::TilingModeConfig::Snap => TilingMode::Snap,
                tile_settings::TilingModeConfig::Bsp => TilingMode::Bsp,
            };
            st.tree.gaps.outer = st.config.gap_outer;
            st.tree.gaps.inner = st.config.gap_inner;
            if let Err(err) = st.config.save() {
                warn!("Failed to save reset config: {}", err);
            }
        }

        #[unsafe(method(openSettingsWindow:))]
        fn open_settings_window(&self, _sender: Option<&AnyObject>) {
            launch_settings_window();
        }

        #[unsafe(method(togglePause:))]
        fn toggle_pause(&self, _sender: Option<&AnyObject>) {
            let Some(state) = app_state() else {
                warn!("Menu state is unavailable");
                return;
            };
            let paused = features::toggle_pause(state);
            warn!("Tile pause toggled: {}", paused);
        }

        #[unsafe(method(flipLayout:))]
        fn flip_layout(&self, _sender: Option<&AnyObject>) {
            let Some(state) = app_state() else {
                warn!("Menu state is unavailable");
                return;
            };
            let screen = tile_ax::get_usable_screen_frame(0)
                .unwrap_or(Rect::new(0.0, 0.0, 1920.0, 1080.0));
            features::flip_layout(state, screen);
        }

        #[unsafe(method(toggleMonocle:))]
        fn toggle_monocle(&self, _sender: Option<&AnyObject>) {
            let Some(state) = app_state() else {
                warn!("Menu state is unavailable");
                return;
            };
            let screen = tile_ax::get_usable_screen_frame(0)
                .unwrap_or(Rect::new(0.0, 0.0, 1920.0, 1080.0));
            features::toggle_monocle(state, screen);
        }

        #[unsafe(method(unstackFocused:))]
        fn unstack_focused(&self, _sender: Option<&AnyObject>) {
            let Some(state) = app_state() else {
                warn!("Menu state is unavailable");
                return;
            };
            let screen = tile_ax::get_usable_screen_frame(0)
                .unwrap_or(Rect::new(0.0, 0.0, 1920.0, 1080.0));
            features::unstack_focused(state, screen);
        }

        #[unsafe(method(unstackAllFocused:))]
        fn unstack_all_focused(&self, _sender: Option<&AnyObject>) {
            let Some(state) = app_state() else {
                warn!("Menu state is unavailable");
                return;
            };
            let screen = tile_ax::get_usable_screen_frame(0)
                .unwrap_or(Rect::new(0.0, 0.0, 1920.0, 1080.0));
            features::unstack_all_focused(state, screen);
        }
    }
);

impl TileMenuHandler {
    fn new(mtm: FoundationMainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { objc2::msg_send![super(this), init] }
    }
}

fn add_disabled_item(menu: &NSMenu, title: &str, mtm: MainThreadMarker) {
    let item_title = NSString::from_str(title);
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &item_title,
            None,
            ns_string!(""),
        )
    };
    item.setEnabled(false);
    menu.addItem(&item);
}

fn add_action_item(
    menu: &NSMenu,
    title: &str,
    action: objc2::runtime::Sel,
    handler: &TileMenuHandler,
    mtm: MainThreadMarker,
) {
    let item_title = NSString::from_str(title);
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &item_title,
            Some(action),
            ns_string!(""),
        )
    };
    unsafe {
        item.setTarget(Some(&*handler));
    }
    menu.addItem(&item);
}

fn add_submenu_header(menu: &NSMenu, title: &str, mtm: MainThreadMarker) {
    let header = NSMenuItem::sectionHeaderWithTitle(&NSString::from_str(title), mtm);
    menu.addItem(&header);
}

fn launch_settings_window() {
    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => {
            warn!("Failed to locate current executable: {}", err);
            return;
        }
    };

    let mut settings_path = exe
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    settings_path.push("tile-settings");

    let mut command = if settings_path.exists() {
        Command::new(settings_path)
    } else {
        Command::new("tile-settings")
    };

    if let Err(err) = command.spawn() {
        warn!("Failed to launch tile-settings: {}", err);
    }
}

/// Set up the status bar menu.
pub(crate) fn setup_status_bar(mtm: MainThreadMarker, state: Arc<Mutex<AppState>>) {
    let _ = APP_STATE.set(state.clone());

    let status_bar = NSStatusBar::systemStatusBar();
    let item = status_bar.statusItemWithLength(-1.0); // NSVariableStatusItemLength
    let handler = TileMenuHandler::new(mtm);

    if let Some(button) = item.button(mtm) {
        button.setTitle(ns_string!("\u{229e}")); // ⊞ symbol
    }

    let menu = NSMenu::new(mtm);
    let current = {
        let st = lock_state(&state);
        st.config.clone()
    };

    add_submenu_header(&menu, "About", mtm);
    add_disabled_item(&menu, &format!("Tile {}", env!("CARGO_PKG_VERSION")), mtm);
    add_disabled_item(
        &menu,
        "A macOS tiling window manager with Rectangle-style shortcuts and drag-to-tile overlays.",
        mtm,
    );
    add_disabled_item(
        &menu,
        "Tile needs Accessibility access to list windows, observe focus, and move or resize them.",
        mtm,
    );
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    add_submenu_header(&menu, "Tiling Mode", mtm);
    add_disabled_item(
        &menu,
        &format!("Current mode: {:?}", current.tiling_mode),
        mtm,
    );
    add_action_item(
        &menu,
        "Switch to Snap Mode",
        objc2::sel!(switchToSnapMode:),
        &handler,
        mtm,
    );
    add_action_item(
        &menu,
        "Switch to BSP Mode",
        objc2::sel!(switchToBspMode:),
        &handler,
        mtm,
    );
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    add_submenu_header(&menu, "Configuration", mtm);
    add_action_item(
        &menu,
        "Open Settings...",
        objc2::sel!(openSettingsWindow:),
        &handler,
        mtm,
    );
    add_action_item(
        &menu,
        "Reset to Defaults",
        objc2::sel!(resetDefaults:),
        &handler,
        mtm,
    );
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    add_submenu_header(&menu, "Komorebi-Style Features", mtm);
    add_action_item(
        &menu,
        "Pause Tiling",
        objc2::sel!(togglePause:),
        &handler,
        mtm,
    );
    add_action_item(
        &menu,
        "Flip Layout",
        objc2::sel!(flipLayout:),
        &handler,
        mtm,
    );
    add_action_item(
        &menu,
        "Toggle Monocle",
        objc2::sel!(toggleMonocle:),
        &handler,
        mtm,
    );
    add_action_item(
        &menu,
        "Unstack Focused",
        objc2::sel!(unstackFocused:),
        &handler,
        mtm,
    );
    add_action_item(
        &menu,
        "Unstack All Focused",
        objc2::sel!(unstackAllFocused:),
        &handler,
        mtm,
    );
    add_disabled_item(
        &menu,
        &format!("Outer gap: {:.0}px", current.gap_outer),
        mtm,
    );
    add_disabled_item(
        &menu,
        &format!("Inner gap: {:.0}px", current.gap_inner),
        mtm,
    );
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
