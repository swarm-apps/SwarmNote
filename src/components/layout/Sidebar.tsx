import { Trans, useLingui } from "@lingui/react/macro";
import { FilePlus, FolderPlus, Search, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { DocumentOutline } from "@/components/editor/DocumentOutline";
import { FileTree } from "@/components/filetree/FileTree";
import { SyncStatusBar } from "@/components/layout/SyncStatusBar";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useFileTreeStore } from "@/stores/fileTreeStore";
import { SIDEBAR_WIDTH_MAX, SIDEBAR_WIDTH_MIN, useUIStore } from "@/stores/uiStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

export function Sidebar() {
  const { t } = useLingui();
  const sidebarOpen = useUIStore((s) => s.sidebarOpen);
  const sidebarWidth = useUIStore((s) => s.sidebarWidth);
  const setSidebarWidth = useUIStore((s) => s.setSidebarWidth);
  const sidebarTab = useUIStore((s) => s.sidebarTab);
  const workspace = useWorkspaceStore((s) => s.workspace);
  const rescan = useFileTreeStore((s) => s.rescan);
  const createAndOpenFile = useFileTreeStore((s) => s.createAndOpenFile);
  const createDir = useFileTreeStore((s) => s.createDir);

  const handleResizeMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const startX = e.clientX;
      const startWidth = useUIStore.getState().sidebarWidth;

      const onMove = (moveEvent: MouseEvent) => {
        setSidebarWidth(startWidth + (moveEvent.clientX - startX));
      };

      const onUp = () => {
        document.body.style.removeProperty("cursor");
        document.body.style.removeProperty("user-select");
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
      };

      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
    },
    [setSidebarWidth],
  );

  const handleResizeKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const STEP = 16;
      const current = useUIStore.getState().sidebarWidth;
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        setSidebarWidth(current - STEP);
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        setSidebarWidth(current + STEP);
      } else if (e.key === "Home") {
        e.preventDefault();
        setSidebarWidth(SIDEBAR_WIDTH_MIN);
      } else if (e.key === "End") {
        e.preventDefault();
        setSidebarWidth(SIDEBAR_WIDTH_MAX);
      }
    },
    [setSidebarWidth],
  );

  const workspaceUuid = workspace?.id;

  useEffect(() => {
    if (workspace) {
      rescan();
    }
  }, [workspace, rescan]);

  const handleCreateFile = useCallback(() => {
    createAndOpenFile("", t`新建笔记`);
  }, [createAndOpenFile, t]);

  const handleCreateDir = useCallback(() => {
    createDir("", t`新建文件夹`);
  }, [createDir, t]);

  const [searchTerm, setSearchTerm] = useState("");
  const [treeHeight, setTreeHeight] = useState(400);
  const treeContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = treeContainerRef.current;
    if (!el) return;

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setTreeHeight(entry.contentRect.height);
      }
    });

    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <aside
      className="group/sidebar relative flex shrink-0 flex-col overflow-hidden border-r border-sidebar-border bg-sidebar transition-[width] duration-200 ease-in-out"
      style={{ width: sidebarOpen ? sidebarWidth : 0 }}
    >
      <div className="flex h-full flex-col" style={{ minWidth: sidebarWidth }}>
        {/* File tree header: search + actions (only in filetree mode) */}
        {sidebarTab === "filetree" && (
          <div className="flex shrink-0 items-center gap-1 px-2 py-1.5">
            <div className="flex min-w-0 flex-1 items-center gap-1.5 rounded-md border border-sidebar-border bg-sidebar px-2">
              <Search className="h-3 w-3 shrink-0 text-muted-foreground" />
              <input
                type="text"
                value={searchTerm}
                onChange={(e) => setSearchTerm(e.target.value)}
                placeholder={t`搜索文件...`}
                className="min-w-0 flex-1 bg-transparent py-1 text-xs text-sidebar-foreground outline-none placeholder:text-muted-foreground"
              />
              {searchTerm && (
                <button
                  type="button"
                  onClick={() => setSearchTerm("")}
                  className="shrink-0 text-muted-foreground hover:text-sidebar-foreground"
                >
                  <X className="h-3 w-3" />
                </button>
              )}
            </div>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  className="shrink-0 text-muted-foreground"
                  onClick={handleCreateFile}
                >
                  <FilePlus className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <Trans>新建文件</Trans>
              </TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  className="shrink-0 text-muted-foreground"
                  onClick={handleCreateDir}
                >
                  <FolderPlus className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <Trans>新建文件夹</Trans>
              </TooltipContent>
            </Tooltip>
          </div>
        )}

        {/* Content area */}
        <div ref={treeContainerRef} className="flex-1 overflow-hidden">
          {sidebarTab === "filetree" ? (
            <FileTree
              width={sidebarWidth}
              height={treeHeight}
              searchTerm={searchTerm || undefined}
            />
          ) : (
            <DocumentOutline height={treeHeight} />
          )}
        </div>

        {/* Sync status indicator */}
        <SyncStatusBar workspaceUuid={workspaceUuid} />
      </div>

      {/* Resize handle */}
      {sidebarOpen && (
        <button
          type="button"
          aria-label={t`调整侧边栏宽度 (当前 ${sidebarWidth} 像素)`}
          onMouseDown={handleResizeMouseDown}
          onKeyDown={handleResizeKeyDown}
          className="absolute top-0 right-0 bottom-0 z-10 w-1 cursor-col-resize border-0 bg-transparent p-0 hover:bg-primary/20 active:bg-primary/30 focus-visible:bg-primary/30 focus-visible:outline-none"
        />
      )}
    </aside>
  );
}
