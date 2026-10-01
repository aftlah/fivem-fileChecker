import { tenant } from "@/tenant";
import { useState, type ReactElement } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { scanRules } from "@/scanner/rules";
import { useSettings } from "@/hooks/useSettings";
import { checkAndInstallUpdate } from "@/lib/updater";

export function SettingsPage(): ReactElement {
  const { settings, setTheme, setOperatorName } = useSettings();
  const [updateMessage, setUpdateMessage] = useState<string | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);

  async function handleCheckUpdate(): Promise<void> {
    setCheckingUpdate(true);
    setUpdateMessage("Checking for updates...");
    const result = await checkAndInstallUpdate();
    setCheckingUpdate(false);

    if (result.status === "up-to-date") {
      setUpdateMessage("Already on the latest version.");
      return;
    }
    if (result.status === "updated") {
      setUpdateMessage(`Updated to ${result.version}. Restarting...`);
      return;
    }
    if (result.status === "skipped") {
      setUpdateMessage(result.reason);
      return;
    }
    setUpdateMessage(result.message);
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Only your character name and dark mode can be changed. All scan and automatic scan
          options are locked.
        </p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Player & Discord</CardTitle>
          <CardDescription>
            Every scan is sent to Discord with this name. The webhook is already configured.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <label className="block cursor-text space-y-2">
            <span className="text-sm font-medium">name in character</span>
            <Input
              value={settings.operatorName}
              onChange={(event) => setOperatorName(event.target.value)}
              placeholder="name in character"
              maxLength={40}
            />
          </label>
          <label className="block space-y-2">
            <span className="text-sm font-medium">Discord report</span>
            <Input value="Configured at build time (locked)" readOnly disabled />
            <span className="text-xs text-muted-foreground">
              The Discord destination is set when the app is built and is not stored in the public
              repository.
            </span>
          </label>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Scan Configuration</CardTitle>
          <CardDescription>
            These checks always run and cannot be turned off.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {scanRules.map((rule) => (
            <div
              key={rule.id}
              className="rounded-lg border border-border px-4 py-3"
            >
              <p className="text-sm font-medium">{rule.name}</p>
              <p className="mt-1 text-sm text-muted-foreground">{rule.description}</p>
              <p className="mt-1 text-xs text-muted-foreground">
                {rule.type === "file-search"
                  ? `Entire FiveM folder — search for ${rule.relativePath}`
                  : rule.relativePath === "."
                    ? "Entire FiveM folder"
                    : rule.relativePath}
              </p>
            </div>
          ))}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Automatic Scan</CardTitle>
          <CardDescription>
            Automatic scan behavior is always enabled and runs silently in the background.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium">Scan automatically</p>
            <p className="text-sm text-muted-foreground">
              Run a scan when the app opens and after a folder is selected.
            </p>
          </div>
          <LockedSwitch checked aria-label="Scan automatically locked on" />
        </CardContent>
        <CardContent className="flex items-center justify-between border-t border-border">
          <div>
            <p className="text-sm font-medium">Scan when FiveM starts</p>
            <p className="text-sm text-muted-foreground">
              When FiveM launches, the folder path switches to that FiveM installation and a scan
              runs automatically.
            </p>
          </div>
          <LockedSwitch checked aria-label="Scan when FiveM starts locked on" />
        </CardContent>
        <CardContent className="flex items-center justify-between border-t border-border">
          <div>
            <p className="text-sm font-medium">Open when FiveM starts</p>
            <p className="text-sm text-muted-foreground">
              Starts this app in watch mode with Windows. When FiveM opens, a scan runs, results
              are sent to Discord, then the app closes until FiveM is opened again.
            </p>
          </div>
          <LockedSwitch checked aria-label="Open when FiveM starts locked on" />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Appearance</CardTitle>
          <CardDescription>Dark mode is the default for this utility.</CardDescription>
        </CardHeader>
        <CardContent className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium">Dark mode</p>
            <p className="text-sm text-muted-foreground">Use a dark interface.</p>
          </div>
          <Switch
            checked={settings.theme === "dark"}
            onCheckedChange={(checked) => setTheme(checked ? "dark" : "light")}
            aria-label="Toggle dark mode"
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Updates</CardTitle>
          <CardDescription>
            The app checks GitHub Releases on startup and can install a signed update automatically.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-sm font-medium">Current version</p>
              <p className="text-sm text-muted-foreground">1.0.5</p>
            </div>
            <Button
              type="button"
              variant="outline"
              disabled={checkingUpdate}
              onClick={() => void handleCheckUpdate()}
            >
              {checkingUpdate ? "Checking..." : "Check for updates"}
            </Button>
          </div>
          {updateMessage ? (
            <p className="text-sm text-muted-foreground">{updateMessage}</p>
          ) : null}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>About</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2 text-sm">
          <Info label="Application" value={tenant.productName} />
          <Info label="Version" value="1.0.5" />
          <Info label="Developer" value="Aftlah" />
          <Info label="Mode" value="Read-only filesystem scanner" />
        </CardContent>
      </Card>
    </div>
  );
}

function LockedSwitch({
  checked,
  "aria-label": ariaLabel,
}: {
  checked: boolean;
  "aria-label": string;
}): ReactElement {
  return (
    <Switch
      checked={checked}
      disabled
      aria-label={ariaLabel}
      className="opacity-70"
    />
  );
}

function Info({ label, value }: { label: string; value: string }): ReactElement {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="text-muted-foreground">{label}</span>
      <span>{value}</span>
    </div>
  );
}
