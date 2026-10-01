use std::path::PathBuf;

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::fs_scan::{ensure_directory, inspect_rule, ScanProgressDto, ScanResultDto, ScanRuleInput};
use crate::process_watch;
use crate::validation::{
    detect_fivem_path as detect_path, validate_fivem_path as validate_path, ValidationResult,
};

#[tauri::command]
pub async fn select_folder(app: AppHandle) -> Result<Option<String>, String> {
    let folder = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Select FiveM Installation Folder")
            .blocking_pick_folder()
    })
    .await
    .map_err(|error| format!("Folder picker failed: {error}"))?;

    match folder {
        Some(file_path) => {
            let path = file_path.into_path().map_err(|error| error.to_string())?;
            Ok(Some(path.to_string_lossy().into_owned()))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub fn validate_fivem_path(path: String) -> Result<ValidationResult, String> {
    Ok(validate_path(&path))
}

#[tauri::command]
pub fn detect_fivem_path() -> Result<Option<String>, String> {
    Ok(detect_path())
}

#[tauri::command]
pub async fn inspect_path(path: String, rule: ScanRuleInput) -> Result<ScanResultDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let base = PathBuf::from(path.trim());
        ensure_directory(&base)?;
        Ok(inspect_rule(&base, &rule))
    })
    .await
    .map_err(|error| error.to_string())?
}

// Async + spawn_blocking: sync commands run on the main thread and would freeze the window
// ("Not Responding") while the whole FiveM folder is walked.
#[tauri::command]
pub async fn scan_fivem(
    app: AppHandle,
    path: String,
    rules: Vec<ScanRuleInput>,
) -> Result<Vec<ScanResultDto>, String> {
    tauri::async_runtime::spawn_blocking(move || run_scan(app, path, rules))
        .await
        .map_err(|error| error.to_string())?
}

fn run_scan(
    app: AppHandle,
    path: String,
    rules: Vec<ScanRuleInput>,
) -> Result<Vec<ScanResultDto>, String> {
    let base = PathBuf::from(path.trim());
    ensure_directory(&base)?;

    if rules.is_empty() {
        return Err("No scan rules are enabled.".to_string());
    }

    let total = rules.len();
    let mut results = Vec::with_capacity(total);

    for (index, rule) in rules.iter().enumerate() {
        let _ = app.emit(
            "scan-progress",
            ScanProgressDto {
                current: index,
                total,
                rule_id: rule.id.clone(),
                rule_name: rule.name.clone(),
            },
        );

        results.push(inspect_rule(&base, rule));
    }

    if let Some(last) = rules.last() {
        let _ = app.emit(
            "scan-progress",
            ScanProgressDto {
                current: total,
                total,
                rule_id: last.id.clone(),
                rule_name: last.name.clone(),
            },
        );
    }

    Ok(results)
}

#[tauri::command]
pub fn open_location(app: AppHandle, path: String) -> Result<(), String> {
    let selected = PathBuf::from(path.trim());
    if path.trim().is_empty() {
        return Err("No path was provided.".to_string());
    }

    let target = if selected.is_file() {
        selected
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| "The parent folder could not be determined.".to_string())?
    } else {
        selected
    };

    if !target.exists() {
        return Err("The location no longer exists.".to_string());
    }

    app.opener()
        .open_path(target.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|error| format!("Unable to open Windows Explorer: {error}"))?;

    Ok(())
}

#[tauri::command]
pub fn get_fivem_status() -> FiveMStatusDto {
    let running = process_watch::is_fivem_running();
    let install_path = if running {
        process_watch::resolve_fivem_install_path()
    } else {
        None
    };

    FiveMStatusDto {
        running,
        install_path,
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiveMStatusDto {
    pub running: bool,
    pub install_path: Option<String>,
}

#[tauri::command]
pub fn hide_main_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn show_main_window_cmd(app: AppHandle) {
    crate::process_watch::show_main_window(&app);
}

#[tauri::command]
pub fn schedule_watch_restart(app: AppHandle) {
    let Ok(exe) = std::env::current_exe() else {
        app.exit(0);
        return;
    };

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(900));
        let _ = std::process::Command::new(exe).arg("--watch").spawn();
    });

    app.exit(0);
}

#[tauri::command]
pub async fn send_discord_report(
    report: crate::discord::DiscordReport,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::discord::send_scan_report(report))
        .await
        .map_err(|error| format!("Failed to send Discord report: {error}"))?
}
