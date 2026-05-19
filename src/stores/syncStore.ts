import type { UnlistenFn } from "@tauri-apps/api/event";
import { create } from "zustand";

import { events, type SyncResult } from "@/lib/bindings";

// ── Types ──

interface ActiveSync {
  workspaceUuid: string;
  peerId: string;
  completed: number;
  total: number;
}

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

  const u1 = await events.syncStarted.listen((event) => {
    const { peerId, workspaceUuid } = event.payload;
    const key = syncKey(workspaceUuid, peerId);
    useSyncStore.setState((state) => ({
      activeSyncs: {
        ...state.activeSyncs,
        [key]: { workspaceUuid, peerId, completed: 0, total: 0 },
      },
    }));
  });

  const u2 = await events.syncProgress.listen((event) => {
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

  const u3 = await events.syncCompleted.listen((event) => {
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
