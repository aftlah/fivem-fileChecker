import type { ScanRule } from "../types";

export const deepInspectRule: ScanRule = {
  id: "deep-inspect",
  name: "Deep Inspect",
  description:
    "Force-opens suspicious files (scripts, .asi, plugin DLLs, archives, cheat-like names) and searches their contents for cheat keywords. Files that cannot be opened are reported too.",
  relativePath: ".",
  type: "deep-scan",
  severity: "high",
};
