import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";

import {
  getWorkspaceInfo,
  type HydrateProgress,
  hydrateWorkspace,
  type WorkspaceInfo,
} from "@/commands/workspace";
import { useEditorStore } from "@/stores/editorStore";
import { useFileTreeStore } from "@/stores/fileTreeStore";
import { useNetworkStore } from "@/stores/networkStore";
import { usePreferencesStore } from "@/stores/preferencesStore";

interface WorkspaceState {
  workspace: WorkspaceInfo | null;
  isLoading: boolean;
  error: string | null;
  /** True while hydrate_workspace is running (generating yjs_state for all docs). */
  hydrating: boolean;
  hydrateProgress: HydrateProgress | null;
}

interface WorkspaceActions {
  /** Check if backend already has a workspace loaded (auto-restore). */
  initFromBackend: () => Promise<void>;
  clearWorkspace: () => void;
}

function clearDependentStores() {
  useFileTreeStore.getState().clear();
  useEditorStore.getState().clear();
}

/** 工作区打开后，根据偏好自动启动 P2P 节点 */
function maybeAutoStartP2P() {
  const { autoStartP2P } = usePreferencesStore.getState();
  const { status, userManuallyStopped, startNode } = useNetworkStore.getState();
  if (autoStartP2P && status === "stopped" && !userManuallyStopped) {
    startNode();
  }
}

/** Generation counter to prevent stale hydrate callbacks from clobbering state. */
let hydrateGeneration = 0;

/** 工作区绑定后，运行 hydrate 确保所有文档 yjs_state 有效 */
async function runHydrate(workspaceId: string) {
  const gen = ++hydrateGeneration;
  useWorkspaceStore.setState({ hydrating: true, hydrateProgress: null });
  try {
    await hydrateWorkspace(workspaceId, (progress) => {
      if (gen === hydrateGeneration) {
        useWorkspaceStore.setState({ hydrateProgress: progress });
      }
    });
  } catch (e) {
    console.warn("hydrate_workspace failed:", e);
  } finally {
    if (gen === hydrateGeneration) {
      useWorkspaceStore.setState({ hydrating: false, hydrateProgress: null });
    }
  }
}

export const useWorkspaceStore = create<WorkspaceState & WorkspaceActions>()((set) => ({
  workspace: null,
  isLoading: false,
  error: null,
  hydrating: false,
  hydrateProgress: null,

  initFromBackend: async () => {
    set({ isLoading: true, error: null });
    try {
      // 先注册事件监听，再调用 get_workspace_info，避免新建窗口场景下事件丢失。
      // 工作区的打开路径统一经由 Rust 的 open_workspace_window 命令，
      // Rust 负责在绑定完成后发 "workspace:ready" 事件（对新窗口和
      // fullscreen picker 的 bind-to-caller 场景都适用）。
      let unlistenFn: (() => void) | null = null;
      const unlistenPromise = listen<WorkspaceInfo>("workspace:ready", (event) => {
        set({ workspace: event.payload });
        clearDependentStores();
        maybeAutoStartP2P();
        runHydrate(event.payload.id);
        unlistenFn?.();
      });
      const info = await getWorkspaceInfo();
      unlistenFn = await unlistenPromise;
      if (info) {
        // auto-restore 或新建窗口（Rust 已在建窗口前完成绑定）场景：直接拿到数据
        set({ workspace: info });
        maybeAutoStartP2P();
        // runHydrate 内部 ++hydrateGeneration 会自动使 listener 中可能的旧调用失效
        runHydrate(info.id);
        unlistenFn();
      }
      // info 为 null 时（全新启动无历史工作区）：保留监听，
      // 等待用户从 WorkspacePicker 选择后触发 "workspace:ready"。
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ isLoading: false });
    }
  },

  clearWorkspace: () => {
    ++hydrateGeneration; // Invalidate any in-flight hydrate
    clearDependentStores();
    set({ workspace: null, error: null, hydrating: false, hydrateProgress: null });
  },
}));
