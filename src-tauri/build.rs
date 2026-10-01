fn main() {
    embed_app_name();
    embed_discord_webhook();
    tauri_build::build();
}

fn embed_discord_webhook() {
    println!("cargo:rerun-if-env-changed=DISCORD_WEBHOOK_URL");

    if let Ok(url) = std::env::var("DISCORD_WEBHOOK_URL") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            println!("cargo:rustc-env=DISCORD_WEBHOOK_URL={trimmed}");
        }
    }
}

fn embed_app_name() {
    println!("cargo:rerun-if-env-changed=APP_DISPLAY_NAME");
    let name = std::env::var("APP_DISPLAY_NAME").unwrap_or_default();
    let name = name.trim();
    let name = if name.is_empty() { "RAGE File Scanner" } else { name };
    println!("cargo:rustc-env=APP_DISPLAY_NAME={name}");
}
