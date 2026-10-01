import type { ReactElement } from "react";
import { ScanSearch } from "lucide-react";
import { tenant } from "@/tenant";
import { cn } from "@/lib/utils";

interface TenantLogoProps {
  /** Tailwind size classes for the logo box, e.g. "h-10 w-10". */
  className?: string;
}

/** The tenant's logo, or a generic scan icon when the tenant has none. */
export function TenantLogo({ className = "h-10 w-10" }: TenantLogoProps): ReactElement {
  return (
    <div
      className={cn(
        "flex shrink-0 items-center justify-center rounded-lg border border-border bg-card p-1",
        className,
      )}
    >
      {tenant.logo ? (
        <img src={tenant.logo} alt="" className="h-full w-full object-contain" />
      ) : (
        <ScanSearch className="h-1/2 w-1/2 text-info" />
      )}
    </div>
  );
}
