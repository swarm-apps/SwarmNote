import type { EditorControl } from "@swarmnote/editor";
import { create } from "zustand";
import { persist } from "zustand/middleware";
import { createTauriStorage, waitForHydration } from "@/lib/tauriStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

const RECENT_DOCS_LIMIT = 10;

export interface RecentDoc {
  id: string;
  title: string;
  relPath: string;
  openedAt: number;
}

interface EditorState {
  currentDocId: string | null;
  /** Stable UUID from the database (set after open_ydoc returns). */
  docUuid: string | null;
  title: string;
  relPath: string;
  isDirty: boolean;
  lastSavedAt: Date | null;
  charCount: number;
  /** Recently opened documents, keyed by workspace id. Most-recent first, capped at 10 per workspace. */
  recentDocs: Record<string, RecentDoc[]>;
  /** Transient: current CM6 EditorControl (not persisted). */
  editorControl: EditorControl | null;
  /** Counter bumped on every editor content change; drives outline re-parse. */
  editorChangeTick: number;
}

interface EditorActions {
  loadDocument: (id: string, title: string, relPath: string) => void;
  setDocUuid: (uuid: string) => void;
  updateTitle: (title: string) => void;
  updateRelPath: (newRelPath: string, newTitle: string) => void;
  markDirty: () => void;
  markFlushed: (lastSavedAt: Date) => void;
  setCharCount: (count: number) => void;
  clear: () => void;
  /** Drop recentDocs entries for workspace ids not present in the given set. */
  pruneRecentDocs: (validWorkspaceIds: Set<string>) => void;
  setEditorControl: (control: EditorControl | null) => void;
  bumpEditorChangeTick: () => void;
}

const ephemeralInitial = {
  currentDocId: null,
  docUuid: null,
  title: "",
  relPath: "",
  isDirty: false,
  lastSavedAt: null,
  charCount: 0,
};

const initialState: EditorState = {
  ...ephemeralInitial,
  recentDocs: {},
  editorControl: null,
  editorChangeTick: 0,
};

export const useEditorStore = create<EditorState & EditorActions>()(
  persist(
    (set) => ({
      ...initialState,

      loadDocument: (id, title, relPath) => {
        const workspaceId = useWorkspaceStore.getState().workspace?.id;
        set((state) => {
          const nextRecent: Record<string, RecentDoc[]> = { ...state.recentDocs };
          if (workspaceId) {
            const existing = nextRecent[workspaceId] ?? [];
            const filtered = existing.filter((d) => d.id !== id);
            const entry: RecentDoc = { id, title, relPath, openedAt: Date.now() };
            nextRecent[workspaceId] = [entry, ...filtered].slice(0, RECENT_DOCS_LIMIT);
          }
          return {
            ...ephemeralInitial,
            currentDocId: id,
            title,
            relPath,
            recentDocs: nextRecent,
          };
        });
      },

      setDocUuid: (uuid) => set({ docUuid: uuid }),

      updateTitle: (title) => set({ title }),

      updateRelPath: (newRelPath, newTitle) =>
        set((state) => {
          const workspaceId = useWorkspaceStore.getState().workspace?.id;
          const nextRecent: Record<string, RecentDoc[]> = { ...state.recentDocs };
          if (workspaceId && state.currentDocId) {
            const oldId = state.currentDocId;
            const existing = nextRecent[workspaceId] ?? [];
            nextRecent[workspaceId] = existing.map((d) =>
              d.id === oldId ? { ...d, id: newRelPath, title: newTitle, relPath: newRelPath } : d,
            );
          }
          return {
            currentDocId: newRelPath,
            relPath: newRelPath,
            title: newTitle,
            recentDocs: nextRecent,
          };
        }),

      markDirty: () => set((state) => (state.isDirty ? state : { isDirty: true })),

      markFlushed: (lastSavedAt) => set({ isDirty: false, lastSavedAt }),

      setCharCount: (count) => set({ charCount: count }),

      clear: () => set((state) => ({ ...ephemeralInitial, recentDocs: state.recentDocs })),

      pruneRecentDocs: (validWorkspaceIds) =>
        set((state) => {
          const next: Record<string, RecentDoc[]> = {};
          for (const [wsId, docs] of Object.entries(state.recentDocs)) {
            if (validWorkspaceIds.has(wsId)) next[wsId] = docs;
          }
          return { recentDocs: next };
        }),

      setEditorControl: (control) => set({ editorControl: control }),
      bumpEditorChangeTick: () =>
        set((state) => ({ editorChangeTick: state.editorChangeTick + 1 })),
    }),
    {
      name: "swarmnote-editor",
      storage: createTauriStorage("settings.json"),
      partialize: (state) => ({ recentDocs: state.recentDocs }),
    },
  ),
);

export const waitForEditorHydration = () => waitForHydration(useEditorStore);
