import { Trans, useLingui } from "@lingui/react/macro";
import { createFileRoute } from "@tanstack/react-router";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  Copy,
  EllipsisVertical,
  ExternalLink,
  FolderPlus,
  Minus,
  PenLine,
  Trash2,
  X,
} from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";

import {
  getRecentWorkspaces,
  openWorkspaceWindow,
  type RecentWorkspace,
  removeRecentWorkspace,
} from "@/commands/workspace";
import { Button } from "@/components/ui/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { WorkspaceSyncDialog } from "@/components/workspace/WorkspaceSyncDialog";
import { isMac } from "@/lib/utils";
import { useNetworkStore } from "@/stores/networkStore";

// ── Shared sub-components ──

/** 工作区项的菜单内容（DropdownMenu 和 ContextMenu 共用） */
function WorkspaceMenuItems({
  path,
  onRemove,
  Item,
  Separator,
}: {
  path: string;
  onRemove: () => void;
  Item: typeof DropdownMenuItem | typeof ContextMenuItem;
  Separator: typeof DropdownMenuSeparator | typeof ContextMenuSeparator;
}) {
  return (
    <>
      <Item onClick={() => openPath(path)}>
        <ExternalLink className="mr-2 h-4 w-4" />
        <Trans>在文件管理器中打开</Trans>
      </Item>
      <Item onClick={() => navigator.clipboard.writeText(path)}>
        <Copy className="mr-2 h-4 w-4" />
        <Trans>复制路径</Trans>
      </Item>
      <Separator />
      <Item onClick={onRemove} className="text-destructive focus:text-destructive">
        <Trash2 className="mr-2 h-4 w-4" />
        <Trans>从列表移除</Trans>
      </Item>
    </>
  );
}

/** 右侧操作卡片中的一行 */
function ActionRow({
  title,
  description,
  primary,
  children,
}: {
  title: ReactNode;
  description: ReactNode;
  primary?: boolean;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between px-5 py-4">
      <div className="min-w-0 flex-1">
        <div className={`text-sm font-semibold ${primary ? "text-primary" : "text-foreground"}`}>
          {title}
        </div>
        <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
      </div>
      <div className="ml-4 shrink-0">{children}</div>
    </div>
  );
}

// ── Main page ──

