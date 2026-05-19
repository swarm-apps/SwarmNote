import { FolderOpen } from "lucide-react";

import type { RecentWorkspace } from "@/lib/bindings";
import { formatRelativeTime } from "@/lib/dateUtils";

interface WorkspaceItemProps {
  workspace: RecentWorkspace;
  onClick: (path: string) => void;
}

export function WorkspaceItem({ workspace, onClick }: WorkspaceItemProps) {
  const timeAgo = formatRelativeTime(workspace.last_opened_at);

  return (
    <button
      type="button"
      onClick={() => onClick(workspace.path)}
      className="flex w-full items-center gap-3 rounded-lg border border-border bg-card p-3 text-left transition-colors hover:bg-accent"
    >
      <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-muted">
        <FolderOpen className="h-5 w-5 text-muted-foreground" />
      </div>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="text-sm font-medium text-foreground">{workspace.name}</span>
        <span className="truncate text-xs text-muted-foreground">{workspace.path}</span>
      </div>
      <span className="shrink-0 text-xs text-muted-foreground">{timeAgo}</span>
    </button>
  );
}
