use flate2::read::DeflateDecoder;
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

const RPF7_MAGIC: u32 = 0x5250_4637;
const ENC_OPEN: u32 = 0x4E45_504F;
const ENC_AES: u32 = 0x0FFF_FFF9;
const ENC_NG: u32 = 0x0FEF_FFFF;
const DIRECTORY_MARKER: u32 = 0x7FFF_FF00;
const RESOURCE_SIZE_MARKER: u32 = 0x00FF_FFFF;
const BLOCK_SIZE: u64 = 512;
const HEADER_SIZE: u64 = 16;
const ENTRY_SIZE: u64 = 16;

const MAX_TOC_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 192 * 1024 * 1024;
const MAX_NESTED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_NESTING_DEPTH: u32 = 3;
const MAX_REASONS: usize = 8;

const SUSPICIOUS_NAMES: [&str; 6] = [
    "pedaccuracy.meta",
    "weapons.meta",
    "weaponcomponents.meta",
    "weaponanimations.meta",
    "handling.meta",
    "vehicles.meta",
];

struct Signature {
    needle: &'static [u8],
    expected_name: &'static str,
    label: &'static str,
}

const SIGNATURES: [Signature; 6] = [
    Signature {
        needle: b"<sPedAccuracyModifiers",
        expected_name: "pedaccuracy.meta",
        label: "ped accuracy data",
    },
    Signature {
        needle: b"PLAYER_RECOIL_MODIFIER",
        expected_name: "pedaccuracy.meta",
        label: "player recoil modifiers",
    },
    Signature {
        needle: b"<CWeaponInfoBlob",
        expected_name: "weapons.meta",
        label: "weapon data",
    },
    Signature {
        needle: b"<CWeaponComponentInfoBlob",
        expected_name: "weaponcomponents.meta",
        label: "weapon component data",
    },
    Signature {
        needle: b"<CHandlingDataMgr",
        expected_name: "handling.meta",
        label: "vehicle handling data",
    },
    Signature {
        needle: b"<CVehicleModelInfo__InitDataList",
        expected_name: "vehicles.meta",
        label: "vehicle model data",
    },
];

/// Archives that FiveM and the Rockstar launcher ship themselves. They are not plain RPF7
/// files (or are encrypted), which would otherwise be reported as disguised or unreadable.
const OFFICIAL_ARCHIVES: [&str; 2] = [
    "citizen/streaming_surrogate.rpf",
    "data/game-storage/launcher/launcher.rpf",
];

/// True when `relative` is an official archive and the only findings are structural
/// (invalid or encrypted). Any content finding still gets reported.
pub fn is_official_archive_noise(relative: &str, reasons: &[String]) -> bool {
    let relative = relative.replace('\\', "/").to_ascii_lowercase();
    OFFICIAL_ARCHIVES.contains(&relative.as_str())
        && reasons.iter().all(|reason| {
            reason.starts_with("not a valid RPF7") || reason.contains("cannot be inspected")
        })
}

pub fn is_rpf_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("rpf"))
        .unwrap_or(false)
}

/// Returns the reasons an archive looks suspicious, or an empty list when it looks clean.
pub fn analyze_rpf(path: &Path) -> Vec<String> {
    let known = crate::blocklist::check_file(path);

    let mut reasons = match analyze_inner(path) {
        Ok(reasons) => reasons,
        Err(message) => vec![message],
    };
    if let Some(known) = known {
        reasons.insert(0, known);
    }
    reasons
}

fn analyze_inner(path: &Path) -> Result<Vec<String>, String> {
    let mut file = File::open(path).map_err(|error| format!("unreadable archive ({error})"))?;
    let file_len = file
        .metadata()
        .map_err(|error| format!("unreadable archive ({error})"))?
        .len();
    analyze_reader(&mut file, file_len, 0)
}

