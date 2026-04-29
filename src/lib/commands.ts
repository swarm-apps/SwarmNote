import { useLingui } from "@lingui/react/macro";
import {
  Code,
  FilePlus,
  FileText,
  Image as ImageIcon,
  Link as LinkIcon,
  type LucideIcon,
  PanelLeft,
  Settings as SettingsIcon,
  Table as TableIcon,
} from "lucide-react";
import { useMemo } from "react";
import { openSettingsWindow } from "@/commands/workspace";
import { modKey } from "@/lib/utils";
import { useEditorStore } from "@/stores/editorStore";
import { useFileTreeStore } from "@/stores/fileTreeStore";
import { useUIStore } from "@/stores/uiStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

export type CommandCategory = "action" | "recent";

export interface Command {
  id: string;
  label: string;
  category: CommandCategory;
  shortcut?: string;
  icon: LucideIcon;
  run: () => void | Promise<void>;
}

/**
 * Returns the full list of Command Palette entries for the current state.
 * Built as a hook so every entry's translations and handlers stay reactive.
 * Recent files come from editorStore.recentDocs for the active workspace.
 */
export function useCommands(): { actions: Command[]; recents: Command[] } {
  const { t } = useLingui();

  // Subscribe to the derived boolean, not the control instance itself —
  // the palette only needs to know whether editor commands are available.
  // Avoids re-renders when the control reference changes but availability doesn't.
  const editorOpen = useEditorStore((s) => s.editorControl !== null);

  return useMemo<{ actions: Command[]; recents: Command[] }>(() => {
    const actions: Command[] = [
      {
        id: "new-note",
        label: t`新建笔记`,
        category: "action",
        icon: FilePlus,
        shortcut: `${modKey}N`,
        run: async () => {
          const createAndOpenFile = useFileTreeStore.getState().createAndOpenFile;
          await createAndOpenFile("", t`新建笔记`);
        },
      },
      {
        id: "toggle-sidebar",
        label: t`切换侧边栏`,
        category: "action",
        icon: PanelLeft,
        shortcut: `${modKey}B`,
        run: () => {
          useUIStore.getState().toggleSidebar();
        },
      },
      {
        id: "open-settings",
        label: t`打开设置`,
        category: "action",
        icon: SettingsIcon,
        shortcut: `${modKey},`,
        run: async () => {
          await openSettingsWindow("general");
        },
      },
    ];

    if (editorOpen) {
      actions.push(
        {
          id: "insert-code-block",
          label: t`插入代码块`,
          category: "action",
          icon: Code,
          run: () => {
            useEditorStore.getState().editorControl?.execCommand("insertCodeBlock");
          },
        },
        {
          id: "insert-table",
          label: t`插入表格`,
          category: "action",
          icon: TableIcon,
          run: () => {
            useEditorStore.getState().editorControl?.execCommand("insertTable");
          },
        },
        {
          id: "insert-link",
          label: t`插入链接`,
          category: "action",
          icon: LinkIcon,
          shortcut: `${modKey}K`,
          run: () => {
            useEditorStore.getState().editorControl?.execCommand("insertLink");
          },
        },
        {
          id: "insert-image",
          label: t`插入图片`,
          category: "action",
          icon: ImageIcon,
          run: () => {
            // Insert an editable template; user replaces the url placeholder.
            const control = useEditorStore.getState().editorControl;
            if (!control) return;
            const { view } = control;
            const { from, to } = view.state.selection.main;
            const template = "![alt](url)";
            const urlStart = from + "![alt](".length;
            const urlEnd = urlStart + "url".length;
            view.dispatch({
              changes: { from, to, insert: template },
              selection: { anchor: urlStart, head: urlEnd },
            });
            view.focus();
          },
        },
      );
    }

    const workspaceId = useWorkspaceStore.getState().workspace?.id;
    const recentDocs = workspaceId ? (useEditorStore.getState().recentDocs[workspaceId] ?? []) : [];

    const recents: Command[] = recentDocs.map((doc) => ({
      id: `recent:${doc.id}`,
      label: doc.title.replace(/\.md$/i, ""),
      category: "recent",
      icon: FileText,
      run: () => {
        useEditorStore.getState().loadDocument(doc.id, doc.title, doc.relPath);
      },
    }));

    return { actions, recents };
  }, [t, editorOpen]);
}
