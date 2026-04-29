import { Trans, useLingui } from "@lingui/react/macro";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, FolderPlus, Plus, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";

import {
  getRecentWorkspaces,
  openWorkspaceWindow,
  type RecentWorkspace,
} from "@/commands/workspace";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { WorkspaceItem } from "@/components/workspace/WorkspaceItem";
import { WorkspaceSyncDialog } from "@/components/workspace/WorkspaceSyncDialog";
import { useNetworkStore } from "@/stores/networkStore";

interface WorkspacePickerProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function WorkspacePicker({ open: dialogOpen, onOpenChange }: WorkspacePickerProps) {
  const { t } = useLingui();
  const [recents, setRecents] = useState<RecentWorkspace[]>([]);
  const [syncDialogOpen, setSyncDialogOpen] = useState(false);
  const devices = useNetworkStore((s) => s.devices);

  const onlineDevices = devices.filter((d) => d.status === "online" && d.isPaired);
  const hasOnlineDevices = onlineDevices.length > 0;

  useEffect(() => {
    if (dialogOpen) {
      getRecentWorkspaces().then(setRecents);
    }
  }, [dialogOpen]);

  async function openPath(path: string) {
    await openWorkspaceWindow(path);
    onOpenChange?.(false);
  }

  async function handleSelectWorkspace(path: string) {
    await openPath(path);
  }

  async function handleOpenFolder() {
    const selected = await open({ directory: true, title: t`打开工作区文件夹` });
    if (!selected) return;
    await openPath(selected);
  }

  async function handleCreateWorkspace() {
    const selected = await open({ directory: true, title: t`选择新工作区目录` });
    if (!selected) return;
    await openPath(selected);
  }

  const content = (
    <div className="flex min-w-0 flex-col gap-6">
      {/* Actions */}
      <div className="flex gap-3">
        <Button variant="outline" className="flex-1 gap-1.5" onClick={handleCreateWorkspace}>
          <Plus className="h-4 w-4" />
          <Trans>创建新工作区</Trans>
        </Button>
        <Button variant="outline" className="flex-1 gap-1.5" onClick={handleOpenFolder}>
          <FolderOpen className="h-4 w-4" />
          <Trans>打开文件夹</Trans>
        </Button>
      </div>

      {/* Sync from paired devices */}
      {hasOnlineDevices && (
        <div className="flex flex-col gap-1.5">
          <Button
            variant="outline"
            className="w-full gap-1.5"
            onClick={() => setSyncDialogOpen(true)}
          >
            <RefreshCw className="h-4 w-4" />
            <Trans>同步已配对设备工作区</Trans>
          </Button>
          <p className="text-center text-xs text-muted-foreground">
            <Trans>{onlineDevices.length} 台设备在线，可同步工作区</Trans>
          </p>
        </div>
      )}

      {/* Recent workspaces */}
      {recents.length > 0 ? (
        <div className="flex min-h-0 flex-col gap-2">
          <span className="text-xs font-medium text-muted-foreground">
            <Trans>最近打开</Trans>
          </span>
          <div className="flex max-h-48 flex-col gap-1.5 overflow-y-auto">
            {recents.map((ws) => (
              <WorkspaceItem key={ws.path} workspace={ws} onClick={handleSelectWorkspace} />
            ))}
          </div>
        </div>
      ) : (
        <div className="flex flex-col items-center gap-2 py-8">
          <FolderPlus className="h-10 w-10 text-muted-foreground/40" />
          <p className="text-sm text-muted-foreground">
            <Trans>还没有工作区，创建一个开始使用吧。</Trans>
          </p>
        </div>
      )}
    </div>
  );

  return (
    <>
      <Dialog open={dialogOpen} onOpenChange={onOpenChange}>
        <DialogContent className="max-h-[80vh] overflow-x-hidden overflow-y-auto">
          <DialogHeader>
            <DialogTitle>
              <Trans>工作区管理</Trans>
            </DialogTitle>
          </DialogHeader>
          {content}
        </DialogContent>
      </Dialog>
      <WorkspaceSyncDialog open={syncDialogOpen} onOpenChange={setSyncDialogOpen} />
    </>
  );
}
