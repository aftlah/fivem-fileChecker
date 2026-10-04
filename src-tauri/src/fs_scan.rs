use crate::{deep_scan, rpf_scan};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanRuleInput {
    pub id: String,
    pub name: String,
    pub relative_path: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub severity: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanResultDto {
    pub rule_id: String,
    pub name: String,
    pub status: String,
    pub path: String,
    pub relative_path: String,
    pub exists: bool,
    #[serde(rename = "type")]
    pub item_type: String,
    pub severity: String,
    pub modified_at: Option<i64>,
    pub error: Option<String>,
    pub found_files: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressDto {
    pub current: usize,
    pub total: usize,
    pub rule_id: String,
    pub rule_name: String,
}

pub fn join_relative(base: &Path, relative: &str) -> Result<PathBuf, String> {
    let trimmed = relative.trim();
    if trimmed.is_empty() {
        return Err("Scan rule is missing a relative path.".to_string());
    }

    let mut path = base.to_path_buf();
    for part in trimmed.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err("Invalid relative path.".to_string());
        }
        path.push(part);
    }

    Ok(path)
}

pub fn inspect_rule(base: &Path, rule: &ScanRuleInput) -> ScanResultDto {
    if rule.item_type == "scripts" {
        return inspect_scripts(base, rule);
    }

    if rule.item_type == "file-search" {
        return inspect_named_file_search(base, rule);
    }

    if rule.item_type == "rpf-archives" {
        return inspect_flagged_files(base, rule, rpf_scan::is_rpf_file, rpf_scan::analyze_rpf);
    }

    if rule.item_type == "deep-scan" {
        return inspect_flagged_files(base, rule, deep_scan::is_candidate, deep_scan::analyze);
    }

    if rule.item_type == "content-scan" {
        return inspect_flagged_files(
            base,
            rule,
            rpf_scan::is_loose_data_file,
            rpf_scan::analyze_loose_file,
        );
    }

    let full_path = match join_relative(base, &rule.relative_path) {
        Ok(path) => path,
        Err(message) => {
            return error_result(rule, String::new(), message, false);
        }
    };

    let path_str = full_path.to_string_lossy().into_owned();

    match fs::metadata(&full_path) {
        Ok(metadata) => {
            let is_dir = metadata.is_dir();
            let is_file = metadata.is_file();
            let matches_type = match rule.item_type.as_str() {
                "directory" => is_dir,
                "file" => is_file,
                other => {
                    return error_result(
                        rule,
                        path_str,
                        format!("Unsupported scan type: {other}."),
                        true,
                    );
                }
            };

            if !matches_type {
                let found = if is_dir { "directory" } else { "file" };
                return error_result(
                    rule,
                    path_str,
                    format!(
                        "Expected a {}, but found a {found} instead.",
                        rule.item_type
                    ),
                    true,
                );
            }

            let found_files = if is_dir {
                list_contained_files(&full_path, 80)
            } else {
                Vec::new()
            };

            ScanResultDto {
                rule_id: rule.id.clone(),
                name: rule.name.clone(),
                status: "DETECTED".to_string(),
                path: path_str,
                relative_path: rule.relative_path.clone(),
                exists: true,
                item_type: rule.item_type.clone(),
                severity: rule.severity.clone(),
                modified_at: metadata.modified().ok().and_then(to_epoch_ms),
                error: None,
                found_files,
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => ScanResultDto {
            rule_id: rule.id.clone(),
            name: rule.name.clone(),
            status: "NOT_FOUND".to_string(),
            path: path_str,
            relative_path: rule.relative_path.clone(),
            exists: false,
            item_type: rule.item_type.clone(),
            severity: rule.severity.clone(),
            modified_at: None,
            error: None,
            found_files: Vec::new(),
        },
        Err(error) if error.kind() == ErrorKind::PermissionDenied => error_result(
            rule,
            path_str,
            "Permission denied. This folder or file could not be read.".to_string(),
            false,
        ),
        Err(error) => error_result(
            rule,
            path_str,
            format!("Unable to inspect this path: {error}"),
            false,
        ),
    }
}

pub fn ensure_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err("The selected folder does not exist.".to_string());
    }

    if !path.is_dir() {
        return Err("The selected path is not a folder.".to_string());
    }

    Ok(())
}

fn inspect_named_file_search(base: &Path, rule: &ScanRuleInput) -> ScanResultDto {
    let target_name = rule.relative_path.trim();
    if target_name.is_empty()
        || target_name.contains('/')
        || target_name.contains('\\')
        || target_name == "."
        || target_name == ".."
    {
        return error_result(
            rule,
            base.to_string_lossy().into_owned(),
            "file-search rules must use a plain file name (for example pedaccuracy.meta).".to_string(),
            false,
        );
    }

    let found_files = find_named_files(base, target_name, 80);
    let path_str = base.to_string_lossy().into_owned();

    if found_files.is_empty() {
        return ScanResultDto {
            rule_id: rule.id.clone(),
            name: rule.name.clone(),
            status: "NOT_FOUND".to_string(),
            path: path_str,
            relative_path: rule.relative_path.clone(),
            exists: false,
            item_type: rule.item_type.clone(),
            severity: rule.severity.clone(),
            modified_at: None,
            error: None,
            found_files: Vec::new(),
        };
    }

    let first_full = join_relative(base, &found_files[0])
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path_str.clone());

    ScanResultDto {
        rule_id: rule.id.clone(),
        name: rule.name.clone(),
        status: "DETECTED".to_string(),
        path: first_full,
        relative_path: rule.relative_path.clone(),
        exists: true,
        item_type: rule.item_type.clone(),
        severity: rule.severity.clone(),
        modified_at: None,
        error: None,
        found_files,
    }
}

