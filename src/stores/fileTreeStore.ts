import { create } from "zustand";
import { commands, events, type FileTreeNode_Serialize as FileTreeNode } from "@/lib/bindings";
import { useEditorStore } from "@/stores/editorStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

interface FileTreeState {
  tree: FileTreeNode[];
  selectedId: string | null;
  isLoading: boolean;
}

interface FileTreeActions {
  rescan: () => Promise<void>;
  selectFile: (id: string | null) => void;
  createFile: (parentRel: string, name: string) => Promise<string>;
  createAndOpenFile: (parentRel: string, name: string) => Promise<string>;
  createDir: (parentRel: string, name: string) => Promise<string>;
  deleteFile: (relPath: string) => Promise<void>;
  deleteDir: (relPath: string) => Promise<void>;
  rename: (relPath: string, newName: string) => Promise<string>;
  /** Move a file or folder; `toRelPath` is the full target path, not a parent dir. */
  move: (fromRelPath: string, toRelPath: string) => Promise<string>;
  clear: () => void;
}

const initialState: FileTreeState = {
  tree: [],
  selectedId: null,
  isLoading: false,
};

export const useFileTreeStore = create<FileTreeState & FileTreeActions>()((set, get) => ({
  ...initialState,

  rescan: async () => {
    set({ isLoading: true });
    try {
      const tree = await commands.scanWorkspaceTree();
      set({ tree });
    } finally {
      set({ isLoading: false });
    }
  },

  selectFile: (id) => set({ selectedId: id }),

  createFile: async (parentRel, name) => {
    const relPath = await commands.fsCreateFile(parentRel, name);
    const workspace = useWorkspaceStore.getState().workspace;
    if (workspace) {
      const title = relPath.split("/").pop() ?? name;
      await commands.dbUpsertDocument({
        id: null,
        workspace_id: workspace.id,
        folder_id: null,
        title,
        rel_path: relPath,
        file_hash: null,
      });
    }
    await get().rescan();
    return relPath;
  },

  createAndOpenFile: async (parentRel, name) => {
    const relPath = await get().createFile(parentRel, name);
    const title = relPath.split("/").pop() ?? name;
    set({ selectedId: relPath });
    await useEditorStore.getState().loadDocument(relPath, title, relPath);
    return relPath;
  },

  createDir: async (parentRel, name) => {
    const relPath = await commands.fsCreateDir(parentRel, name);
    await get().rescan();
    return relPath;
  },

  deleteFile: async (relPath) => {
    await commands.fsDeleteFile(relPath);
    await commands.deleteDocumentByRelPath(relPath);
    const { selectedId } = get();
    if (selectedId === relPath) {
      set({ selectedId: null });
      useEditorStore.getState().clear();
    }
    await get().rescan();
  },

  deleteDir: async (relPath) => {
    await commands.deleteDocumentsByPrefix(`${relPath}/`);
    await commands.fsDeleteDir(relPath);
    await get().rescan();
  },

  rename: async (relPath, newName) => {
    const newRelPath = await commands.fsRename(relPath, newName);
    const newTitle = newRelPath.split("/").pop()?.replace(/\.md$/i, "") ?? newName;
    await commands.renameDocument({
      oldRelPath: relPath,
      newRelPath,
      newTitle,
    });
    const { selectedId } = get();
    if (selectedId === relPath) {
      set({ selectedId: newRelPath });
    }
    await get().rescan();
    return newRelPath;
  },

  move: async (fromRelPath, toRelPath) => {
    const result = await commands.moveDocument({
      fromRelPath,
      toRelPath,
    });
    // Rebase any path that was equal to `from` or (for a moved folder) a
    // descendant of `from`. Returns `null` when the path is unaffected.
    const rebase = (path: string | null): string | null => {
      if (path === fromRelPath) return result.newRelPath;
      if (result.isDir && path?.startsWith(`${fromRelPath}/`)) {
        return `${result.newRelPath}/${path.slice(fromRelPath.length + 1)}`;
      }
      return null;
    };

    const nextSelected = rebase(get().selectedId);
    if (nextSelected !== null) set({ selectedId: nextSelected });

    const editor = useEditorStore.getState();
    const nextEditorPath = rebase(editor.currentDocId);
    if (nextEditorPath !== null) {
      editor.updateRelPath(nextEditorPath, nextEditorPath.split("/").pop() ?? "");
    }

    await get().rescan();
    return result.newRelPath;
  },

  clear: () => set(initialState),
}));

// Register file-tree-changed listener with throttle
let throttleTimer: ReturnType<typeof setTimeout> | null = null;

events.fileTreeChanged.listen(() => {
  if (throttleTimer) return;
  throttleTimer = setTimeout(() => {
    throttleTimer = null;
    useFileTreeStore.getState().rescan();
  }, 200);
});
