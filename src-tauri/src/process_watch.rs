use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::validation::{is_fivem_data_root_name, pick_fivem_data_root};

const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FiveMStatus {
    running: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FiveMLaunched {
    install_path: Option<String>,
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut was_running = is_fivem_running();
        let _ = app.emit("fivem-status", FiveMStatus { running: was_running });

        loop {
            std::thread::sleep(POLL_INTERVAL);
            let running = is_fivem_running();

            if running != was_running {
                let _ = app.emit("fivem-status", FiveMStatus { running });
            }

            if running && !was_running {
                std::thread::sleep(Duration::from_millis(1500));
                let _ = app.emit(
                    "fivem-launched",
                    FiveMLaunched {
                        install_path: resolve_fivem_app_path(),
                    },
                );
            }

            was_running = running;
        }
    });
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn is_fivem_process_name(name: &str) -> bool {
    let name = name.trim().trim_end_matches(".exe").to_ascii_lowercase();
    name == "fivem"
        || name.starts_with("fivem_")
        || name == "citizenfx"
        || name.starts_with("citizenfx_")
}

fn app_folder_from_exe(exe: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let Some(mut current) = exe.parent() else {
        return candidates;
    };

    for _ in 0..10 {
        if current
            .file_name()
            .is_some_and(is_fivem_data_root_name)
            && current.is_dir()
        {
            candidates.push(current.to_path_buf());
        }

        candidates.push(current.join("FiveM Application Data"));
        candidates.push(current.join("FiveM.app"));

        let Some(parent) = current.parent() else {
            break;
        };
        current = parent;
    }

    candidates
}

#[cfg(windows)]
pub fn is_fivem_running() -> bool {
    has_fivem_process()
}

#[cfg(windows)]
pub fn resolve_fivem_install_path() -> Option<String> {
    resolve_fivem_app_path()
}

#[cfg(not(windows))]
pub fn resolve_fivem_install_path() -> Option<String> {
    None
}

#[cfg(windows)]
fn resolve_fivem_app_path() -> Option<String> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    for exe in running_fivem_executables() {
        candidates.extend(app_folder_from_exe(&exe));
        if let Some(parent) = exe.parent() {
            candidates.push(parent.to_path_buf());
        }
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let fivem_root = PathBuf::from(local_app_data).join("FiveM");
        candidates.push(fivem_root.join("FiveM Application Data"));
        candidates.push(fivem_root.join("FiveM.app"));
        candidates.push(fivem_root);
    }

    pick_fivem_data_root(candidates)
}

#[cfg(windows)]
fn has_fivem_process() -> bool {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }

        let mut entry: PROCESSENTRY32W = zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let name = wchar_to_string(&entry.szExeFile);
                if is_fivem_process_name(&name) {
                    found = true;
                    break;
                }

                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
        found
    }
}

#[cfg(windows)]
fn running_fivem_executables() -> Vec<PathBuf> {
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Vec::new();
        }

        let mut entry: PROCESSENTRY32W = zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut paths = Vec::new();

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let name = wchar_to_string(&entry.szExeFile);
                if is_fivem_process_name(&name) {
                    if let Some(path) = query_image_path(entry.th32ProcessID) {
                        paths.push(path);
                    }
                }

                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
        paths
    }
}

#[cfg(windows)]
fn query_image_path(pid: u32) -> Option<PathBuf> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }

        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size);
        CloseHandle(handle);

        if ok == 0 {
            return None;
        }

        Some(PathBuf::from(String::from_utf16_lossy(
            &buffer[..size as usize],
        )))
    }
}

#[cfg(windows)]
fn wchar_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&ch| ch == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

#[cfg(not(windows))]
pub fn is_fivem_running() -> bool {
    false
}

#[cfg(not(windows))]
fn resolve_fivem_app_path() -> Option<String> {
    None
}