fn find_named_files(root: &Path, target_name: &str, max: usize) -> Vec<String> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            if files.len() >= max {
                files.sort();
                return files;
            }

            let path = entry.path();
            if path.is_dir() {
                if is_skipped_dir(&entry.file_name()) {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if !path.is_file() {
                continue;
            }

            let matches_name = entry
                .file_name()
                .to_str()
                .map(|name| name.eq_ignore_ascii_case(target_name))
                .unwrap_or(false);

            if !matches_name {
                continue;
            }

            if let Ok(relative) = path.strip_prefix(root) {
                files.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    files.sort();
    files
}

fn inspect_flagged_files(
    base: &Path,
    rule: &ScanRuleInput,
    is_candidate: fn(&Path) -> bool,
    analyze: fn(&Path) -> Vec<String>,
) -> ScanResultDto {
    let path_str = base.to_string_lossy().into_owned();
    let mut found_files = Vec::new();
    let mut first_path: Option<String> = None;

    for candidate in find_files_where(base, 20000, is_candidate) {
        if found_files.len() >= 80 {
            break;
        }
        let reasons = analyze(&candidate);
        if reasons.is_empty() {
            continue;
        }

        let rpf = candidate;
        let relative = rpf
            .strip_prefix(base)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| rpf.to_string_lossy().into_owned());
        if rule.item_type == "rpf-archives"
            && rpf_scan::is_official_archive_noise(&relative, &reasons)
        {
            continue;
        }
        if first_path.is_none() {
            first_path = Some(rpf.to_string_lossy().into_owned());
        }
        found_files.push(format!("{relative} ({})", reasons.join("; ")));
    }

    found_files.sort();
    let detected = !found_files.is_empty();

    ScanResultDto {
        rule_id: rule.id.clone(),
        name: rule.name.clone(),
        status: if detected { "DETECTED" } else { "NOT_FOUND" }.to_string(),
        path: first_path.unwrap_or(path_str),
        relative_path: rule.relative_path.clone(),
        exists: detected,
        item_type: rule.item_type.clone(),
        severity: rule.severity.clone(),
        modified_at: None,
        error: None,
        found_files,
    }
}

fn find_files_where(root: &Path, max: usize, matches: fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            if files.len() >= max {
                return files;
            }

            let path = entry.path();
            if path.is_dir() {
                if is_skipped_dir(&entry.file_name()) {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if path.is_file() && matches(&path) {
                files.push(path);
            }
        }
    }

    files
}

fn inspect_scripts(base: &Path, rule: &ScanRuleInput) -> ScanResultDto {
    let search_root = match join_relative(base, &rule.relative_path) {
        Ok(path) => path,
        Err(message) => {
            return error_result(rule, String::new(), message, false);
        }
    };

    if !search_root.exists() {
        return ScanResultDto {
            rule_id: rule.id.clone(),
            name: rule.name.clone(),
            status: "NOT_FOUND".to_string(),
            path: search_root.to_string_lossy().into_owned(),
            relative_path: rule.relative_path.clone(),
            exists: false,
            item_type: rule.item_type.clone(),
            severity: rule.severity.clone(),
            modified_at: None,
            error: None,
            found_files: Vec::new(),
        };
    }

    let found_files = find_extra_scripts(&search_root, 80);
    let path_str = search_root.to_string_lossy().into_owned();

    if found_files.is_empty() {
        return ScanResultDto {
            rule_id: rule.id.clone(),
            name: rule.name.clone(),
            status: "NOT_FOUND".to_string(),
            path: path_str,
            relative_path: rule.relative_path.clone(),
            exists: true,
            item_type: rule.item_type.clone(),
            severity: rule.severity.clone(),
            modified_at: None,
            error: None,
            found_files: Vec::new(),
        };
    }

    ScanResultDto {
        rule_id: rule.id.clone(),
        name: rule.name.clone(),
        status: "DETECTED".to_string(),
        path: path_str,
        relative_path: rule.relative_path.clone(),
        exists: true,
        item_type: rule.item_type.clone(),
        severity: rule.severity.clone(),
        modified_at: None,
        error: None,
        found_files,
    }
}

const SKIP_DIR_NAMES: [&str; 7] = [
    "cache",
    "logs",
    "crashes",
    "scripting",
    "system_resources",
    "clr2",
    "nui",
];

fn is_skipped_dir(name: &std::ffi::OsStr) -> bool {
    SKIP_DIR_NAMES
        .iter()
        .any(|skip| name.eq_ignore_ascii_case(skip))
}

/// FiveM ships the FxDK (citizen/sdk) with hundreds of its own .js/.lua files.
fn is_official_script_dir(root: &Path, dir: &Path) -> bool {
    dir.strip_prefix(root)
        .map(|relative| {
            relative.to_string_lossy().replace('\\', "/").eq_ignore_ascii_case("citizen/sdk")
        })
        .unwrap_or(false)
}

fn is_script_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "lua" | "luac" | "js" | "asi"
            )
        })
        .unwrap_or(false)
}

