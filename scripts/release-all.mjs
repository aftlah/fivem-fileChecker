import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { loadTenant, releaseTag, root } from "./tenant.mjs";

// Builds every tenant at the same version and gathers everything to upload in one folder.
// Usage: npm run release:all            (all tenants in tenants/)
//        npm run release:all -- rage dtf (only these)
const requested = process.argv.slice(2).filter((arg) => !arg.startsWith("-"));
const tenantIds = requested.length
  ? requested
  : fs
      .readdirSync(path.join(root, "tenants"), { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name);

const version = JSON.parse(
  fs.readFileSync(path.join(root, "src-tauri", "tauri.conf.json"), "utf8"),
).version;
const nsisDir = path.join(root, "src-tauri", "target", "release", "bundle", "nsis");
const outDir = path.join(root, "release-assets", `v${version}`);

for (const id of tenantIds) {
  console.log(`\n=== Building ${id} ${version} ===`);
  const result = spawnSync(
    process.execPath,
    [path.join(root, "scripts", "build-release.mjs"), `--tenant=${id}`],
    { cwd: root, stdio: "inherit", env: process.env },
  );
  if (result.status !== 0) {
    console.error(`\nBuild failed for tenant "${id}". Nothing was released.`);
    process.exit(result.status ?? 1);
  }
}

fs.rmSync(outDir, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });

for (const id of tenantIds) {
  const tenant = loadTenant(id);
  const setupName = `${tenant.productName}_${version}_x64-setup.exe`;
  // GitHub turns spaces in asset names into dots; latest.json already points at that name.
  fs.copyFileSync(path.join(nsisDir, setupName), path.join(outDir, setupName.replaceAll(" ", ".")));
}

// Apps older than the multi-tenant updater read `releases/latest/download/latest.json`,
// which is always RAGE's. Keep it in every release so they never get a 404.
const rageLatest = path.join(root, "updates", "rage", "latest.json");
if (tenantIds.includes("rage") && fs.existsSync(rageLatest)) {
  fs.copyFileSync(rageLatest, path.join(outDir, "latest.json"));
}

const tag = releaseTag(loadTenant(tenantIds[0]), version);
console.log(`\nDone. Upload everything in ${outDir} to GitHub release ${tag}:`);
for (const file of fs.readdirSync(outDir)) console.log(`  - ${file}`);
console.log(`Then commit updates/*/latest.json to main (after the release is published).`);
