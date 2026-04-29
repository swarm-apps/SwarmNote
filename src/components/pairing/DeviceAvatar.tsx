import { Laptop, Monitor, Smartphone, TabletSmartphone, Terminal } from "lucide-react";
import { cn } from "@/lib/utils";

function getDeviceIcon(os: string) {
  const lower = os.toLowerCase();
  if (lower.includes("macos") || lower.includes("mac os")) return Laptop;
  if (lower.includes("ipados")) return TabletSmartphone;
  if (lower.includes("ios") || lower.includes("android")) return Smartphone;
  if (lower.includes("linux")) return Terminal;
  return Monitor;
}

interface DeviceAvatarProps {
  os: string;
  isCurrent?: boolean;
  className?: string;
}

export function DeviceAvatar({ os, isCurrent, className }: DeviceAvatarProps) {
  const Icon = getDeviceIcon(os);
  return (
    <div
      className={cn(
        "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg",
        isCurrent ? "bg-primary/15" : "bg-muted",
        className,
      )}
    >
      <Icon className={cn("h-4 w-4", isCurrent ? "text-primary" : "text-muted-foreground")} />
    </div>
  );
}
