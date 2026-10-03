import type { ScanRule } from "../types";

export const recoilContentRule: ScanRule = {
  id: "recoil-content",
  name: "No Recoil / Disguised Meta",
  description:
    "Reads the contents of loose .meta/.xml files and flags no-recoil values, game data disguised under another file name, and OpenIV packages.",
  relativePath: ".",
  type: "content-scan",
  severity: "high",
};
