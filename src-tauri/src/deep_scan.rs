//! Forced, read-only content check of every file that looks suspicious by type or name.
//!
//! Unlike the loose-file check, there is no extension whitelist and the size limit is much
//! higher: scripts, ASI plugins, archives and anything named like a cheat are opened and
//! searched byte by byte. A file that cannot be opened is reported too, because blocking
//! the scanner is itself suspicious.

use aho_corasick::AhoCorasick;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const FULL_READ_LIMIT: u64 = 64 * 1024 * 1024;
const HALF_READ: u64 = 32 * 1024 * 1024;
const MAX_REASONS: usize = 8;
/// Source scripts are normally 4-6 bits of entropy per byte; encrypted or packed payloads
/// are close to 8.
const ENTROPY_THRESHOLD: f64 = 7.2;
const ENTROPY_MIN_BYTES: usize = 2048;

/// Extensions that are opened no matter what they are called.
const FORCED_EXTENSIONS: [&str; 8] = ["lua", "luac", "js", "asi", "zip", "rar", "7z", "cfg"];
/// `.dll` files are only opened in folders where mods are normally dropped, so the
/// legitimate FiveM runtime libraries are not read.
const DLL_FOLDERS: [&str; 5] = ["plugins", "scripts", "mods", "asi", "bin"];

/// (keyword, label). Matching is ASCII case-insensitive and also runs over binary files.
const KEYWORDS: [(&str, &str); 19] = [
    ("norecoil", "no recoil"),
    ("no_recoil", "no recoil"),
    ("no-recoil", "no recoil"),
    ("no recoil", "no recoil"),
    ("PLAYER_RECOIL_MODIFIER", "recoil modifier values"),
    ("sPedAccuracyModifiers", "ped accuracy data"),
    ("pedaccuracy.meta", "pedaccuracy.meta reference"),
    ("aimbot", "aimbot"),
    ("triggerbot", "triggerbot"),
    ("silentaim", "silent aim"),
    ("magicbullet", "magic bullet"),
    ("eulen", "known cheat menu (Eulen)"),
    ("redengine", "known cheat menu (RedEngine)"),
    ("skript.gg", "cheat marketplace reference"),
    ("SetPlayerWeaponDamageModifier", "weapon damage native"),
    ("SetPedInfiniteAmmoClip", "infinite ammo native"),
    ("SET_PLAYER_WEAPON_DAMAGE_MODIFIER", "weapon damage native"),
    ("assembly.xml", "OpenIV package reference"),
    ("weaponcomponents.meta", "weapon meta reference"),
];

/// File-name fragments that are suspicious on their own (any extension).
const NAME_KEYWORDS: [&str; 7] = [
    "norecoil",
    "no_recoil",
    "no-recoil",
    "aimbot",
    "triggerbot",
    "silentaim",
    "cheat",
];

pub fn is_candidate(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if NAME_KEYWORDS.iter().any(|keyword| name.contains(keyword)) {
        return true;
    }

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if FORCED_EXTENSIONS.contains(&extension.as_str()) {
        return true;
    }

    if extension == "dll" {
        return path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|n| n.to_str())
            .map(|folder| DLL_FOLDERS.iter().any(|f| folder.eq_ignore_ascii_case(f)))
            .unwrap_or(false);
    }

    false
}

pub fn analyze(path: &Path) -> Vec<String> {
    let mut reasons = Vec::new();

    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if let Some(keyword) = NAME_KEYWORDS.iter().find(|k| name.contains(**k)) {
        reasons.push(format!("file name contains \"{keyword}\""));
    }

    if let Some(known) = crate::blocklist::check_file(path) {
        reasons.insert(0, known);
    }

    let data = match read_forced(path) {
        Ok(data) => data,
        Err(error) => {
            reasons.push(format!("could not be opened for inspection ({error})"));
            return reasons;
        }
    };

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    reasons.extend(script_heuristics(&extension, &data));

    let patterns: Vec<&str> = KEYWORDS.iter().map(|(keyword, _)| *keyword).collect();
    let Ok(matcher) = AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .build(&patterns)
    else {
        return reasons;
    };

    let mut seen = [false; KEYWORDS.len()];
    for found in matcher.find_iter(&data) {
        seen[found.pattern().as_usize()] = true;
    }

    for (index, hit) in seen.iter().enumerate() {
        if !*hit || reasons.len() >= MAX_REASONS {
            continue;
        }
        let (keyword, label) = KEYWORDS[index];
        let reason = format!("{label} (\"{keyword}\" found in contents)");
        if !reasons.iter().any(|r| r.starts_with(label)) {
            reasons.push(reason);
        }
    }

    reasons
}

