import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { root, extractTenantArg, loadTenant, prepareTenantBuild } from "./tenant.mjs";

const cargoBin = path.join(os.homedir(), ".cargo", "bin");
const cargoFile = process.platform === "win32" ? "cargo.exe" : "cargo";

if (!fs.existsSync(path.join(cargoBin, cargoFile))) {
  console.error(
    "Rust/Cargo was not found. Install it from https://rustup.rs then open a new terminal.",
  );
  process.exit(1);
}

const pathParts = (process.env.PATH ?? "").split(path.delimiter).filter(Boolean);
if (!pathParts.includes(cargoBin)) {
  process.env.PATH = `${cargoBin}${path.delimiter}${process.env.PATH ?? ""}`;
}

const tauriJs = path.join(
  process.cwd(),
  "node_modules",
  "@tauri-apps",
  "cli",
  "tauri.js",
);

const [tenantId, tauriArgs] = extractTenantArg(process.argv.slice(2));
const tenant = loadTenant(tenantId);
const configPath = prepareTenantBuild(tenant);
if (["dev", "build"].includes(tauriArgs[0])) {
  tauriArgs.splice(1, 0, "--config", configPath);
}
if (tauriArgs[0] === "dev") {
  // Own port (tenant.json devPort) and build dir so several tenants can run side by side.
  process.env.CARGO_TARGET_DIR ??= path.join(root, "src-tauri", "target", "tenants", tenant.id);
}
console.log(`Tenant: ${tenant.id} (${tenant.productName})`);

const child = spawn(process.execPath, [tauriJs, ...tauriArgs], {
  stdio: "inherit",
  env: process.env,
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.exit(1);
  }
  process.exit(code ?? 1);
});
