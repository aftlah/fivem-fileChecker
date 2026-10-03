import type { ScanRule } from "../types";

export const rpfArchivesRule: ScanRule = {
  id: "rpf-archives",
  name: "RPF Archives",
  description:
    "Opens .rpf archives in the FiveM folder and flags gameplay-data mods (recoil, ped accuracy, weapon/handling meta), disguised files, OpenIV packages, and encrypted or fake archives.",
  relativePath: ".",
  type: "rpf-archives",
  severity: "high",
};