/// Scripts that hide their code: compiled bytecode, remote loaders and packed payloads.
fn script_heuristics(extension: &str, data: &[u8]) -> Vec<String> {
    let mut reasons = Vec::new();
    let is_script = matches!(extension, "lua" | "luac" | "js");
    if !is_script {
        return reasons;
    }

    if data.starts_with(b"\x1bLua") {
        reasons.push("compiled Lua bytecode (source hidden)".to_string());
    }

    if let Ok(loader) = AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .build(["loadstring", "performhttprequest"])
    {
        let mut seen = [false; 2];
        for found in loader.find_iter(data) {
            seen[found.pattern().as_usize()] = true;
        }
        if seen[0] && seen[1] {
            reasons.push("remote script loader (loadstring + PerformHttpRequest)".to_string());
        }
    }

    if data.len() >= ENTROPY_MIN_BYTES {
        let entropy = shannon_entropy(data);
        if entropy >= ENTROPY_THRESHOLD {
            reasons.push(format!(
                "high-entropy content ({entropy:.1}/8), likely encrypted or obfuscated"
            ));
        }
    }

    reasons
}

fn shannon_entropy(data: &[u8]) -> f64 {
    let mut counts = [0usize; 256];
    for byte in data {
        counts[*byte as usize] += 1;
    }
    let total = data.len() as f64;
    counts
        .iter()
        .filter(|count| **count > 0)
        .map(|count| {
            let p = *count as f64 / total;
            -p * p.log2()
        })
        .sum()
}

/// Reads the whole file, or the first and last 32 MB of a very large one.
fn read_forced(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let len = file.metadata().map_err(|error| error.to_string())?.len();

    if len <= FULL_READ_LIMIT {
        let mut data = Vec::with_capacity(len as usize);
        file.read_to_end(&mut data)
            .map_err(|error| error.to_string())?;
        return Ok(data);
    }

    let mut data = vec![0u8; (HALF_READ * 2) as usize];
    file.read_exact(&mut data[..HALF_READ as usize])
        .map_err(|error| error.to_string())?;
    file.seek(SeekFrom::Start(len - HALF_READ))
        .map_err(|error| error.to_string())?;
    file.read_exact(&mut data[HALF_READ as usize..])
        .map_err(|error| error.to_string())?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rage-deep-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_script_contents_and_reports_keywords() {
        let dir = temp_dir("lua");
        let script = dir.join("menu.lua");
        std::fs::write(
            &script,
            b"Citizen.CreateThread(function() SetPedInfiniteAmmoClip(ped, true) -- NoRecoil\nend)",
        )
        .unwrap();

        assert!(is_candidate(&script));
        let reasons = analyze(&script);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(reasons.iter().any(|r| r.starts_with("no recoil")), "{reasons:?}");
        assert!(reasons.iter().any(|r| r.starts_with("infinite ammo")));
    }

    #[test]
    fn flags_suspicious_name_with_any_extension() {
        let dir = temp_dir("name");
        let file = dir.join("My_NoRecoil_v2.txt");
        std::fs::write(&file, b"hello").unwrap();

        assert!(is_candidate(&file));
        let reasons = analyze(&file);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(reasons[0].contains("file name contains"));
    }

    #[test]
    fn finds_plain_text_names_inside_zip_bytes() {
        let dir = temp_dir("zip");
        let archive = dir.join("clouds.zip");
        let mut bytes = b"PK\x03\x04junkjunk".to_vec();
        bytes.extend_from_slice(b"data/pedaccuracy.meta");
        std::fs::write(&archive, bytes).unwrap();

        let reasons = analyze(&archive);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(reasons
            .iter()
            .any(|r| r.starts_with("pedaccuracy.meta reference")));
    }

    #[test]
    fn clean_script_has_no_reasons_and_runtime_dlls_are_skipped() {
        let dir = temp_dir("clean");
        let script = dir.join("hello.lua");
        std::fs::write(&script, b"print('hello')").unwrap();
        assert!(analyze(&script).is_empty());

        let dll = dir.join("citizen-runtime.dll");
        std::fs::write(&dll, b"MZ").unwrap();
        assert!(!is_candidate(&dll));

        let plugins = dir.join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let plugin_dll = plugins.join("thing.dll");
        std::fs::write(&plugin_dll, b"MZ").unwrap();
        assert!(is_candidate(&plugin_dll));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unreadable_file_is_reported() {
        let dir = temp_dir("missing");
        let ghost = dir.join("ghost.asi");
        let reasons = analyze(&ghost);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(reasons[0].contains("could not be opened"));
    }

    #[test]
    fn flags_remote_loader_bytecode_and_packed_scripts() {
        let dir = temp_dir("heur");

        let loader = dir.join("a.lua");
        std::fs::write(
            &loader,
            b"PerformHttpRequest(url, function(c, body) loadstring(body)() end)",
        )
        .unwrap();
        assert!(analyze(&loader)
            .iter()
            .any(|r| r.starts_with("remote script loader")));

        let bytecode = dir.join("b.luac");
        std::fs::write(&bytecode, b"\x1bLuaQ\x00\x01\x04\x08").unwrap();
        assert!(analyze(&bytecode)
            .iter()
            .any(|r| r.starts_with("compiled Lua bytecode")));

        let packed = dir.join("c.lua");
        let noise: Vec<u8> = (0..8192u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        std::fs::write(&packed, noise).unwrap();
        assert!(analyze(&packed)
            .iter()
            .any(|r| r.starts_with("high-entropy content")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn normal_lua_source_is_not_high_entropy() {
        let source = "local function add(a, b) return a + b end\n".repeat(200);
        assert!(shannon_entropy(source.as_bytes()) < ENTROPY_THRESHOLD);
    }
}
