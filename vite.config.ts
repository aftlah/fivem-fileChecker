import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";
import fs from "node:fs";
import { fileURLToPath } from "node:url";

const host = process.env.TAURI_DEV_HOST;
const projectRoot = path.dirname(fileURLToPath(import.meta.url));

const tenantId = process.env.TENANT ?? "rage";
const tenantPath = path.join(projectRoot, "tenants", tenantId, "tenant.json");
if (!fs.existsSync(tenantPath)) {
  throw new Error(`Unknown tenant "${tenantId}" (missing ${tenantPath})`);
}
const tenant = JSON.parse(fs.readFileSync(tenantPath, "utf8"));
const appVersion: string = JSON.parse(
  fs.readFileSync(path.join(projectRoot, "package.json"), "utf8"),
).version;

// Optional in-app logo: path (relative to tenants/<id>/) from tenant.json "logo", default logo.png.
const logoPath = path.join(projectRoot, "tenants", tenantId, tenant.logo ?? "logo.png");
const hasLogo = fs.existsSync(logoPath);

function themeCss(): string {
  const block = (selector: string, vars: Record<string, string> = {}) => {
    const primary = vars.primary;
    if (!primary) return "";
    const foreground = vars.primaryForeground ? `--primary-foreground:${vars.primaryForeground};` : "";
    return `${selector}{--primary:${primary};--ring:${primary};--info:${primary};${foreground}}`;
  };
  return block("html:root", tenant.theme?.light) + block("html.dark", tenant.theme?.dark);
}

export default defineConfig({
  define: {
    __TENANT__: JSON.stringify({
      id: tenant.id,
      productName: tenant.productName,
      version: appVersion,
      tagline: tenant.tagline,
      rules: tenant.rules ?? null,
    }),
  },
  plugins: [
    react(),
    tailwindcss(),
    {
      name: "tenant-logo",
      resolveId: (id) => (id === "virtual:tenant-logo" ? "\0virtual:tenant-logo" : null),
      load: (id) =>
        id === "\0virtual:tenant-logo"
          ? hasLogo
            ? `import url from ${JSON.stringify(logoPath.replaceAll("\\", "/"))}; export default url;`
            : "export default null;"
          : null,
    },
    {
      name: "tenant-html",
      transformIndexHtml: (html) =>
        html
          .replace(/<title>.*?<\/title>/, `<title>${tenant.productName}</title>`)
          .replace("</head>", `<style>${themeCss()}</style></head>`),
    },
  ],
  resolve: {
    alias: {
      "@": path.resolve(projectRoot, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: tenant.devPort ?? 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
