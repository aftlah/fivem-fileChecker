use chrono::Local;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const DISCORD_COOLDOWN: Duration = Duration::from_secs(120);
static LAST_DISCORD_REPORTS: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiscordResultItem {
    pub name: String,
    pub status: String,
    pub relative_path: String,
    #[serde(default)]
    pub found_files: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiscordReport {
    pub player_name: String,
    pub five_m_path: String,
    pub overall_status: String,
    pub detected: u32,
    pub not_detected: u32,
    pub errors: u32,
    pub results: Vec<DiscordResultItem>,
}

const FIELD_LIMIT: usize = 1024;
const MAX_FIELDS: usize = 25;
const EMBED_CHAR_BUDGET: usize = 5500;

fn configured_webhook() -> Result<String, String> {
    match option_env!("DISCORD_WEBHOOK_URL") {
        Some(url) if !url.trim().is_empty() => Ok(url.trim().to_string()),
        _ => Err(
            "Discord webhook is not configured in this build. Set DISCORD_WEBHOOK_URL or .discord.env before building."
                .to_string(),
        ),
    }
}

pub fn send_scan_report(report: DiscordReport) -> Result<(), String> {
    let webhook = configured_webhook()?;
    if !is_discord_webhook(&webhook) {
        return Err("Discord webhook URL is invalid.".to_string());
    }

    let player_name = report.player_name.trim().to_string();
    if player_name.is_empty() {
        return Err("A name is required before sending to Discord.".to_string());
    }

    enforce_discord_cooldown(&player_name)?;

    let (title, description, color) = match report.overall_status.as_str() {
        "DETECTED" => (
            "Ada file mencurigakan",
            format!("Scan **{player_name}** menemukan file yang tidak seharusnya ada."),
            0xEF_44_44,
        ),
        "WARNING" => (
            "Ada peringatan",
            format!("Scan **{player_name}** selesai dengan peringatan."),
            0xEA_B3_08,
        ),
        _ => (
            "Tidak ada file mencurigakan",
            format!("Scan **{player_name}** bersih."),
            0x22_C5_5E,
        ),
    };

    let scanned_at = scan_time_wib();

    let payload = json!({
        "username": crate::APP_NAME,
        "embeds": [{
            "author": { "name": crate::APP_NAME },
            "title": title,
            "description": description,
            "color": color,
            "timestamp": scan_timestamp_iso(),
            "fields": build_fields(&report, &player_name, &scanned_at),
            "footer": { "text": "Scan read-only · file tidak diubah" }
        }]
    });

    ureq::post(&webhook)
        .set("Content-Type", "application/json")
        .send_json(payload)
        .map_err(|error| format!("Failed to send Discord report: {error}"))?;

    record_discord_report(&player_name);
    Ok(())
}

fn reclassify_archive_hits(report: &DiscordReport) -> DiscordReport {
    let mut adjusted = report.clone();

    for index in 0..adjusted.results.len() {
        let target = adjusted.results[index]
            .relative_path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .to_lowercase();
        if adjusted.results[index].status != "NOT_FOUND" || !target.ends_with(".meta") {
            continue;
        }

        let hits: Vec<String> = report
            .results
            .iter()
            .filter(|other| other.status != "NOT_FOUND")
            .flat_map(|other| other.found_files.iter())
            .filter(|file| file.to_lowercase().contains(&target))
            .map(|file| {
                let archive = file.split(" (").next().unwrap_or(file);
                format!("{archive} (berisi {target})")
            })
            .collect();

        if !hits.is_empty() {
            let item = &mut adjusted.results[index];
            item.status = "DETECTED".to_string();
            item.found_files = hits;
        }
    }

    let moved = adjusted
        .results
        .iter()
        .zip(&report.results)
        .filter(|(after, before)| after.status != before.status)
        .count() as u32;
    adjusted.detected += moved;
    adjusted.not_detected = adjusted.not_detected.saturating_sub(moved);
    adjusted
}

fn build_fields(report: &DiscordReport, player_name: &str, scanned_at: &str) -> Vec<Value> {
    let report = &reclassify_archive_hits(report);
    let mut fields = vec![
        field("Nama karakter", player_name, true),
        field("Waktu scan", scanned_at, true),
        field(
            "Hasil",
            &format!("{} ketemu · {} aman", report.detected, report.not_detected),
            true,
        ),
        field(
            "Lokasi FiveM",
            &format!("```\n{}\n```", report.five_m_path),
            false,
        ),
    ];

    if report.errors > 0 {
        fields.push(field("Error", &report.errors.to_string(), true));
    }

    let clean = format_clean_list(report);
    let clean_cost = if clean.is_empty() { 0 } else { clean.chars().count() + 40 };
    let fixed_cost: usize = fields.iter().map(field_size).sum();
    let mut budget = EMBED_CHAR_BUDGET.saturating_sub(fixed_cost + clean_cost);
    let mut slots = MAX_FIELDS
        .saturating_sub(fields.len() + usize::from(!clean.is_empty()) + 1);

    let mut items: Vec<&DiscordResultItem> = report
        .results
        .iter()
        .filter(|item| item.status != "NOT_FOUND")
        .collect();
    items.sort_by(|left, right| left.name.cmp(&right.name));

    if items.is_empty() {
        fields.push(field("File yang ketemu", "Tidak ada.", false));
    }

    let mut skipped_files = 0usize;
    for item in items {
        let total = item.found_files.len();
        let heading = if item.status == "ERROR" {
            format!("{} — gagal dibaca", item.name)
        } else if total == 0 {
            item.name.clone()
        } else {
            format!("{} ({} file)", item.name, total)
        };

        let chunks = if item.status == "ERROR" || total == 0 {
            vec!["—".to_string()]
        } else {
            chunk_lines(&item.found_files)
        };

        for (index, chunk) in chunks.iter().enumerate() {
            let name = if index == 0 {
                heading.clone()
            } else {
                format!("{} (lanjutan)", item.name)
            };
            let cost = name.chars().count() + chunk.chars().count();
            if slots == 0 || cost > budget {
                skipped_files += chunk.lines().count();
                continue;
            }
            slots -= 1;
            budget -= cost;
            fields.push(field(name, chunk, false));
        }
    }

    if skipped_files > 0 {
        fields.push(field(
            "Tidak muat di Discord",
            &format!("+{skipped_files} file lain — lihat detail lengkap di aplikasi."),
            false,
        ));
    }

    if !clean.is_empty() {
        fields.push(field(
            format!("Tidak ketemu ({})", report.not_detected),
            &clean,
            false,
        ));
    }

    fields
}

fn chunk_lines(files: &[String]) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for file in files {
        let line = truncate(&format!("• {file}"), FIELD_LIMIT);
        if !current.is_empty() && current.chars().count() + 1 + line.chars().count() > FIELD_LIMIT {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(&line);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn field_size(value: &Value) -> usize {
    ["name", "value"]
        .iter()
        .filter_map(|key| value.get(*key).and_then(Value::as_str))
        .map(|text| text.chars().count())
        .sum()
}

fn format_clean_list(report: &DiscordReport) -> String {
    let names: Vec<&str> = report
        .results
        .iter()
        .filter(|item| item.status == "NOT_FOUND")
        .map(|item| item.name.as_str())
        .collect();

    if names.is_empty() {
        return String::new();
    }

    truncate(&names.join(" · "), FIELD_LIMIT)
}

fn enforce_discord_cooldown(player_name: &str) -> Result<(), String> {
    let mut guard = LAST_DISCORD_REPORTS
        .lock()
        .map_err(|_| "Discord cooldown check failed.".to_string())?;

    let reports = guard.get_or_insert_with(HashMap::new);

    if let Some(last_sent) = reports.get(player_name) {
        let elapsed = Instant::now().duration_since(*last_sent);
        if elapsed < DISCORD_COOLDOWN {
            let remaining = DISCORD_COOLDOWN - elapsed;
            let seconds = remaining.as_secs().max(1);
            return Err(format!(
                "Scan terlalu cepat. Tunggu {seconds} detik sebelum kirim lagi."
            ));
        }
    }

    Ok(())
}

fn record_discord_report(player_name: &str) {
    if let Ok(mut guard) = LAST_DISCORD_REPORTS.lock() {
        let reports = guard.get_or_insert_with(HashMap::new);
        reports.insert(player_name.to_string(), Instant::now());
    }
}

fn scan_timestamp_iso() -> String {
    Local::now().to_rfc3339()
}

fn scan_time_wib() -> String {
    Local::now().format("%d %b %Y, %H:%M WIB").to_string()
}

fn field(name: impl Into<String>, value: &str, inline: bool) -> Value {
    let text = if value.trim().is_empty() { "—" } else { value };
    json!({
        "name": name.into(),
        "value": truncate(text, FIELD_LIMIT),
        "inline": inline
    })
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }

    let keep = max.saturating_sub(1);
    let mut truncated: String = value.chars().take(keep).collect();
    truncated.push('…');
    truncated
}

fn is_discord_webhook(url: &str) -> bool {
    url.starts_with("https://discord.com/api/webhooks/")
        || url.starts_with("https://discordapp.com/api/webhooks/")
        || url.starts_with("https://canary.discord.com/api/webhooks/")
}
