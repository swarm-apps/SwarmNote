import { useLingui } from "@lingui/react/macro";
import { RadioTower, Wifi, Zap } from "lucide-react";
import type { ConnectionType } from "@/lib/bindings";
import { cn } from "@/lib/utils";

const connectionConfig: Record<
  ConnectionType,
  {
    icon: React.ComponentType<{ className?: string }>;
    className: string;
  }
> = {
  lan: {
    icon: Wifi,
    className: "bg-green-500/15 text-green-700 dark:text-green-400",
  },
  dcutr: {
    icon: Zap,
    className: "bg-blue-500/15 text-blue-700 dark:text-blue-400",
  },
  relay: {
    icon: RadioTower,
    className: "bg-orange-500/15 text-orange-700 dark:text-orange-400",
  },
};

interface ConnectionBadgeProps {
  type: ConnectionType;
  latency?: number | null;
  className?: string;
}

export function ConnectionBadge({ type, latency, className }: ConnectionBadgeProps) {
  const { t } = useLingui();
  const config = connectionConfig[type];
  const Icon = config.icon;
  const typeLabels: Record<ConnectionType, string> = {
    lan: t`局域网`,
    dcutr: t`打洞`,
    relay: t`中继`,
  };
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium",
        config.className,
        className,
      )}
    >
      <Icon className="h-3 w-3" />
      {typeLabels[type]}
      {latency != null && <span className="opacity-70">{latency}ms</span>}
    </span>
  );
}
