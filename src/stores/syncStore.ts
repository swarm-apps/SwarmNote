import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { create } from "zustand";

// ── Types ──

interface ActiveSync {
  workspaceUuid: string;
  peerId: string;
  completed: number;
  total: number;
}

type SyncResult = "success" | "cancelled" | "error" | "partial";

interface LastSyncResult {
  lastSyncedAt: number;
  result: SyncResult;
}

interface SyncState {
  /** 正在进行的同步，key 为 `${workspaceUuid}:${peerId}` */
  activeSyncs: Record<string, ActiveSync>;
  /** 上次同步结果，key 为 workspaceUuid */
  lastSyncResults: Record<string, LastSyncResult>;
}

interface SyncActions {
  /** 清除所有状态（节点停止时调用） */
  reset: () => void;
}

// ── Event Payloads ──

interface SyncStartedPayload {
  peerId: string;
  workspaceUuid: string;
}

interface SyncProgressPayload {
  peerId: string;
  workspaceUuid: string;
  completed: number;
  total: number;
}

interface SyncCompletedPayload {
  peerId: string;
  workspaceUuid: string;
  result: SyncResult;
}

// ── Store ──

export const useSyncStore = create<SyncState & SyncActions>()((set) => ({
  activeSyncs: {},
  lastSyncResults: {},

  reset: () => set({ activeSyncs: {}, lastSyncResults: {} }),
}));

// ── Tauri Event Listeners ──

let unlisteners: UnlistenFn[] = [];

function syncKey(workspaceUuid: string, peerId: string) {
  return `${workspaceUuid}:${peerId}`;
}

export async function setupSyncListeners() {
  await cleanupSyncListeners();

  const u1 = await listen<SyncStartedPayload>("sync-started", (event) => {
    const { peerId, workspaceUuid } = event.payload;
    const key = syncKey(workspaceUuid, peerId);
    useSyncStore.setState((state) => ({
      activeSyncs: {
        ...state.activeSyncs,
        [key]: { workspaceUuid, peerId, completed: 0, total: 0 },
      },
    }));
  });

  const u2 = await listen<SyncProgressPayload>("sync-progress", (event) => {
    const { peerId, workspaceUuid, completed, total } = event.payload;
    const key = syncKey(workspaceUuid, peerId);
    useSyncStore.setState((state) => {
      if (!state.activeSyncs[key]) return state;
      return {
        activeSyncs: {
          ...state.activeSyncs,
          [key]: { ...state.activeSyncs[key], completed, total },
        },
      };
    });
  });

  const u3 = await listen<SyncCompletedPayload>("sync-completed", (event) => {
    const { peerId, workspaceUuid, result } = event.payload;
    const key = syncKey(workspaceUuid, peerId);
    useSyncStore.setState((state) => {
      const { [key]: _, ...remaining } = state.activeSyncs;
      return {
        activeSyncs: remaining,
        lastSyncResults: {
          ...state.lastSyncResults,
          [workspaceUuid]: { lastSyncedAt: Date.now(), result },
        },
      };
    });
  });

  unlisteners = [u1, u2, u3];
}

export async function cleanupSyncListeners() {
  for (const unlisten of unlisteners) {
    unlisten();
  }
  unlisteners = [];
}

// Auto-register listeners on module load
setupSyncListeners().catch(() => {});
