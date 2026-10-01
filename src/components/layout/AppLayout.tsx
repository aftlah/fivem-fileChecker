import { TenantLogo } from "@/components/TenantLogo";
import { tenant } from "@/tenant";
import type { ReactElement, ReactNode } from "react";
import {
  Clock3,
  LayoutDashboard,
  Settings,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { AppView } from "@/scanner/types";

interface AppLayoutProps {
  view: AppView;
  onNavigate: (view: AppView) => void;
  children: ReactNode;
}

const navItems: Array<{ id: AppView; label: string; icon: typeof LayoutDashboard }> = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "history", label: "Scan History", icon: Clock3 },
  { id: "settings", label: "Settings", icon: Settings },
];

export function AppLayout({
  view,
  onNavigate,
  children,
}: AppLayoutProps): ReactElement {
  return (
    <div className="flex h-screen overflow-hidden bg-background text-foreground">
      <aside className="flex w-64 shrink-0 flex-col border-r border-border bg-sidebar">
        <div className="flex items-center gap-3 px-5 py-6">
          <TenantLogo />
          <div>
            <p className="text-sm font-semibold">{tenant.productName}</p>
            <p className="text-xs text-muted-foreground">{tenant.tagline}</p>
          </div>
        </div>

        <nav className="flex flex-1 flex-col gap-1 px-3">
          {navItems.map((item) => {
            const Icon = item.icon;
            const active = view === item.id;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => onNavigate(item.id)}
                className={cn(
                  "flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors",
                  active
                    ? "bg-accent text-foreground"
                    : "text-muted-foreground hover:bg-accent hover:text-foreground",
                )}
              >
                <Icon className="h-4 w-4" />
                {item.label}
              </button>
            );
          })}
        </nav>

        <div className="px-5 py-4 text-xs text-muted-foreground">Version {tenant.version}</div>
      </aside>

      <main className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-5xl px-8 py-8">{children}</div>
      </main>
    </div>
  );
}