fn find_extra_scripts(root: &Path, max: usize) -> Vec<String> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            if files.len() >= max {
                files.sort();
                return files;
            }

            let path = entry.path();
            if path.is_dir() {
                if is_skipped_dir(&entry.file_name()) || is_official_script_dir(root, &path) {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if !path.is_file() || !is_script_file(&path) {
                continue;
            }

            if let Ok(relative) = path.strip_prefix(root) {
                files.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    files.sort();
    files
}

fn error_result(
    rule: &ScanRuleInput,
    path: String,
    message: String,
    exists: bool,
) -> ScanResultDto {
    ScanResultDto {
        rule_id: rule.id.clone(),
        name: rule.name.clone(),
        status: "ERROR".to_string(),
        path,
        relative_path: rule.relative_path.clone(),
        exists,
        item_type: rule.item_type.clone(),
        severity: rule.severity.clone(),
        modified_at: None,
        error: Some(message),
        found_files: Vec::new(),
    }
}

fn list_contained_files(root: &Path, max: usize) -> Vec<String> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            if files.len() >= max {
                files.sort();
                return files;
            }

            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }

            if !path.is_file() {
                continue;
            }

            if let Ok(relative) = path.strip_prefix(root) {
                files.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    files.sort();
    files
}

fn to_epoch_ms(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_forward_slash_relative_path() {
        let base = PathBuf::from(r"C:\FiveM.app");
        let joined = join_relative(&base, "citizen/common/data/ai").unwrap();
        assert_eq!(joined, PathBuf::from(r"C:\FiveM.app\citizen\common\data\ai"));
    }

    #[test]
    fn rejects_parent_directory_segments() {
        let base = PathBuf::from(r"C:\FiveM.app");
        let result = join_relative(&base, "citizen/../Windows");
        assert!(result.is_err());
    }

    #[test]
    fn lists_files_inside_detected_directory() {
        let dir = std::env::temp_dir().join(format!(
            "rage-scan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let ai = dir.join("citizen").join("common").join("data").join("ai");
        fs::create_dir_all(&ai).unwrap();
        fs::write(ai.join("pedaccuracy.meta"), b"test").unwrap();

        let rule = ScanRuleInput {
            id: "ai-folder".into(),
            name: "AI Folder".into(),
            relative_path: "citizen/common/data/ai".into(),
            item_type: "directory".into(),
            severity: "high".into(),
        };

        let result = inspect_rule(&dir, &rule);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(result.status, "DETECTED");
        assert!(result
            .found_files
            .iter()
            .any(|file| file == "pedaccuracy.meta"));
    }

    #[test]
    fn named_file_search_finds_pedaccuracy_outside_ai_folder() {
        let dir = std::env::temp_dir().join(format!(
            "rage-named-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let hidden = dir.join("mods").join("custom");
        fs::create_dir_all(&hidden).unwrap();
        fs::write(hidden.join("pedaccuracy.meta"), b"cheat").unwrap();

        let rule = ScanRuleInput {
            id: "ped-accuracy".into(),
            name: "Ped Accuracy".into(),
            relative_path: "pedaccuracy.meta".into(),
            item_type: "file-search".into(),
            severity: "high".into(),
        };

        let result = inspect_rule(&dir, &rule);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(result.status, "DETECTED");
        assert!(result
            .found_files
            .iter()
            .any(|file| file == "mods/custom/pedaccuracy.meta"));
    }

    #[test]
    fn extra_scripts_ignore_official_scripting_folder() {
        let dir = std::env::temp_dir().join(format!(
            "rage-scripts-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let official = dir.join("citizen").join("scripting").join("lua");
        let plugins = dir.join("plugins");
        fs::create_dir_all(&official).unwrap();
        fs::create_dir_all(&plugins).unwrap();
        fs::write(official.join("scheduler.lua"), b"official").unwrap();
        fs::write(plugins.join("menu.asi"), b"asi").unwrap();
        fs::write(dir.join("extra.lua"), b"lua").unwrap();

        let rule = ScanRuleInput {
            id: "extra-scripts".into(),
            name: "Extra Scripts".into(),
            relative_path: ".".into(),
            item_type: "scripts".into(),
            severity: "high".into(),
        };

        let result = inspect_rule(&dir, &rule);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(result.status, "DETECTED");
        assert!(result.found_files.iter().any(|file| file == "extra.lua"));
        assert!(result
            .found_files
            .iter()
            .any(|file| file == "plugins/menu.asi"));
        assert!(!result
            .found_files
            .iter()
            .any(|file| file.contains("scheduler.lua")));
    }
}
