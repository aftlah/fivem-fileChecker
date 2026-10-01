import { useState, type FormEvent, type ReactElement } from "react";
import { TenantLogo } from "@/components/TenantLogo";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useSettings } from "@/hooks/useSettings";
import { markSetupComplete } from "@/lib/storage";
import { tenant } from "@/tenant";

export function NameGate(): ReactElement {
  const { setOperatorName } = useSettings();
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);

  function handleSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    const trimmed = name.trim();
    if (trimmed.length < 2) {
      setError("Enter a name in character with at least 2 characters.");
      return;
    }
    setOperatorName(trimmed);
    markSetupComplete();
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-background px-6 py-10 text-foreground">
      <Card className="w-full max-w-md">
        <CardHeader>
          {tenant.logo ? (
            <img
              src={tenant.logo}
              alt={tenant.productName}
              className="mx-auto mb-2 h-44 w-auto max-w-full object-contain"
            />
          ) : (
            <TenantLogo className="mx-auto mb-2 h-32 w-32" />
          )}
          <CardTitle>name in character</CardTitle>
          <CardDescription>
            This character name is attached to every scan result sent to Discord. After setup, the
            app runs in the background and scans when FiveM opens.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form className="cursor-text space-y-4" onSubmit={handleSubmit}>
            <Input
              autoFocus
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="name in character"
              maxLength={40}
            />
            {error ? <p className="text-sm text-destructive">{error}</p> : null}
            <Button type="submit" className="w-full">
              Continue
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
