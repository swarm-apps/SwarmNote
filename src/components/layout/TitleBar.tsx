import { useLingui } from "@lingui/react/macro";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ChevronsUpDown,
  FolderTree,
  List,
  Minus,
  PanelLeft,
  PenLine,
  Search,
  Settings,
  Square,
  X,
} from "lucide-react";
import { openSettingsWindow } from "@/commands/workspace";
import { OPEN_COMMAND_PALETTE } from "@/components/layout/CommandPalette";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { WorkspacePopover } from "@/components/workspace/WorkspacePopover";
import { isMac, modKey } from "@/lib/utils";
import { type SidebarTab, useUIStore } from "@/stores/uiStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

export function TitleBar() {
  const { t } = useLingui();
  const appWindow = getCurrentWindow();
  const sidebarOpen = useUIStore((s) => s.sidebarOpen);
  const toggleSidebar = useUIStore((s) => s.toggleSidebar);
  const sidebarTab = useUIStore((s) => s.sidebarTab);
  const setSidebarTab = useUIStore((s) => s.setSidebarTab);
  const workspace = useWorkspaceStore((s) => s.workspace);

  const needsTrafficLightPadding = isMac;

  return (
    <header
      data-tauri-drag-region
      className="flex h-10 shrink-0 items-center justify-between border-b border-border bg-card px-3"
    >
      {/* Left: Logo + Workspace + Sidebar Controls */}
      <div
        className={`flex items-center gap-2 ${needsTrafficLightPadding ? "pl-17.5" : ""}`}
        data-tauri-drag-region
      >
        {/* Logo */}
        <div className="flex items-center gap-1.5">
          <div className="flex h-5.5 w-5.5 items-center justify-center rounded bg-primary">
            <PenLine className="h-3.5 w-3.5 text-white" />
          </div>
          <span className="text-sm font-semibold text-foreground">SwarmNote</span>
        </div>

        <div className="h-4 w-px bg-border" />

        {/* Workspace switcher */}
        <WorkspacePopover side="bottom">
          <button
            type="button"
            className="flex items-center gap-1 rounded-md px-1.5 py-1 text-[13px] font-medium text-foreground hover:bg-muted"
          >
            <span className="max-w-32 truncate">{workspace?.name ?? "SwarmNote"}</span>
            <ChevronsUpDown className="h-3 w-3 shrink-0 text-muted-foreground" />
          </button>
        </WorkspacePopover>

        <div className="h-4 w-px bg-border" />

        {/* Sidebar toggle + view switch + actions */}
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon-xs" onClick={toggleSidebar}>
              <PanelLeft className="h-3.5 w-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            {sidebarOpen ? t`收起侧边栏` : t`展开侧边栏`} ({modKey}B)
          </TooltipContent>
        </Tooltip>

        {sidebarOpen && (
          <ToggleGroup
            type="single"
            size="sm"
            variant="outline"
            value={sidebarTab}
            onValueChange={(v) => {
              if (v) setSidebarTab(v as SidebarTab);
            }}
          >
            <ToggleGroupItem value="filetree" aria-label={t`文件树`}>
              <FolderTree className="h-3.5 w-3.5" />
            </ToggleGroupItem>
            <ToggleGroupItem value="outline" aria-label={t`大纲`}>
              <List className="h-3.5 w-3.5" />
            </ToggleGroupItem>
          </ToggleGroup>
        )}
      </div>

      {/* Right: Command Palette + Settings + Window Controls */}
      <div className="flex items-center gap-1">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={() => document.dispatchEvent(new CustomEvent(OPEN_COMMAND_PALETTE))}
            >
              <Search className="h-3.5 w-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            {t`命令面板`} ({modKey}P)
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon-xs" onClick={() => openSettingsWindow("general")}>
              <Settings className="h-3.5 w-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t`设置`}</TooltipContent>
        </Tooltip>

        {!isMac && (
          <>
            <div className="h-4 w-px bg-border" />
            <button
              type="button"
              onClick={() => appWindow.minimize()}
              className="flex h-7 w-9 items-center justify-center text-muted-foreground hover:bg-muted"
            >
              <Minus className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              onClick={() => appWindow.toggleMaximize()}
              className="flex h-7 w-9 items-center justify-center text-muted-foreground hover:bg-muted"
            >
              <Square className="h-3 w-3" />
            </button>
            <button
              type="button"
              onClick={() => appWindow.close()}
              className="flex h-7 w-9 items-center justify-center text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </>
        )}
      </div>
    </header>
  );
}