fn analyze_reader<R: Read + Seek>(
    file: &mut R,
    file_len: u64,
    depth: u32,
) -> Result<Vec<String>, String> {
    let mut header = [0u8; HEADER_SIZE as usize];
    if file.read_exact(&mut header).is_err() || read_u32(&header, 0) != RPF7_MAGIC {
        return Err("not a valid RPF7 archive (disguised file?)".to_string());
    }

    let entry_count = read_u32(&header, 4) as u64;
    let names_len = read_u32(&header, 8) as u64;
    let encryption = read_u32(&header, 12);

    if encryption == ENC_AES || encryption == ENC_NG {
        return Err("encrypted RPF, contents cannot be inspected".to_string());
    }
    if encryption != ENC_OPEN {
        return Err("RPF with unknown encryption, contents cannot be inspected".to_string());
    }

    let toc_len = entry_count * ENTRY_SIZE + names_len;
    if toc_len > MAX_TOC_BYTES || HEADER_SIZE + toc_len > file_len {
        return Err("corrupt RPF table of contents".to_string());
    }

    let mut toc = vec![0u8; toc_len as usize];
    file.read_exact(&mut toc)
        .map_err(|_| "corrupt RPF table of contents".to_string())?;
    let (entries, names) = toc.split_at((entry_count * ENTRY_SIZE) as usize);

    let mut reasons: Vec<String> = Vec::new();
    let mut obfuscated = false;
    let mut total_read = 0u64;

    for index in 0..entry_count as usize {
        let entry = &entries[index * ENTRY_SIZE as usize..(index + 1) * ENTRY_SIZE as usize];
        let is_directory = read_u32(entry, 4) == DIRECTORY_MARKER;
        let name_offset = if is_directory {
            read_u32(entry, 0) as usize
        } else {
            u16::from_le_bytes([entry[0], entry[1]]) as usize
        };
        let name = read_name(names, name_offset);

        if index > 0 && !is_valid_name(&name) {
            obfuscated = true;
        }
        if is_directory {
            continue;
        }

        let name_lower = name.to_ascii_lowercase();
        if SUSPICIOUS_NAMES.contains(&name_lower.as_str()) {
            push_reason(&mut reasons, format!("contains {name_lower}"));
        }

        let size = u32::from_le_bytes([entry[2], entry[3], entry[4], 0]);
        let block = u32::from_le_bytes([entry[5], entry[6], entry[7], 0]) & 0x007F_FFFF;
        let uncompressed = read_u32(entry, 8) as u64;
        let entry_encryption = read_u32(entry, 12);

        if size == RESOURCE_SIZE_MARKER || entry_encryption == ENC_AES || entry_encryption == ENC_NG
        {
            continue;
        }

        let is_nested_rpf = name_lower.ends_with(".rpf");
        let entry_limit = if is_nested_rpf {
            MAX_NESTED_BYTES
        } else {
            MAX_ENTRY_BYTES
        };
        let stored_len = if size == 0 { uncompressed } else { size as u64 };
        let offset = block as u64 * BLOCK_SIZE;
        if stored_len == 0
            || stored_len > entry_limit
            || offset + stored_len > file_len
            || total_read + stored_len > MAX_TOTAL_BYTES
        {
            continue;
        }

        let Some(data) = read_entry(file, offset, stored_len, size != 0) else {
            continue;
        };
        total_read += stored_len;

        if is_nested_rpf {
            push_reason(&mut reasons, format!("nested archive {name_lower}"));
            if depth < MAX_NESTING_DEPTH {
                let nested_len = data.len() as u64;
                match analyze_reader(&mut Cursor::new(data), nested_len, depth + 1) {
                    Ok(inner) => {
                        for reason in inner {
                            let reason = format!("inside {name_lower}: {reason}");
                            if reason.contains("NO RECOIL") {
                                if !reasons.contains(&reason) {
                                    reasons.insert(0, reason);
                                }
                            } else {
                                push_reason(&mut reasons, reason);
                            }
                        }
                    }
                    Err(message) => {
                        push_reason(&mut reasons, format!("inside {name_lower}: {message}"));
                    }
                }
            }
            continue;
        }

        if name_lower == "assembly.xml" {
            push_reason(&mut reasons, "OpenIV package (assembly.xml)".to_string());
            let text = String::from_utf8_lossy(&data).to_ascii_lowercase();
            if text.contains("common\\data") || text.contains("common/data") {
                push_reason(
                    &mut reasons,
                    "package overwrites common/data game files".to_string(),
                );
            }
        }

        if let Some(recoil) = find_recoil_hack(&data) {
            let reason = format!("NO RECOIL: {recoil} in {name_lower}");
            if !reasons.contains(&reason) {
                reasons.insert(0, reason);
            }
        }

        for signature in &SIGNATURES {
            if !contains_bytes(&data, signature.needle) {
                continue;
            }
            if name_lower == signature.expected_name {
                push_reason(&mut reasons, format!("{} in {name_lower}", signature.label));
            } else {
                push_reason(
                    &mut reasons,
                    format!("{} disguised as {name_lower}", signature.label),
                );
            }
        }
    }

    if obfuscated {
        push_reason(&mut reasons, "obfuscated entry names".to_string());
    }

    Ok(reasons)
}

