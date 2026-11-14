// SPDX-License-Identifier: GPL-3.0-or-later

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, Runtime,
};

pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let status_item = MenuItem::with_id(app, "status", "Status: Unknown", false, None::<&str>)?;
    let start_item = MenuItem::with_id(app, "start", "Start Service", true, None::<&str>)?;
    let stop_item = MenuItem::with_id(app, "stop", "Stop Service", true, None::<&str>)?;
    let portal_item =
        MenuItem::with_id(app, "open_portal", "Open Portal in Browser", true, None::<&str>)?;
    let show_item = MenuItem::with_id(app, "show_window", "Show Window", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &status_item,
            &start_item,
            &stop_item,
            &portal_item,
            &show_item,
            &quit_item,
        ],
    )?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
            tauri::image::Image::new(&[], 1, 1)
        }))
        .menu(&menu)
        .tooltip("OpenSimulator Companion")
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "start" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    match crate::commands::start_service().await {
                        Ok(status) => {
                            if status.running {
                                let _ = send_notification(&app, "Service Started",
                                    "OpenSimulator service is now running.");
                            }
                        }
                        Err(e) => {
                            let _ = send_notification(&app, "Start Failed", &e);
                        }
                    }
                });
            }
            "stop" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    match crate::commands::stop_service().await {
                        Ok(status) => {
                            if !status.running {
                                let _ = send_notification(&app, "Service Stopped",
                                    "OpenSimulator service has stopped.");
                            }
                        }
                        Err(e) => {
                            let _ = send_notification(&app, "Stop Failed", &e);
                        }
                    }
                });
            }
            "open_portal" => {
                let _ = open::that("http://localhost:8002");
            }
            "show_window" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn send_notification<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) -> tauri::Result<()> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()?;
    Ok(())
}

pub fn update_tray_status<R: Runtime>(app: &AppHandle<R>, running: bool) {
    if let Some(tray) = app.tray_by_id("main") {
        let label = if running {
            "Status: Running"
        } else {
            "Status: Stopped"
        };
        let tooltip = if running {
            "OpenSimulator — Running"
        } else {
            "OpenSimulator — Stopped"
        };
        let _ = tray.set_tooltip(Some(tooltip));
        if let Ok(menu) = tray.menu() {
            if let Some(menu) = menu {
                if let Some(item) = menu.get("status") {
                    if let Some(mi) = item.as_menuitem() {
                        let _ = mi.set_text(label);
                    }
                }
            }
        }
    }
}
