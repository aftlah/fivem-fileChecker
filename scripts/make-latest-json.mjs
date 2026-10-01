import fs from "node:fs";
import path from "node:path";
import { extractTenantArg, loadTenant, releaseAssetUrl, releaseTag, root } from "./tenant.mjs";

const [tenantId] = extractTenantArg(process.argv.slice(2));
const tenant = loadTenant(tenantId);
const confPath = path.join(root, "src-tauri", "tauri.conf.json");
const conf = JSON.parse(fs.readFileSync(confPath, "utf8"));

const version = conf.version;
const productName = tenant.productName;
const setupName = `${productName}_${version}_x64-setup.exe`;
const nsisDir = path.join(root, "src-tauri", "target", "release", "bundle", "nsis");
const setupPath = path.join(nsisDir, setupName);
const sigPath = `${setupPath}.sig`;

if (!fs.existsSync(setupPath)) {
  console.error(`Installer not found: ${setupPath}`);
  console.error("Run a signed release build first.");
  process.exit(1);
}

if (!fs.existsSync(sigPath)) {
  console.error(`Signature not found: ${sigPath}`);
  console.error("Build with TAURI_SIGNING_PRIVATE_KEY set so updater artifacts are created.");
  process.exit(1);
}

const signature = fs.readFileSync(sigPath, "utf8").trim();
const notesPath = path.join(root, "RELEASE_NOTES.txt");
const notes = fs.existsSync(notesPath)
  ? fs.readFileSync(notesPath, "utf8").trim()
  : `${productName} ${version}`;

const githubAssetName = setupName.replaceAll(" ", ".");

const latest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature,
      url: releaseAssetUrl(tenant, version, githubAssetName),
    },
  },
};

const outDir = path.join(root, "updates", tenant.id);
fs.mkdirSync(outDir, { recursive: true });
const outPath = path.join(outDir, "latest.json");
fs.writeFileSync(outPath, `${JSON.stringify(latest, null, 2)}\n`, "utf8");

console.log(`Wrote ${outPath}`);
console.log(`Upload these files to GitHub release ${releaseTag(tenant, version)}:`);
console.log(`  - ${setupPath}`);
console.log(`  - ${outPath}  `);
console.log(`  - commit updates/${tenant.id}/latest.json to main (the app reads it from there)`);
