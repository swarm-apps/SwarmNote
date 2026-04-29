import { i18n } from "@lingui/core";
import { useEffect } from "react";
import { openSettingsWindow } from "@/commands/workspace";
import { OPEN_COMMAND_PALETTE } from "@/components/layout/CommandPalette";
import { isMac } from "@/lib/utils";
import { useFileTreeStore } from "@/stores/fileTreeStore";
import { useUIStore } from "@/stores/uiStore";

export function useKeyboardShortcuts() {
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Let editor-owned shortcuts (e.g. CM6's Ctrl+B = bold) win when they've
      // already claimed the event — CM6 calls preventDefault() before the event
      // bubbles up here.
      if (e.defaultPrevented) return;

      const mod = isMac ? e.metaKey : e.ctrlKey;
      if (!mod) return;

      // Ctrl+Shift+O: open workspace picker
      if (e.shiftKey && e.key.toLowerCase() === "o") {
        e.preventDefault();
        const { setWorkspacePickerOpen, workspacePickerOpen } = useUIStore.getState();
        setWorkspacePickerOpen(!workspacePickerOpen);
        return;
      }

      switch (e.key.toLowerCase()) {
        case "b":
          e.preventDefault();
          useUIStore.getState().toggleSidebar();
          break;
        case "n":
          e.preventDefault();
          useFileTreeStore.getState().createAndOpenFile("", i18n._("新建笔记"));
          break;
        case "p":
          e.preventDefault();
          document.dispatchEvent(new CustomEvent(OPEN_COMMAND_PALETTE));
          break;
        case ",":
          e.preventDefault();
          openSettingsWindow("general");
          break;
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);
}
