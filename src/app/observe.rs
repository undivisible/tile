use std::sync::{Arc, Mutex};

use crate::app::state::{lock_state, AppState};

/// Start observing all running regular applications.
pub(crate) fn observe_running_apps(state: &Arc<Mutex<AppState>>) {
    let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
    let apps = workspace.runningApplications();

    let mut st = lock_state(state);
    let observer = match st.observer.as_mut() {
        Some(o) => o,
        None => return,
    };

    for app in apps.iter() {
        if app.activationPolicy() == objc2_app_kit::NSApplicationActivationPolicy::Regular {
            let pid = app.processIdentifier();
            observer.observe_app(pid);
        }
    }
}
