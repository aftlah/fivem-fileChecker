import { TenantLogo } from "@/components/TenantLogo";
import { tenant } from "@/tenant";
import type { ReactElement } from "react";
import { CheckCircle2, Loader2, Play } from "lucide-react";
import { PathSelector } from "@/components/dashboard/PathSelector";
import { ResultCard } from "@/components/dashboard/ResultCard";
import { ScanProgressPanel } from "@/components/dashboard/ScanProgressPanel";
import { ScanSummaryPanel } from "@/components/dashboard/ScanSummaryPanel";
import { ValidationWarning } from "@/components/dashboard/ValidationWarning";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { useScanContext } from "@/hooks/useScanContext";
import { useSettings } from "@/hooks/useSettings";
import { formatCooldownRemaining } from "@/lib/scanCooldown";
import type { ScanResult } from "@/scanner/types";

export function DashboardPage(): ReactElement {
  const scan = useScanContext();
  const { settings } = useSettings();
  const onCooldown = scan.cooldownRemainingMs > 0;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <TenantLogo className="h-14 w-14" />
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">{tenant.productName}</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Scanning as {settings.operatorName}. The app runs in the background and sends results to
            Discord automatically when FiveM opens.
          </p>
        </div>
      </div>

      <PathSelector
        selectedPath={scan.selectedPath}
        isBrowsing={scan.isBrowsing}
        isDetecting={scan.isDetecting}
        pathSource={scan.pathSource}
        disabled={scan.isScanning || scan.isDetecting}
        onBrowse={() => void scan.browse()}
      />

      <div className="rounded-xl border border-border bg-card px-4 py-3">
        <div className="flex items-center justify-between gap-3">
          <p className="text-sm text-muted-foreground">
            {scan.fiveMRunning
              ? "FiveM is running. The path was updated to this FiveM installation."
              : "Watching for FiveM. Opening FiveM will set the path automatically."}
          </p>
          <Badge variant={scan.fiveMRunning ? "info" : "outline"}>
            {scan.fiveMRunning ? "FiveM running" : "Waiting for FiveM"}
          </Badge>
        </div>
      </div>

      {scan.validation && !scan.validation.isValid ? (
        <ValidationWarning
          validation={scan.validation}
          onChooseAnother={() => void scan.browse()}
          onUseSuggested={(path) => void scan.applyPath(path, "auto")}
        />
      ) : null}

      {scan.error ? (
        <div className="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
          {scan.error}
        </div>
      ) : null}

      {scan.discordStatus === "sending" ? (
        <div className="rounded-xl border border-info/30 bg-info/10 px-4 py-3 text-sm text-info">
          Sending scan result to Discord...
        </div>
      ) : null}

      {scan.discordStatus === "sent" ? (
        <div className="rounded-xl border border-success/30 bg-success/10 px-4 py-3 text-sm text-success">
          Scan result sent to Discord.
        </div>
      ) : null}

      {scan.discordStatus === "error" && scan.discordError ? (
        <div className="rounded-xl border border-warning/30 bg-warning/10 px-4 py-3 text-sm text-warning">
          Scan finished, but Discord could not be notified: {scan.discordError}
        </div>
      ) : null}

      <div className="flex justify-center">
        <Button
          type="button"
          size="lg"
          onClick={() => void scan.startScan()}
          disabled={
            scan.isScanning || scan.isDetecting || !scan.selectedPath || onCooldown
          }
        >
          {scan.isScanning ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Play className="h-4 w-4" />
          )}
          {scan.isScanning
            ? "Scanning..."
            : onCooldown
              ? `Tunggu ${formatCooldownRemaining(scan.cooldownRemainingMs)}`
              : scan.hasScanned
                ? "Scan Again"
                : "Start Scan"}
        </Button>
      </div>

      {scan.isScanning && scan.progress ? (
        <ScanProgressPanel progress={scan.progress} />
      ) : null}

      <Separator />

      <section className="space-y-4">
        <div>
          <h2 className="text-lg font-semibold">Scan Result</h2>
          <p className="text-sm text-muted-foreground">
            {scan.hasScanned && scan.summary
              ? `Scan completed. ${scan.summary.detected} issue${scan.summary.detected === 1 ? "" : "s"} detected.`
              : scan.isDetecting
                ? "Looking for a FiveM installation..."
                : "Waiting for an automatic scan, or press Start Scan."}
          </p>
        </div>

        {scan.hasScanned && scan.summary ? <ScanSummaryPanel summary={scan.summary} /> : null}

        {scan.hasScanned && scan.results.length > 0 ? (
          <ScanResultList results={scan.results} />
        ) : null}

        {!scan.hasScanned && !scan.isScanning && !scan.isDetecting ? (
          <div className="rounded-xl border border-dashed border-border px-6 py-12 text-center">
            <p className="text-sm text-muted-foreground">
              {scan.selectedPath
                ? "Press Start Scan if automatic scanning is turned off."
                : "FiveM was not found automatically. Use Browse to select FiveM Application Data or FiveM.app."}
            </p>
          </div>
        ) : null}
      </section>
    </div>
  );
}

function ScanResultList({ results }: { results: ScanResult[] }): ReactElement {
  const flagged = results.filter((result) => result.status !== "NOT_FOUND");
  const clean = results.filter((result) => result.status === "NOT_FOUND");

  return (
    <div className="space-y-3">
      {flagged.length > 0 ? (
        <div className="grid gap-3">
          {flagged.map((result) => (
            <ResultCard key={result.ruleId} result={result} />
          ))}
        </div>
      ) : null}

      {clean.length > 0 ? (
        <div className="rounded-xl border border-dashed border-border bg-card/60 px-4 py-3">
          <div className="flex items-start gap-2">
            <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-success" />
            <div>
              <p className="text-sm font-medium">
                {clean.length} check{clean.length === 1 ? "" : "s"} not detected
              </p>
              <p className="mt-1 text-xs text-muted-foreground">
                {clean.map((result) => result.name).join(" · ")}
              </p>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
