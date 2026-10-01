# rage-file-checker

Read-only Windows desktop scanner for FiveM installations. The first release checks whether `citizen/common/data/ai` exists. The app never deletes, modifies, replaces, or executes FiveM files.

## Requirements

- Windows 10/11
- [Node.js](https://nodejs.org/) 20+
- [Rust](https://www.rust-lang.org/tools/install) (stable). If `cargo` is not found, close the terminal and open a new one after installing Rust.
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the Windows 10/11 SDK
- WebView2 (included with current Windows 10/11)

## Install

```bash
npm install
```

## Run in development

```bash
npm run tauri dev
```

## Build a Windows executable

```bash
npm run tauri build
```

After a successful build:

- Portable app: `src-tauri/target/release/rage-file-checker.exe`
- Installer: `src-tauri/target/release/bundle/nsis/`

## Adding a scan rule

Create a rule in `src/scanner/rules/` and export it from `src/scanner/rules/index.ts`. The scanner engine and native filesystem checks pick it up automatically.

Example:

```ts
{
  id: "gameconfig",
  name: "GameConfig",
  relativePath: "citizen/common/data/gameconfig.xml",
  type: "file",
  severity: "medium",
  description: "Checks whether gameconfig.xml exists."
}
```

## Multiple companies (tenants)

One codebase builds a separate app per company. Each tenant lives in `tenants/<id>/tenant.json` (name, bundle identifier, icons, theme colors, optional `rules` list of rule ids).

```bash
npm run tauri dev -- --tenant=bbc      # dev build for BBC (default tenant: rage)
npm run release -- --tenant=bbc        # signed installer + updates/bbc/latest.json
```

Per tenant you need:

- `.discord.<id>.env` with `DISCORD_WEBHOOK_URL` (see `.discord.env.example`)
- GitHub release tagged `<id>-v<version>` holding the installer; commit `updates/<id>/latest.json` to `main` (the updater reads it from there)

Add a company by copying `tenants/bbc/` to `tenants/<id>/` and editing it. Different identifiers keep installs and local data (history, name) separate.
