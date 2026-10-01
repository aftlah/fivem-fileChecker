import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const REPO = "aftlah/rage-fivem-fileChecker";

export function loadEnvFile(filePath) {
  if (!fs.existsSync(filePath)) return;
  for (const line of fs.readFileSync(filePath, "utf8").split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq === -1) continue;
    const key = trimmed.slice(0, eq).trim();
    const value = trimmed.slice(eq + 1).trim();
    if (key && process.env[key] == null) process.env[key] = value;
  }
}

/** Removes a `--tenant=<id>` / `--tenant <id>` flag from argv and returns [id, remainingArgs]. */
export function extractTenantArg(argv) {
  const rest = [];
  let id = process.env.TENANT;
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg.startsWith("--tenant=")) id = arg.slice("--tenant=".length);
    else if (arg === "--tenant") id = argv[++i];
    else rest.push(arg);
  }
  return [id || "rage", rest];
}

export function loadTenant(id) {
  const file = path.join(root, "tenants", id, "tenant.json");
  if (!fs.existsSync(file)) {
    const known = fs.readdirSync(path.join(root, "tenants")).join(", ");
    console.error(`Unknown tenant "${id}". Available: ${known}`);
    process.exit(1);
  }
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

export function releaseTag(tenant, version) {
  return `${tenant.id}-v${version}`;
}

export function updaterEndpoint(tenant) {
  return `https://raw.githubusercontent.com/${REPO}/main/updates/${tenant.id}/latest.json`;
}

export function releaseAssetUrl(tenant, version, assetName) {
  return `https://github.com/${REPO}/releases/download/${releaseTag(tenant, version)}/${encodeURIComponent(assetName)}`;
}

/**
 * Sets the env vars the frontend/Rust build reads and writes a Tauri config
 * override for the tenant. Returns the override path to pass as `--config`.
 */
export function prepareTenantBuild(tenant) {
  process.env.TENANT = tenant.id;
  process.env.APP_DISPLAY_NAME = tenant.productName;
  // Never inherit a webhook from the shell or another tenant.
  delete process.env.DISCORD_WEBHOOK_URL;
  loadEnvFile(path.join(root, `.discord.${tenant.id}.env`));

  const base = JSON.parse(
    fs.readFileSync(path.join(root, "src-tauri", "tauri.conf.json"), "utf8"),
  );
  const override = {
    productName: tenant.productName,
    identifier: tenant.identifier,
    app: {
      windows: base.app.windows.map((w) => ({ ...w, title: tenant.productName })),
    },
    build: { devUrl: `http://localhost:${tenant.devPort ?? 1420}` },
    bundle: { shortDescription: tenant.shortDescription, icon: tenant.icons },
    plugins: { updater: { endpoints: [updaterEndpoint(tenant)] } },
  };

  const outPath = path.join(root, "src-tauri", `tauri.tenant.${tenant.id}.conf.json`);
  fs.writeFileSync(outPath, `${JSON.stringify(override, null, 2)}\n`, "utf8");
  return outPath;
}
