import { extraScriptsRule } from "./scripts";
import { aiFolderRule } from "./aiFolder";
import { gameDataRules } from "./gameData";
import { deepInspectRule } from "./deepInspect";
import { recoilContentRule } from "./recoilContent";
import { rpfArchivesRule } from "./rpfArchives";
import { tenant } from "@/tenant";
import type { ScanRule } from "../types";

const allRules: ScanRule[] = [
  extraScriptsRule,
  aiFolderRule,
  ...gameDataRules,
  rpfArchivesRule,
  recoilContentRule,
  deepInspectRule,
];

export const scanRules: ScanRule[] = tenant.rules
  ? allRules.filter((rule) => tenant.rules?.includes(rule.id))
  : allRules;

export function getRuleById(id: string): ScanRule | undefined {
  return scanRules.find((rule) => rule.id === id);
}
