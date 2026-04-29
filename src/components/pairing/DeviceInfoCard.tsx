import { Monitor } from "lucide-react";
import type * as React from "react";
import { cn } from "@/lib/utils";

interface DeviceInfoCardProps {
  name?: string;
  hostname: string;
  os: string;
  platform?: string;
  children?: React.ReactNode;
  className?: string;
}

export function DeviceInfoCard({
  name,
  hostname,
  os,
  platform,
  children,
  className,
}: DeviceInfoCardProps) {
  const displayName = name ?? hostname;
  const osText = platform ? `${os} · ${platform}` : os;

  return (
    <div className={cn("flex items-center gap-3 rounded-lg border p-3", className)}>
      <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-muted">
        <Monitor className="h-5 w-5 text-muted-foreground" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium">{displayName}</div>
        <div className="text-xs text-muted-foreground">
          {name ? `${hostname} · ${osText}` : osText}
        </div>
        {children}
      </div>
    </div>
  );
}
