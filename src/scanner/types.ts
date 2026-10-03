export type ScanItemType = "directory" | "file" | "scripts" | "file-search" | "rpf-archives" | "content-scan" | "deep-scan";
export type ScanSeverity = "high" | "medium" | "low";
export type ScanStatus = "NOT_FOUND" | "DETECTED" | "ERROR";
export type OverallStatus = "CLEAN" | "DETECTED" | "WARNING";
export type AppView = "dashboard" | "history" | "settings";
export type ThemeMode = "dark" | "light";

export interface ScanRule {
  id: string;
  name: string;
  description: string;
  relativePath: string;
  type: ScanItemType;
  severity: ScanSeverity;
}

export interface ScanResult {
  ruleId: string;
  name: string;
  status: ScanStatus;
  path: string;
  relativePath: string;
  exists: boolean;
  type: ScanItemType;
  severity: ScanSeverity;
  modifiedAt: number | null;
  error: string | null;
  foundFiles?: string[];
}

export interface ScanProgressEvent {
  current: number;
  total: number;
  ruleId: string;
  ruleName: string;
}

export interface ValidationResult {
  isValid: boolean;
  path: string;
  missing: string[];
  message: string;
  suggestedPath: string | null;
}

export interface ScanSummary {
  totalChecks: number;
  detected: number;
  notDetected: number;
  errors: number;
  overallStatus: OverallStatus;
}

export interface ScanHistoryEntry {
  id: string;
  scannedAt: string;
  fiveMPath: string;
  totalChecks: number;
  detected: number;
  notDetected: number;
  errors: number;
  overallStatus: OverallStatus;
}

export type DiscordSendStatus = "idle" | "sending" | "sent" | "error";

export interface AppSettings {
  enabledRules: Record<string, boolean>;
  theme: ThemeMode;
  autoScan: boolean;
  scanWhenFiveMStarts: boolean;
  operatorName: string;
}
