import { Trans, useLingui } from "@lingui/react/macro";
import { documentDir } from "@tauri-apps/api/path";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { CheckCircle, Circle, Loader2, XCircle } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { getRemoteWorkspaces, type RemoteWorkspaceInfo } from "@/commands/pairing";
import {
  createWorkspaceForSync,
  openWorkspaceWindow,
  triggerWorkspaceSync,
} from "@/commands/workspace";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useSyncStore } from "@/stores/syncStore";

// ── Types ──

type Phase = "loading" | "selecting" | "syncing" | "done";

type WorkspaceSyncStatus = "pending" | "syncing" | "done" | "error";

interface WorkspaceSyncItem {
  ws: RemoteWorkspaceInfo;
  status: WorkspaceSyncStatus;
  error?: string;
  localPath?: string;
}

interface WorkspaceSyncDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

// ── Helpers ──

const FALLBACK_BASE_PATH = "~/Documents/SwarmNote";

async function resolveDefaultBasePath(): Promise<string> {
  try {
    const docs = await documentDir();
    const sep = docs.endsWith("/") || docs.endsWith("\\") ? "" : "/";
    return `${docs}${sep}SwarmNote`;
  } catch {
    return FALLBACK_BASE_PATH;
  }
}

function groupByPeer(workspaces: RemoteWorkspaceInfo[]): Map<string, RemoteWorkspaceInfo[]> {
  const map = new Map<string, RemoteWorkspaceInfo[]>();
  for (const ws of workspaces) {
    const key = ws.peerId;
    if (!map.has(key)) map.set(key, []);
    map.get(key)?.push(ws);
  }
  return map;
}

// ── SyncProgressRow ──

function SyncProgressRow({
  item,
  peerId,
  onOpen,
}: {
  item: WorkspaceSyncItem;
  peerId: string;
  onOpen?: (path: string) => void;
}) {
  const activeSyncs = useSyncStore((s) => s.activeSyncs);
  const syncKey = `${item.ws.uuid}:${peerId}`;
  const activeSync = activeSyncs[syncKey];

  const hasProgress = item.status === "syncing" && activeSync && activeSync.total > 0;
  const percent = hasProgress ? Math.round((activeSync.completed / activeSync.total) * 100) : 0;
  const canOpen = item.status === "done" && !!item.localPath && !!onOpen;

  const handleOpen = canOpen
    ? () => {
        if (item.localPath) onOpen(item.localPath);
      }
    : undefined;

  const Container = canOpen ? "button" : "div";

  return (
    <Container
      type={canOpen ? "button" : undefined}
      className={`flex w-full flex-col gap-2 rounded-lg border p-3 text-left${canOpen ? " cursor-pointer transition-colors hover:bg-muted/50" : ""}`}
      onClick={handleOpen}
    >
      <div className="flex items-center gap-3">
        <div className="shrink-0">
          {item.status === "done" && <CheckCircle className="h-4 w-4 text-green-500" />}
          {item.status === "error" && <XCircle className="h-4 w-4 text-destructive" />}
          {item.status === "syncing" && <Loader2 className="h-4 w-4 animate-spin text-primary" />}
          {item.status === "pending" && <Circle className="h-4 w-4 text-muted-foreground" />}
        </div>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">{item.ws.name}</p>
          {item.status === "pending" && (
            <p className="text-xs text-muted-foreground">
              <Trans>{item.ws.docCount} 篇文档</Trans>
            </p>
          )}
          {item.status === "syncing" && !hasProgress && (
            <p className="text-xs text-muted-foreground">
              <Trans>准备同步 · {item.ws.docCount} 篇文档</Trans>
            </p>
          )}
          {item.status === "error" && item.error && (
            <p className="truncate text-xs text-destructive">{item.error}</p>
          )}
          {item.status === "done" && (
            <p className="text-xs text-muted-foreground">
              <Trans>同步完成 · {item.ws.docCount} 篇文档</Trans>
            </p>
          )}
        </div>
        {hasProgress && (
          <span className="shrink-0 text-xs tabular-nums text-muted-foreground">{percent}%</span>
        )}
        {canOpen && (
          <span className="shrink-0 text-xs text-primary">
            <Trans>打开</Trans>
          </span>
        )}
      </div>
      {hasProgress && (
        <div className="flex items-center gap-2">
          <div className="relative h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
            <div
              className="h-full rounded-full bg-primary transition-all duration-300 ease-out"
              style={{ width: `${percent}%` }}
            />
          </div>
          <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
            {activeSync.completed}/{activeSync.total}
          </span>
        </div>
      )}
    </Container>
  );
}