fn push_reason(reasons: &mut Vec<String>, reason: String) {
    if reasons.len() < MAX_REASONS && !reasons.contains(&reason) {
        reasons.push(reason);
    }
}

fn read_entry<R: Read + Seek>(file: &mut R, offset: u64, stored_len: u64, compressed: bool) -> Option<Vec<u8>> {
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut raw = vec![0u8; stored_len as usize];
    file.read_exact(&mut raw).ok()?;

    if !compressed {
        return Some(raw);
    }

    let mut data = Vec::new();
    DeflateDecoder::new(raw.as_slice())
        .take(MAX_ENTRY_BYTES)
        .read_to_end(&mut data)
        .ok()?;
    Some(data)
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn read_name(names: &[u8], offset: usize) -> String {
    let Some(tail) = names.get(offset..) else {
        return "\u{fffd}".to_string();
    };
    let end = tail.iter().position(|byte| *byte == 0).unwrap_or(tail.len());
    String::from_utf8_lossy(&tail[..end]).into_owned()
}

fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| !c.is_control() && c != '\u{fffd}' && !"<>:\"|?*".contains(c))
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    find_bytes(haystack, needle).is_some()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

const RECOIL_HACK_THRESHOLD: f32 = -1.0;

pub fn find_recoil_hack(data: &[u8]) -> Option<String> {
    for key in ["PLAYER_RECOIL_MODIFIER_MIN", "PLAYER_RECOIL_MODIFIER_MAX"] {
        let Some(position) = find_bytes(data, key.as_bytes()) else {
            continue;
        };
        let start = position + key.len();
        let tail = String::from_utf8_lossy(&data[start..data.len().min(start + 64)]).into_owned();
        let Some(value_at) = tail.find("value=\"") else {
            continue;
        };
        let raw = &tail[value_at + 7..];
        let Some(end) = raw.find('"') else {
            continue;
        };
        if let Ok(value) = raw[..end].trim().parse::<f32>() {
            if value < RECOIL_HACK_THRESHOLD {
                return Some(format!("{key}={value}"));
            }
        }
    }
    None
}

const LOOSE_EXTENSIONS: [&str; 6] = ["meta", "xml", "dat", "ymt", "txt", "cfg"];
const MAX_LOOSE_BYTES: u64 = 2 * 1024 * 1024;

