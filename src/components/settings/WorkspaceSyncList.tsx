import { Trans } from "@lingui/react/macro";
import { CheckCircle2, Circle, FolderSync, Loader2, WifiOff } from "lucide-react";
import { useEffect, useState } from "react";
import type { RecentWorkspace } from "@/commands/workspace";
import { getRecentWorkspaces } from "@/commands/workspace";
import { useSyncDisplayState } from "@/hooks/useSyncDisplayState";
import { cn } from "@/lib/utils";
import { useNetworkStore } from "@/stores/networkStore";

function WorkspaceSyncItem({ workspace }: { workspace: RecentWorkspace }) {
  const syncState = useSyncDisplayState(workspace.uuid);

  if (syncState.status === "syncing") {
    const completed = syncState.completed ?? 0;
    const pct = syncState.total ? Math.round((completed / syncState.total) * 100) : 0;
    return (
      <div className="flex items-center gap-3 py-2.5">
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-blue-500/10">
          <Loader2 className="h-4 w-4 animate-spin text-blue-500" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium">{workspace.name}</div>
          <div className="mt-0.5 flex items-center gap-1.5 text-xs text-muted-foreground">
            <span>
              <Trans>
                同步中 · {syncState.completed}/{syncState.total} 篇
              </Trans>
            </span>
          </div>
          <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-muted">
            <div
              className="h-full rounded-full bg-blue-500 transition-all"
              style={{ width: `${pct}%` }}
            />
          </div>
        </div>
      </div>
    );
  }

  if (syncState.status === "synced") {
    const timeStr = syncState.lastSyncedAt
      ? new Date(syncState.lastSyncedAt).toLocaleTimeString([], {
          hour: "2-digit",
          minute: "2-digit",
        })
      : "—";
    return (
      <div className="flex items-center gap-3 py-2.5">
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-green-500/10">
          <CheckCircle2 className="h-4 w-4 text-green-500" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium">{workspace.name}</div>
          <div className="mt-0.5 flex items-center gap-1.5 text-xs text-muted-foreground">
            <span>
              <Trans>已同步 · 最后同步 {timeStr}</Trans>
            </span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-3 py-2.5">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-muted">
        <FolderSync className="h-4 w-4 text-muted-foreground" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium">{workspace.name}</div>
        <div className="mt-0.5 flex items-center gap-1.5 text-xs text-muted-foreground">
          <Circle className="h-3 w-3 fill-muted-foreground/40 text-muted-foreground/40" />
          <span>
            <Trans>仅本地</Trans>
          </span>
        </div>
      </div>
    </div>
  );
}

export function WorkspaceSyncList() {
  const status = useNetworkStore((s) => s.status);
  const [recentWorkspaces, setRecentWorkspaces] = useState<RecentWorkspace[]>([]);

  useEffect(() => {
    getRecentWorkspaces()
      .then(setRecentWorkspaces)
      .catch(() => null);
  }, []);

  if (status !== "running") {
    return (
      <div className="flex flex-col items-center gap-2 rounded-lg border border-dashed py-6">
        <WifiOff className="h-5 w-5 text-muted-foreground/40" />
        <p className="text-xs font-medium text-muted-foreground">
          <Trans>网络未启动</Trans>
        </p>
        <p className="text-[11px] text-muted-foreground/60">
          <Trans>启动 P2P 网络后即可同步工作区</Trans>
        </p>
      </div>
    );
  }

  if (recentWorkspaces.length === 0) {
    return (
      <div className="flex flex-col items-center gap-1.5 rounded-lg border border-dashed py-6">
        <FolderSync className="h-5 w-5 text-muted-foreground/40" />
        <p className="text-xs font-medium text-muted-foreground">
          <Trans>暂无工作区</Trans>
        </p>
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded-lg border">
      <div
        className={cn("space-y-0 px-3.5", recentWorkspaces.length > 1 && "divide-y divide-border")}
      >
        {recentWorkspaces.map((ws) => (
          <WorkspaceSyncItem key={ws.path} workspace={ws} />
        ))}
      </div>
    </div>
  );
}
