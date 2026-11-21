//! IDE-CORE-IPC-01B-R3: local OS menu + native human approval only.
//!
//! This code is feature-gated, installed-profile-gated and never part of the
//! WebView invoke list. A menu click is not approval: a separate native OS
//! confirmation dialog must be accepted before any issuer contact.
#![cfg(all(feature = "native-core-health", target_os = "linux"))]
use lu_ide_workspace::installed_core_health::InstalledCoreHealthRuntime;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuEvent, MenuItem, Submenu},
    App, AppHandle, Manager,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

const CHECK_ID: &str = "lu_ide_native_core_health_confirmed_check";
const CANCEL_ID: &str = "lu_ide_native_core_health_cancel";

pub(super) fn install(app: &App) -> tauri::Result<()> {
    // The opt-in menu is attached only after root-owned installed profile,
    // true binary identity and durable restart reconciliation have passed.
    let check = MenuItem::with_id(app, CHECK_ID, "Check Lu Core health…", true, None::<&str>)?;
    let cancel = MenuItem::with_id(
        app,
        CANCEL_ID,
        "Cancel Core health check",
        true,
        None::<&str>,
    )?;
    let submenu = Submenu::with_items(app, "Lu Core (read only)", true, &[&check, &cancel])?;
    let menu = match app.menu() {
        Some(menu) => menu,
        None => Menu::new(app)?,
    };
    menu.append(&submenu)?;
    app.set_menu(menu)?;
    Ok(())
}

pub(super) fn on_menu_event(app: &AppHandle, event: &MenuEvent) {
    if event.id() == CANCEL_ID {
        if let Some(runtime) = app.try_state::<Arc<InstalledCoreHealthRuntime>>() {
            runtime.cancel();
        }
    } else if event.id() == CHECK_ID {
        let Some(runtime) = app.try_state::<Arc<InstalledCoreHealthRuntime>>() else {
            return;
        };
        let Ok(permit) = runtime.reserve_native_action() else {
            return;
        };
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let approved = app.dialog()
                .message("Request ONE read-only Lu Core Health observation?\n\nThe installed OS issuer must independently authorize this operation. Health never grants access to tools, files or model execution. Deny/Cancel will consume this attempt without borrowing credentials.")
                .title("Lu IDE · Native Core Health approval")
                .buttons(MessageDialogButtons::OkCancelCustom("Approve one health check".into(), "Deny / Cancel".into()))
                .blocking_show();
            let disposition = permit.finish(approved);
            // Show a fixed, redacted native result, never a credential or
            // operation reference. No WebView event is emitted.
            app.dialog()
                .message(disposition.message())
                .title("Lu IDE · Core Health result")
                .blocking_show();
        });
    }
}