// ── Main Component ──

export function WorkspaceSyncDialog({ open, onOpenChange }: WorkspaceSyncDialogProps) {
  const { t } = useLingui();
  const [phase, setPhase] = useState<Phase>("loading");
  const [remoteWorkspaces, setRemoteWorkspaces] = useState<RemoteWorkspaceInfo[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [basePath, setBasePath] = useState(FALLBACK_BASE_PATH);
  const [syncItems, setSyncItems] = useState<WorkspaceSyncItem[]>([]);

  const loadWorkspaces = useCallback(async () => {
    setPhase("loading");
    setLoadError(null);
    try {
      const [data, defaultPath] = await Promise.all([
        getRemoteWorkspaces(),
        resolveDefaultBasePath(),
      ]);
      setBasePath(defaultPath);
      setRemoteWorkspaces(data);
      setPhase("selecting");
    } catch (e) {
      setLoadError(String(e));
      setPhase("selecting");
    }
  }, []);

  // Reset state when dialog opens
  useEffect(() => {
    if (open) {
      setRemoteWorkspaces([]);
      setSelectedIds(new Set());
      setBasePath(FALLBACK_BASE_PATH);
      setSyncItems([]);
      loadWorkspaces();
    }
  }, [open, loadWorkspaces]);

  const grouped = useMemo(() => groupByPeer(remoteWorkspaces), [remoteWorkspaces]);

  function toggleSelection(uuid: string) {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(uuid)) {
        next.delete(uuid);
      } else {
        next.add(uuid);
      }
      return next;
    });
  }

  async function handleChangeBasePath() {
    const selected = await openDialog({ directory: true, title: t`选择同步目标目录` });
    if (selected) setBasePath(selected);
  }

  async function handleStartSync() {
    const selected = remoteWorkspaces.filter((ws) => selectedIds.has(ws.uuid));
    if (selected.length === 0) return;

    const items: WorkspaceSyncItem[] = selected.map((ws) => ({
      ws,
      status: "pending",
    }));
    setSyncItems(items);
    setPhase("syncing");

    const updatedItems = [...items];

    for (let i = 0; i < updatedItems.length; i++) {
      updatedItems[i] = { ...updatedItems[i], status: "syncing" };
      setSyncItems([...updatedItems]);

      try {
        const localPath = await createWorkspaceForSync(
          updatedItems[i].ws.uuid,
          updatedItems[i].ws.name,
          basePath,
        );
        await triggerWorkspaceSync(updatedItems[i].ws.uuid, updatedItems[i].ws.peerId);
        updatedItems[i] = { ...updatedItems[i], status: "done", localPath };
      } catch (e) {
        updatedItems[i] = { ...updatedItems[i], status: "error", error: String(e) };
      }

      setSyncItems([...updatedItems]);
    }

    setPhase("done");
  }

  async function handleOpenSyncedWorkspace(path: string) {
    await openWorkspaceWindow(path);
    onOpenChange(false);
  }

  function handleBackground() {
    onOpenChange(false);
  }

  function preventDismiss(e: Event) {
    e.preventDefault();
  }

  // ── Dialog title by phase ──

  const title =
    phase === "syncing" ? (
      <Trans>正在同步...</Trans>
    ) : phase === "done" ? (
      <Trans>同步完成</Trans>
    ) : (
      <Trans>同步工作区</Trans>
    );

  // ── Content by phase ──

  const isSyncing = phase === "syncing";

  const selectedCount = selectedIds.size;

  const doneCount = syncItems.filter((i) => i.status === "done").length;
  const errorCount = syncItems.filter((i) => i.status === "error").length;

  return (
    <Dialog open={open} onOpenChange={isSyncing ? undefined : onOpenChange}>
      <DialogContent
        className="max-w-md"
        showCloseButton={!isSyncing}
        onInteractOutside={isSyncing ? preventDismiss : undefined}
        onEscapeKeyDown={isSyncing ? preventDismiss : undefined}
      >
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>

        {/* Loading phase */}
        {phase === "loading" && (
          <div className="flex flex-col items-center gap-3 py-8">
            <Loader2 className="h-8 w-8 animate-spin text-primary" />
            <p className="text-sm text-muted-foreground">
              <Trans>正在获取可同步的工作区...</Trans>
            </p>
          </div>
        )}

        {/* Selecting phase */}
        {phase === "selecting" && (
          <div className="flex flex-col gap-4">
            {loadError ? (
              <div className="flex flex-col items-center gap-3 py-6">
                <p className="text-sm text-destructive">{loadError}</p>
                <Button variant="outline" size="sm" onClick={loadWorkspaces}>
                  <Trans>重试</Trans>
                </Button>
              </div>
            ) : remoteWorkspaces.length === 0 ? (
              <div className="flex flex-col items-center gap-3 py-6">
                <p className="text-sm text-muted-foreground">
                  <Trans>未找到可同步的工作区</Trans>
                </p>
                <Button variant="outline" size="sm" onClick={loadWorkspaces}>
                  <Trans>重试</Trans>
                </Button>
              </div>
            ) : (
              <div className="flex max-h-64 flex-col gap-3 overflow-y-auto">
                {Array.from(grouped.entries()).map(([peerId, wsList]) => (
                  <div key={peerId} className="flex flex-col gap-1.5">
                    <div className="flex items-center gap-2">
                      <span className="text-xs font-semibold text-foreground">
                        {wsList[0].peerName}
                      </span>
                      <span className="rounded-full bg-green-100 px-1.5 py-0.5 text-xs text-green-700 dark:bg-green-900/30 dark:text-green-400">
                        <Trans>在线</Trans>
                      </span>
                    </div>
                    {wsList.map((ws) => (
                      <label
                        key={ws.uuid}
                        className="flex cursor-pointer items-center gap-3 rounded-lg border p-3 transition-colors hover:bg-muted/50"
                      >
                        <input
                          type="checkbox"
                          className="h-4 w-4 accent-primary"
                          checked={selectedIds.has(ws.uuid)}
                          onChange={() => toggleSelection(ws.uuid)}
                        />
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-sm font-medium">{ws.name}</p>
                          <p className="text-xs text-muted-foreground">
                            <Trans>{ws.docCount} 篇文档</Trans>
                          </p>
                        </div>
                        {ws.isLocal && (
                          <span className="shrink-0 rounded-full bg-green-100 px-1.5 py-0.5 text-xs text-green-700 dark:bg-green-900/30 dark:text-green-400">
                            <Trans>已同步</Trans>
                          </span>
                        )}
                      </label>
                    ))}
                  </div>
                ))}
              </div>
            )}

            {/* Base path selector */}
            {remoteWorkspaces.length > 0 && !loadError && (
              <div className="flex flex-col gap-1.5">
                <span className="text-xs font-medium text-muted-foreground">
                  <Trans>同步位置</Trans>
                </span>
                <div className="flex items-center gap-2">
                  <span className="min-w-0 flex-1 truncate rounded-md border bg-muted/30 px-3 py-1.5 text-xs text-foreground">
                    {basePath}
                  </span>
                  <Button variant="outline" size="sm" onClick={handleChangeBasePath}>
                    <Trans>更改</Trans>
                  </Button>
                </div>
              </div>
            )}

            <DialogFooter>
              <Button variant="outline" onClick={() => onOpenChange(false)}>
                <Trans>取消</Trans>
              </Button>
              <Button onClick={handleStartSync} disabled={selectedCount === 0}>
                <Trans>开始同步</Trans>
              </Button>
            </DialogFooter>
          </div>
        )}

        {/* Syncing phase */}
        {phase === "syncing" && (
          <div className="flex flex-col gap-4">
            <div className="flex max-h-64 flex-col gap-2 overflow-y-auto">
              {syncItems.map((item) => (
                <SyncProgressRow key={item.ws.uuid} item={item} peerId={item.ws.peerId} />
              ))}
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={handleBackground}>
                <Trans>后台运行</Trans>
              </Button>
            </DialogFooter>
          </div>
        )}

        {/* Done phase */}
        {phase === "done" && (
          <div className="flex flex-col gap-4">
            <div className="flex max-h-64 flex-col gap-2 overflow-y-auto">
              {syncItems.map((item) => (
                <SyncProgressRow
                  key={item.ws.uuid}
                  item={item}
                  peerId={item.ws.peerId}
                  onOpen={handleOpenSyncedWorkspace}
                />
              ))}
            </div>
            <p className="text-sm text-muted-foreground">
              <Trans>
                {doneCount} 个成功，{errorCount} 个失败
              </Trans>
            </p>
            <DialogFooter>
              <Button onClick={() => onOpenChange(false)}>
                <Trans>完成</Trans>
              </Button>
            </DialogFooter>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
