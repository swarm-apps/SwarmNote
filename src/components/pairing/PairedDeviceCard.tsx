import { useLingui } from "@lingui/react/macro";
import { Unlink } from "lucide-react";
import { useState } from "react";
import type { Device } from "@/commands/pairing";
import { ConnectionBadge } from "@/components/pairing/ConnectionBadge";
import { formatRelativeTime } from "@/lib/dateUtils";
import { cn } from "@/lib/utils";
import { DeviceAvatar } from "./DeviceAvatar";
import { UnpairConfirmDialog } from "./UnpairConfirmDialog";

interface PairedDeviceCardProps {
  device: Device;
  onUnpaired?: () => void;
  isLast?: boolean;
}

export function PairedDeviceCard({ device, onUnpaired, isLast }: PairedDeviceCardProps) {
  const { t } = useLingui();
  const isOnline = device.status === "online";
  const [confirmOpen, setConfirmOpen] = useState(false);

  return (
    <>
      <div
        className={cn(
          "group flex items-center gap-2.5 px-3.5 py-2.5 transition-colors hover:bg-muted/50",
          !isLast && "border-b",
        )}
      >
        <DeviceAvatar os={device.os} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="text-[13px] font-medium">{device.name ?? device.hostname}</span>
            {isOnline && device.connection && (
              <ConnectionBadge type={device.connection} latency={device.latency} />
            )}
          </div>
          <div className="text-[11px] text-muted-foreground">
            {isOnline
              ? `${device.name ? `${device.hostname} · ` : ""}${device.os} · ${device.platform}`
              : `${device.name ? `${device.hostname} · ` : ""}${device.os} · ${device.platform} · ${t`最后在线 ${formatRelativeTime(device.lastSeen ?? null)}`}`}
          </div>
        </div>
        <button
          type="button"
          onClick={() => setConfirmOpen(true)}
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-muted-foreground/60 opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive group-hover:opacity-100"
          title={t`取消配对`}
        >
          <Unlink className="h-3.5 w-3.5" />
        </button>
      </div>

      <UnpairConfirmDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        deviceName={device.name ?? device.hostname}
        peerId={device.peerId}
        onConfirm={() => {
          setConfirmOpen(false);
          onUnpaired?.();
        }}
      />
    </>
  );
}
