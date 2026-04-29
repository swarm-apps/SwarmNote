import { Trans } from "@lingui/react/macro";
import { ArrowUpRight, Check, Settings2 } from "lucide-react";
import { useEffect, useState } from "react";

import {
  getRecentWorkspaces,
  openWorkspaceManagerWindow,
  openWorkspaceWindow,
  type RecentWorkspace,
} from "@/commands/workspace";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { useWorkspaceStore } from "@/stores/workspaceStore";

interface WorkspacePopoverProps {
  children: React.ReactNode;
  side?: "top" | "bottom" | "left" | "right";
}

export function WorkspacePopover({ children, side = "bottom" }: WorkspacePopoverProps) {
  const [open, setOpen] = useState(false);
  const [recents, setRecents] = useState<RecentWorkspace[]>([]);
  const workspace = useWorkspaceStore((s) => s.workspace);
  useEffect(() => {
    if (open) {
      getRecentWorkspaces().then(setRecents);
    }
  }, [open]);

  async function handleSelect(path: string) {
    await openWorkspaceWindow(path);
    setOpen(false);
  }

  async function handleManage() {
    setOpen(false);
    await openWorkspaceManagerWindow();
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>{children}</PopoverTrigger>
      <PopoverContent className="w-64 p-1" side={side} align="start">
        <div className="flex flex-col">
          {recents.map((ws) => {
            const isCurrent = workspace?.path === ws.path;
            return (
              <button
                key={ws.path}
                type="button"
                disabled={isCurrent}
                onClick={isCurrent ? undefined : () => handleSelect(ws.path)}
                className={
                  isCurrent
                    ? "flex cursor-not-allowed items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm opacity-70"
                    : "flex items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm hover:bg-accent"
                }
              >
                {isCurrent ? (
                  <Check className="h-3.5 w-3.5 shrink-0 text-primary" />
                ) : (
                  <ArrowUpRight className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                )}
                <span className="min-w-0 flex-1 truncate text-foreground">{ws.name}</span>
              </button>
            );
          })}

          {recents.length > 0 && <Separator className="my-1" />}

          <button
            type="button"
            onClick={handleManage}
            className="flex items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm hover:bg-accent"
          >
            <Settings2 className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-foreground">
              <Trans>工作区管理...</Trans>
            </span>
          </button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
