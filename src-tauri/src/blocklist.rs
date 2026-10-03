//! Exact-match blocklist of known cheat files (SHA-256). Zero false positives by design.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

/// Copy of `updates/blocklist.txt`, used when the online list cannot be downloaded.
const BUNDLED_BLOCKLIST: &str = include_str!("../blocklist.txt");
/// Fixed address only; the scanner never downloads from any other URL.
const REMOTE_BLOCKLIST_URL: &str =
    "https://raw.githubusercontent.com/aftlah/fivem-fileChecker/main/updates/blocklist.txt";
const REMOTE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_REMOTE_BYTES: u64 = 512 * 1024;
const MAX_HASH_BYTES: u64 = 256 * 1024 * 1024;

/// Online list, fetched at most once per run. `None` means it was unavailable.
fn remote_blocklist() -> Option<&'static str> {
    static REMOTE: OnceLock<Option<String>> = OnceLock::new();
    REMOTE
        .get_or_init(|| {
            let response = ureq::get(REMOTE_BLOCKLIST_URL)
                .timeout(REMOTE_TIMEOUT)
                .set("User-Agent", "rage-file-checker")
                .call()
                .ok()?;
            let mut text = String::new();
            response
                .into_reader()
                .take(MAX_REMOTE_BYTES)
                .read_to_string(&mut text)
                .ok()?;
            Some(text)
        })
        .as_deref()
}

fn find_label<'a>(list: &'a str, hash: &str) -> Option<&'a str> {
    list.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (known, label) = line.split_once(char::is_whitespace)?;
        known.eq_ignore_ascii_case(hash).then(|| label.trim())
    })
}

fn lookup(hash: &str) -> Option<String> {
    remote_blocklist()
        .and_then(|list| find_label(list, hash))
        .or_else(|| find_label(BUNDLED_BLOCKLIST, hash))
        .map(str::to_string)
}

pub fn sha256_hex(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    if file.metadata().ok()?.len() > MAX_HASH_BYTES {
        return None;
    }

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

/// Returns the label of the known cheat this file is an exact copy of.
pub fn check_file(path: &Path) -> Option<String> {
    let hash = sha256_hex(path)?;
    lookup(&hash).map(|label| format!("KNOWN CHEAT FILE: {label}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_label_matches_listed_hash_and_ignores_comments() {
        let known = "ff0ab1db2cb6270feebac6599d98472dad472e164be6e9f2d2996529cab4e759";
        assert!(find_label(BUNDLED_BLOCKLIST, known)
            .unwrap()
            .contains("norecoil-DDW"));
        assert!(find_label(BUNDLED_BLOCKLIST, &known.to_uppercase()).is_some());
        assert!(find_label(BUNDLED_BLOCKLIST, "0000").is_none());
        assert!(find_label(BUNDLED_BLOCKLIST, "#").is_none());
    }

    #[test]
    fn find_label_reads_a_downloaded_list() {
        let list = "# header\n\nabc123  Some Cheat (v2)\n";
        assert_eq!(find_label(list, "ABC123"), Some("Some Cheat (v2)"));
        assert_eq!(find_label(list, "def456"), None);
    }

    #[test]
    fn hashes_a_file() {
        let path = std::env::temp_dir().join(format!("rage-hash-{}.bin", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        let hash = sha256_hex(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            hash.as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }
}
