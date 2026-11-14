// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use opensim_companion::commands::{self, AppState};
use opensim_companion::tray;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::start_service,
            commands::stop_service,
            commands::service_status,
            commands::view_logs,
            commands::open_portal,
            commands::connect_remote,
            commands::list_remote_grids,
            commands::set_portal_url,
        ])
        .setup(|app| {
            tray::setup_tray(app.handle())?;

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    if let Ok(status) = commands::service_status().await {
                        tray::update_tray_status(&handle, status.running);
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running OpenSimulator Companion");
}
