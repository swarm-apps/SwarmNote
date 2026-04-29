import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import {
  deleteDocumentByRelPath,
  deleteDocumentsByPrefix,
  moveDocument,
  renameDocument,
  upsertDocument,
} from "@/commands/document";
import {
  type FileTreeNode,
  fsCreateDir,
  fsCreateFile,
  fsDeleteDir,
  fsDeleteFile,
  fsRename,
  scanWorkspaceTree,
} from "@/commands/fs";
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
      const tree = await scanWorkspaceTree();
      set({ tree });
    } finally {
      set({ isLoading: false });
    }
  },

  selectFile: (id) => set({ selectedId: id }),

  createFile: async (parentRel, name) => {
    const relPath = await fsCreateFile(parentRel, name);
    const workspace = useWorkspaceStore.getState().workspace;
    if (workspace) {
      const title = relPath.split("/").pop() ?? name;
      await upsertDocument({
        workspace_id: workspace.id,
        title,
        rel_path: relPath,
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
    const relPath = await fsCreateDir(parentRel, name);
    await get().rescan();
    return relPath;
  },

  deleteFile: async (relPath) => {
    await fsDeleteFile(relPath);
    await deleteDocumentByRelPath(relPath);
    const { selectedId } = get();
    if (selectedId === relPath) {
      set({ selectedId: null });
      useEditorStore.getState().clear();
    }
    await get().rescan();
  },

  deleteDir: async (relPath) => {
    await deleteDocumentsByPrefix(`${relPath}/`);
    await fsDeleteDir(relPath);
    await get().rescan();
  },

  rename: async (relPath, newName) => {
    const newRelPath = await fsRename(relPath, newName);
    const newTitle = newRelPath.split("/").pop()?.replace(/\.md$/i, "") ?? newName;
    await renameDocument(relPath, newRelPath, newTitle);
    const { selectedId } = get();
    if (selectedId === relPath) {
      set({ selectedId: newRelPath });
    }
    await get().rescan();
    return newRelPath;
  },

  move: async (fromRelPath, toRelPath) => {
    const result = await moveDocument(fromRelPath, toRelPath);
    // Rebase any path that was equal to `from` or (for a moved folder) a
    // descendant of `from`. Returns `null` when the path is unaffected.
    const rebase = (path: string | null): string | null => {
      if (path === fromRelPath) return result.new_rel_path;
      if (result.is_dir && path?.startsWith(`${fromRelPath}/`)) {
        return `${result.new_rel_path}/${path.slice(fromRelPath.length + 1)}`;
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
    return result.new_rel_path;
  },

  clear: () => set(initialState),
}));

// Register fs:tree-changed listener with throttle
let throttleTimer: ReturnType<typeof setTimeout> | null = null;

listen("fs:tree-changed", () => {
  if (throttleTimer) return;
  throttleTimer = setTimeout(() => {
    throttleTimer = null;
    useFileTreeStore.getState().rescan();
  }, 200);
});