pub fn is_loose_data_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| LOOSE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn analyze_loose_file(path: &Path) -> Vec<String> {
    let mut reasons = Vec::new();

    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return reasons;
    };
    let name_lower = name.to_ascii_lowercase();
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !LOOSE_EXTENSIONS.contains(&extension.as_str()) {
        return reasons;
    }

    let Ok(metadata) = std::fs::metadata(path) else {
        return reasons;
    };
    if metadata.len() == 0 || metadata.len() > MAX_LOOSE_BYTES {
        return reasons;
    }
    let Ok(data) = std::fs::read(path) else {
        return reasons;
    };

    if let Some(recoil) = find_recoil_hack(&data) {
        reasons.push(format!("NO RECOIL: {recoil}"));
    }

    for signature in &SIGNATURES {
        if name_lower != signature.expected_name && contains_bytes(&data, signature.needle) {
            push_reason(
                &mut reasons,
                format!("{} disguised as {name_lower}", signature.label),
            );
        }
    }

    if name_lower == "assembly.xml"
        && contains_bytes(&data, b"<package")
        && contains_bytes(&data, b"target=\"Five\"")
    {
        push_reason(&mut reasons, "OpenIV package (assembly.xml)".to_string());
    }

    reasons
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct TestEntry<'a> {
        name: &'a str,
        data: &'a [u8],
    }

    fn build_rpf(entries: &[TestEntry]) -> Vec<u8> {
        let mut names = vec![0u8];
        let mut name_offsets = Vec::new();
        for entry in entries {
            name_offsets.push(names.len() as u16);
            names.extend_from_slice(entry.name.as_bytes());
            names.push(0);
        }

        let count = entries.len() as u32 + 1;
        let toc_len = HEADER_SIZE + count as u64 * ENTRY_SIZE + names.len() as u64;
        let mut next_block = toc_len.div_ceil(BLOCK_SIZE);

        let mut out = Vec::new();
        out.extend_from_slice(&RPF7_MAGIC.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&(names.len() as u32).to_le_bytes());
        out.extend_from_slice(&ENC_OPEN.to_le_bytes());

        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&DIRECTORY_MARKER.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(entries.len() as u32).to_le_bytes());

        let mut data_section = Vec::new();
        for (entry, name_offset) in entries.iter().zip(&name_offsets) {
            out.extend_from_slice(&name_offset.to_le_bytes());
            out.extend_from_slice(&[0, 0, 0]);
            out.extend_from_slice(&next_block.to_le_bytes()[..3]);
            out.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());

            data_section.extend_from_slice(entry.data);
            let padded = (entry.data.len() as u64).div_ceil(BLOCK_SIZE) * BLOCK_SIZE;
            data_section.resize(data_section.len() + (padded as usize - entry.data.len()), 0);
            next_block += padded / BLOCK_SIZE;
        }

        out.extend_from_slice(&names);
        out.resize(toc_len.div_ceil(BLOCK_SIZE) as usize * BLOCK_SIZE as usize, 0);
        out.extend_from_slice(&data_section);
        out
    }

    fn write_temp(label: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rage-rpf-{label}-{}-{}.rpf",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        File::create(&path).unwrap().write_all(bytes).unwrap();
        path
    }

    #[test]
    fn flags_ped_accuracy_disguised_as_carcols() {
        let rpf = build_rpf(&[
            TestEntry {
                name: "assembly.xml",
                data: br#"<add source="a">common\data\ai\pedaccuracy.meta</add>"#,
            },
            TestEntry {
                name: "carcols.meta",
                data: b"<sPedAccuracyModifiers><PLAYER_RECOIL_MODIFIER_MIN value=\"-22\"/>",
            },
        ]);
        let path = write_temp("disguised", &rpf);
        let reasons = analyze_rpf(&path);
        let _ = std::fs::remove_file(&path);

        assert!(reasons.iter().any(|r| r.contains("OpenIV package")));
        assert!(reasons.iter().any(|r| r.contains("common/data")));
        assert!(reasons
            .iter()
            .any(|r| r == "ped accuracy data disguised as carcols.meta"));
    }

    #[test]
    fn flags_known_game_data_names() {
        let rpf = build_rpf(&[TestEntry {
            name: "weapons.meta",
            data: b"<CWeaponInfoBlob>",
        }]);
        let path = write_temp("named", &rpf);
        let reasons = analyze_rpf(&path);
        let _ = std::fs::remove_file(&path);

        assert!(reasons.iter().any(|r| r == "contains weapons.meta"));
        assert!(reasons.iter().any(|r| r == "weapon data in weapons.meta"));
    }

    #[test]
    fn ignores_clean_archive() {
        let rpf = build_rpf(&[TestEntry {
            name: "readme.txt",
            data: b"just a texture pack",
        }]);
        let path = write_temp("clean", &rpf);
        let reasons = analyze_rpf(&path);
        let _ = std::fs::remove_file(&path);

        assert!(reasons.is_empty(), "unexpected reasons: {reasons:?}");
    }

    #[test]
    fn flags_obfuscated_entry_names() {
        let rpf = build_rpf(&[TestEntry {
            name: "z><;;",
            data: b"x",
        }]);
        let path = write_temp("obfuscated", &rpf);
        let reasons = analyze_rpf(&path);
        let _ = std::fs::remove_file(&path);

        assert!(reasons.iter().any(|r| r == "obfuscated entry names"));
    }

    #[test]
    fn flags_encrypted_and_fake_archives() {
        let mut encrypted = build_rpf(&[]);
        encrypted[12..16].copy_from_slice(&ENC_AES.to_le_bytes());
        let path = write_temp("aes", &encrypted);
        let reasons = analyze_rpf(&path);
        let _ = std::fs::remove_file(&path);
        assert!(reasons.iter().any(|r| r.contains("encrypted")));

        let path = write_temp("fake", b"this is not an rpf at all");
        let reasons = analyze_rpf(&path);
        let _ = std::fs::remove_file(&path);
        assert!(reasons.iter().any(|r| r.contains("not a valid RPF7")));
    }

    #[test]
    fn labels_no_recoil_values_first() {
        let rpf = build_rpf(&[TestEntry {
            name: "pedaccuracy.meta",
            data: b"<sPedAccuracyModifiers>
  <PLAYER_RECOIL_MODIFIER_MIN value=\"-20.800000\" />",
        }]);
        let path = write_temp("recoil", &rpf);
        let reasons = analyze_rpf(&path);
        let _ = std::fs::remove_file(&path);

        assert!(
            reasons[0].starts_with("NO RECOIL: PLAYER_RECOIL_MODIFIER_MIN=-20.8"),
            "got {reasons:?}"
        );
    }

    #[test]
    fn recoil_check_ignores_normal_values() {
        assert!(find_recoil_hack(b"<PLAYER_RECOIL_MODIFIER_MIN value=\"1.000000\" />").is_none());
        assert!(find_recoil_hack(b"<PLAYER_RECOIL_MODIFIER_MIN value=\"-0.5\" />").is_none());
        assert!(find_recoil_hack(b"<PLAYER_RECOIL_MODIFIER_MAX value=\"-22\" />").is_some());
    }

    #[test]
    fn loose_file_scan_flags_recoil_and_disguise() {
        let dir = std::env::temp_dir().join(format!("rage-loose-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let recoil = dir.join("pedaccuracy.meta");
        std::fs::write(
            &recoil,
            b"<sPedAccuracyModifiers><PLAYER_RECOIL_MODIFIER_MIN value=\"-22.8\" />",
        )
        .unwrap();
        let disguised = dir.join("carcols.meta");
        std::fs::write(&disguised, b"<sPedAccuracyModifiers></sPedAccuracyModifiers>").unwrap();
        let clean_dir = dir.join("official");
        std::fs::create_dir_all(&clean_dir).unwrap();
        let clean = clean_dir.join("pedaccuracy.meta");
        std::fs::write(
            &clean,
            b"<sPedAccuracyModifiers><PLAYER_RECOIL_MODIFIER_MIN value=\"1.0\" />",
        )
        .unwrap();

        let recoil_reasons = analyze_loose_file(&recoil);
        let disguised_reasons = analyze_loose_file(&disguised);
        let clean_reasons = analyze_loose_file(&clean);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(recoil_reasons[0].starts_with("NO RECOIL"));
        assert!(disguised_reasons
            .iter()
            .any(|r| r == "ped accuracy data disguised as carcols.meta"));
        assert!(clean_reasons.is_empty());
    }

    #[test]
    fn inspects_rpf_nested_inside_rpf() {
        let inner = build_rpf(&[TestEntry {
            name: "pedaccuracy.meta",
            data: b"<sPedAccuracyModifiers><PLAYER_RECOIL_MODIFIER_MIN value=\"-30\" />",
        }]);
        let outer = build_rpf(&[TestEntry {
            name: "textures.rpf",
            data: &inner,
        }]);
        let path = write_temp("nested", &outer);
        let reasons = analyze_rpf(&path);
        let _ = std::fs::remove_file(&path);

        assert!(
            reasons[0].starts_with("inside textures.rpf: NO RECOIL"),
            "got {reasons:?}"
        );
        assert!(reasons.iter().any(|r| r == "nested archive textures.rpf"));
    }

    #[test]
    fn official_archives_are_ignored_only_when_findings_are_structural() {
        let invalid = vec!["not a valid RPF7 archive (disguised file?)".to_string()];
        let encrypted = vec!["RPF with unknown encryption, contents cannot be inspected".to_string()];
        let cheat = vec!["NO RECOIL: PLAYER_RECOIL_MODIFIER_MIN=-22 in pedaccuracy.meta".to_string()];

        assert!(is_official_archive_noise("citizen/streaming_surrogate.rpf", &invalid));
        assert!(is_official_archive_noise(
            "Data/Game-Storage/Launcher/Launcher.rpf",
            &encrypted
        ));
        assert!(!is_official_archive_noise("citizen/streaming_surrogate.rpf", &cheat));
        assert!(!is_official_archive_noise("mods/clouds.rpf", &invalid));
    }
}
