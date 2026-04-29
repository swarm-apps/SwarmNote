import { useLingui } from "@lingui/react/macro";
import { Settings } from "lucide-react";

import type { Device } from "@/commands/pairing";
import { openSettingsWindow } from "@/commands/workspace";
import { Badge } from "@/components/ui/badge";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useSyncDisplayState } from "@/hooks/useSyncDisplayState";
import { computeSyncDisplay, type SyncDisplayLabel, syncDotClass } from "@/lib/syncDisplay";
import { useNetworkStore } from "@/stores/networkStore";

interface SyncStatusBarProps {
  workspaceUuid: string | undefined;
}

/**
 * Turn a structured `SyncDisplayLabel` into a localized human-readable string.
 * Keeping the translation here (rather than in `computeSyncDisplay`) preserves
 * the purity of the state machine while still participating in Lingui i18n.
 */
function useSyncLabelText(label: SyncDisplayLabel): string {
  const { t } = useLingui();
  switch (label.kind) {
    case "connecting":
      return t`连接中...`;
    case "error":
      return t`连接失败`;
    case "stopped":
      return t`网络未启动`;
    case "waiting-for-peers":
      return t`等待设备连接`;
    case "syncing": {
      const { peerCount, completed, total } = label;
      return t`${peerCount} 台 · 同步中 ${completed}/${total}`;
    }
    case "synced": {
      const { peerCount } = label;
      return t`${peerCount} 台设备在线 · 已同步`;
    }
    case "online": {
      const { peerCount } = label;
      return t`${peerCount} 台设备在线`;
    }
  }
}

function connectionLabel(connection: Device["connection"]): string {
  switch (connection) {
    case "lan":
      return "LAN";
    case "dcutr":
      return "DCUtR";
    case "relay":
      return "Relay";
    default:
      return "";
  }
}

function PeerListContent({ onlineDevices }: { onlineDevices: Device[] }) {
  const { t } = useLingui();

  return (
    <>
      <div className="px-1 text-xs font-medium text-muted-foreground">
        {t`已连接设备 (${onlineDevices.length})`}
      </div>

      {onlineDevices.length === 0 ? (
        <div className="px-1 py-3 text-center">
          <p className="text-sm text-muted-foreground">{t`暂无已连接设备`}</p>
          <p className="mt-1 text-xs text-muted-foreground/70">
            {t`确保其他设备已配对并在同一网络中`}
          </p>
        </div>
      ) : (
        <div className="flex flex-col gap-1">
          {onlineDevices.map((device) => (
            <div
              key={device.peerId}
              className="flex items-center justify-between rounded-md px-1 py-1.5"
            >
              <span className="truncate text-sm">{device.name || device.hostname}</span>
              <span className="flex shrink-0 items-center gap-1.5 ml-2">
                {device.connection && (
                  <Badge variant="secondary" className="text-[10px] h-4 px-1.5">
                    {connectionLabel(device.connection)}
                  </Badge>
                )}
                {device.latency != null && (
                  <span className="text-[10px] text-muted-foreground">{device.latency}ms</span>
                )}
              </span>
            </div>
          ))}
        </div>
      )}

      <div className="border-t border-border pt-1">
        <button
          type="button"
          className="flex w-full items-center gap-1.5 rounded-sm px-1 py-1 text-xs text-muted-foreground hover:bg-accent"
          onClick={() => openSettingsWindow("network")}
        >
          <Settings className="h-3 w-3" />
          {t`网络设置`}
        </button>
      </div>
    </>
  );
}

export function SyncStatusBar({ workspaceUuid }: SyncStatusBarProps) {
  const nodeStatus = useNetworkStore((s) => s.status);
  const nodeLoading = useNetworkStore((s) => s.loading);
  const devices = useNetworkStore((s) => s.devices);
  const onlineDevices = devices.filter((d) => d.status === "online");
  const connectedPeerCount = onlineDevices.length;
  const syncState = useSyncDisplayState(workspaceUuid);

  const { dot, label } = computeSyncDisplay({
    nodeLoading,
    nodeStatus,
    connectedPeerCount,
    syncState,
  });

  const labelText = useSyncLabelText(label);

  return (
    <div className="border-t border-sidebar-border px-2 py-1.5">
      <Popover>
        <PopoverTrigger asChild>
          <button
            type="button"
            className="flex w-full items-center gap-1.5 rounded-sm px-1 py-1 text-left hover:bg-sidebar-accent"
          >
            <span className={`inline-block h-2 w-2 shrink-0 rounded-full ${syncDotClass(dot)}`} />
            <span className="truncate text-xs text-muted-foreground">{labelText}</span>
          </button>
        </PopoverTrigger>
        <PopoverContent side="top" align="start" className="w-64">
          <PeerListContent onlineDevices={onlineDevices} />
        </PopoverContent>
      </Popover>
    </div>
  );
}