function WorkspaceManagerPage() {
  const { t } = useLingui();
  const appWindow = getCurrentWindow();
  const [recents, setRecents] = useState<RecentWorkspace[]>([]);
  const [syncDialogOpen, setSyncDialogOpen] = useState(false);
  const [appVersion, setAppVersion] = useState("");

  const devices = useNetworkStore((s) => s.devices);
  const onlineDevices = devices.filter((d) => d.status === "online" && d.isPaired);
  const hasOnlineDevices = onlineDevices.length > 0;

  useEffect(() => {
    getRecentWorkspaces().then(setRecents);
    getVersion().then(setAppVersion);
  }, []);

  async function handleOpen(path: string) {
    await openWorkspaceWindow(path, { closeWindow: "main" });
  }

  async function handlePickFolder(title: string) {
    const selected = await open({ directory: true, title });
    if (!selected) return;
    await openWorkspaceWindow(selected, { closeWindow: "main" });
  }

  async function handleRemove(path: string) {
    await removeRecentWorkspace(path);
    setRecents((prev) => prev.filter((w) => w.path !== path));
  }

  return (
    <div className="flex h-screen flex-col bg-background">
      {/* Title Bar */}
      <header
        data-tauri-drag-region
        className="flex h-10 shrink-0 items-center justify-end border-b border-border px-4"
      >
        <div className={isMac ? "pl-17.5" : ""} data-tauri-drag-region />
        {!isMac && (
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => appWindow.minimize()}
              className="flex h-7 w-9 items-center justify-center text-muted-foreground hover:bg-accent"
            >
              <Minus className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              onClick={() => appWindow.close()}
              className="flex h-7 w-9 items-center justify-center text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        )}
      </header>

      <div className="flex min-h-0 flex-1">
        {/* Left Panel: Workspace List */}
        <div className="flex w-72 shrink-0 flex-col border-r border-border bg-muted/40">
          <ScrollArea className="flex-1">
            <div className="py-1">
              {recents.length > 0 ? (
                recents.map((ws) => (
                  <ContextMenu key={ws.path}>
                    <ContextMenuTrigger asChild>
                      <div className="group flex min-w-0 items-center gap-1 border-b border-border/50 px-3 py-3 hover:bg-accent/50">
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <button
                              type="button"
                              className="w-0 flex-1 text-left"
                              onClick={() => handleOpen(ws.path)}
                            >
                              <div className="text-sm font-semibold leading-tight text-foreground">
                                {ws.name}
                              </div>
                              <div className="mt-0.5 truncate text-xs text-muted-foreground">
                                {ws.path}
                              </div>
                            </button>
                          </TooltipTrigger>
                          <TooltipContent side="right" className="max-w-80">
                            <p className="break-all text-xs">{ws.path}</p>
                          </TooltipContent>
                        </Tooltip>

                        <DropdownMenu>
                          <DropdownMenuTrigger asChild>
                            <button
                              type="button"
                              className="flex h-7 w-7 shrink-0 items-center justify-center rounded text-muted-foreground opacity-0 transition-opacity hover:bg-accent group-hover:opacity-100"
                            >
                              <EllipsisVertical className="h-4 w-4" />
                            </button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent side="right" align="start" className="min-w-48">
                            <WorkspaceMenuItems
                              path={ws.path}
                              onRemove={() => handleRemove(ws.path)}
                              Item={DropdownMenuItem}
                              Separator={DropdownMenuSeparator}
                            />
                          </DropdownMenuContent>
                        </DropdownMenu>
                      </div>
                    </ContextMenuTrigger>
                    <ContextMenuContent>
                      <WorkspaceMenuItems
                        path={ws.path}
                        onRemove={() => handleRemove(ws.path)}
                        Item={ContextMenuItem}
                        Separator={ContextMenuSeparator}
                      />
                    </ContextMenuContent>
                  </ContextMenu>
                ))
              ) : (
                <div className="flex flex-col items-center gap-2 py-16 text-center">
                  <FolderPlus className="h-8 w-8 text-muted-foreground/30" />
                  <p className="text-xs text-muted-foreground">
                    <Trans>还没有工作区</Trans>
                  </p>
                </div>
              )}
            </div>
          </ScrollArea>
        </div>

        {/* Right Panel: Brand + Action Cards */}
        <div className="flex flex-1 flex-col items-center justify-center gap-6 p-8">
          <div className="flex flex-col items-center gap-2">
            <div className="flex h-20 w-20 items-center justify-center rounded-2xl bg-primary shadow-md">
              <PenLine className="h-10 w-10 text-primary-foreground" />
            </div>
            <h1 className="text-xl font-bold tracking-tight text-foreground">SwarmNote</h1>
            {appVersion && (
              <p className="text-xs text-muted-foreground">
                <Trans>版本</Trans> {appVersion}
              </p>
            )}
          </div>

          <div className="w-full overflow-hidden rounded-lg border border-border">
            <ActionRow
              primary
              title={<Trans>新建工作区</Trans>}
              description={<Trans>在指定文件夹下创建一个新的工作区。</Trans>}
            >
              <Button size="sm" onClick={() => handlePickFolder(t`选择新工作区目录`)}>
                <Trans>创建</Trans>
              </Button>
            </ActionRow>

            <div className="mx-5 border-t border-border" />

            <ActionRow
              title={<Trans>打开本地工作区</Trans>}
              description={<Trans>将一个本地文件夹作为工作区打开。</Trans>}
            >
              <Button
                size="sm"
                variant="outline"
                onClick={() => handlePickFolder(t`打开工作区文件夹`)}
              >
                <Trans>打开</Trans>
              </Button>
            </ActionRow>

            <div className="mx-5 border-t border-border" />

            <ActionRow
              title={<Trans>同步远程工作区</Trans>}
              description={
                hasOnlineDevices ? (
                  <Trans>从已配对设备同步工作区到本地。{onlineDevices.length} 台设备在线。</Trans>
                ) : (
                  <Trans>将已配对设备的工作区同步到本地。需先启动 P2P 网络。</Trans>
                )
              }
            >
              <Button
                size="sm"
                variant="outline"
                disabled={!hasOnlineDevices}
                onClick={() => setSyncDialogOpen(true)}
              >
                <Trans>同步</Trans>
              </Button>
            </ActionRow>
          </div>
        </div>
      </div>

      <WorkspaceSyncDialog open={syncDialogOpen} onOpenChange={setSyncDialogOpen} />
    </div>
  );
}

export const Route = createFileRoute("/workspace-manager")({
  component: WorkspaceManagerPage,
});
