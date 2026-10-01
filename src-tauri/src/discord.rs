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

fn configured_webhook() -> Result<String, String> {
    // Injected at compile time from DISCORD_WEBHOOK_URL / .discord.env (never from the client).
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

fn build_fields(report: &DiscordReport, player_name: &str, scanned_at: &str) -> Vec<Value> {
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

    let found = format_found_list(report);
    fields.push(field(
        if found.is_empty() {
            "File yang ketemu".to_string()
        } else {
            format!("File yang ketemu ({})", report.detected)
        },
        if found.is_empty() {
            "Tidak ada."
        } else {
            found.as_str()
        },
        false,
    ));

    let clean = format_clean_list(report);
    if !clean.is_empty() {
        fields.push(field(
            format!("Tidak ketemu ({})", report.not_detected),
            &clean,
            false,
        ));
    }

    fields
}

fn format_found_list(report: &DiscordReport) -> String {
    let mut items: Vec<&DiscordResultItem> = report
        .results
        .iter()
        .filter(|item| item.status != "NOT_FOUND")
        .collect();
    items.sort_by(|left, right| left.name.cmp(&right.name));

    if items.is_empty() {
        return String::new();
    }

    let lines: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(index, item)| format_found_line(index + 1, item))
        .collect();

    truncate(&lines.join("\n"), FIELD_LIMIT)
}

fn format_found_line(number: usize, item: &DiscordResultItem) -> String {
    if item.status == "ERROR" {
        return format!("`{number}.` {} — gagal dibaca", item.name);
    }

    if item.found_files.is_empty() {
        return format!("`{number}.` {}", item.name);
    }

    let extra = item.found_files.len().saturating_sub(1);
    if extra == 0 {
        format!("`{number}.` {} — {}", item.name, item.found_files[0])
    } else {
        format!(
            "`{number}.` {} — {} (+{} file lain)",
            item.name, item.found_files[0], extra
        )
    }
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
