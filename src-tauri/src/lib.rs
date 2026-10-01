mod commands;
mod discord;
mod fs_scan;
mod process_watch;
mod tray;
mod validation;

use commands::{
    detect_fivem_path, get_fivem_status, hide_main_window, inspect_path, open_location, scan_fivem,
    schedule_watch_restart, select_folder, send_discord_report, show_main_window_cmd,
    validate_fivem_path,
};
use process_watch::show_main_window;
use tauri_plugin_autostart::MacosLauncher;

/// Display name of the tenant this binary was built for (set by build.rs).
pub const APP_NAME: &str = env!("APP_DISPLAY_NAME");

pub fn is_watch_mode() -> bool {
    std::env::args().any(|arg| arg == "--watch" || arg == "--autostart")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.iter().any(|arg| arg == "--watch") {
                return;
            }
            show_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--watch".into()]),
        ))
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            tray::setup(app)?;
            tray::configure_main_window(app.handle());
            process_watch::start(app.handle().clone());

            if !is_watch_mode() {
                show_main_window(app.handle());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            select_folder,
            validate_fivem_path,
            detect_fivem_path,
            inspect_path,
            scan_fivem,
            open_location,
            send_discord_report,
            hide_main_window,
            show_main_window_cmd,
            get_fivem_status,
            schedule_watch_restart
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
