/**
 * Pairing Code Store
 *
 * 全局单例的配对码管理，对齐 SwarmDrop / SwarmDrop-RN：
 *   - 跨组件 mount/unmount 持久化（不再是 CodePairingCard local state）
 *   - 过期前 500ms 自动续生（避免 UI 闪 "已过期"）
 *   - 被消耗后（paired-device-added 事件且本端有 activeCode）自动续生
 *
 * 后端 share code 单例设计：reject 不消耗码；本 store 仅在 accept 触发
 * paired-device-added 时续生。
 */

import { create } from "zustand";
import { commands, events, type PairingCodeInfo } from "@/lib/bindings";

const TTL_SECS = 300; // 与 SwarmNote 原 CodePairingCard 一致

interface PairingCodeState {
  codeInfo: PairingCodeInfo | null;
  generating: boolean;
  error: string | null;

  ensure(): Promise<void>;
  regenerate(): Promise<void>;
  clear(): void;
}

let autoRefreshTimer: ReturnType<typeof setTimeout> | null = null;

function clearTimer() {
  if (autoRefreshTimer !== null) {
    clearTimeout(autoRefreshTimer);
    autoRefreshTimer = null;
  }
}

function scheduleRefresh(expiresAt: string) {
  clearTimer();
  const ms = Math.max(0, new Date(expiresAt).getTime() - Date.now() - 500);
  autoRefreshTimer = setTimeout(() => {
    autoRefreshTimer = null;
    if (usePairingCodeStore.getState().codeInfo !== null) {
      void usePairingCodeStore.getState().regenerate();
    }
  }, ms);
}

function isExpired(info: PairingCodeInfo): boolean {
  return new Date(info.expiresAt).getTime() <= Date.now();
}

async function doGenerate(): Promise<void> {
  usePairingCodeStore.setState({ generating: true, error: null });
  try {
    const info = await commands.generatePairingCode(TTL_SECS);
    usePairingCodeStore.setState({
      codeInfo: info,
      generating: false,
      error: null,
    });
    scheduleRefresh(info.expiresAt);
  } catch (err) {
    clearTimer();
    usePairingCodeStore.setState({
      codeInfo: null,
      generating: false,
      error: err instanceof Error ? err.message : String(err),
    });
    console.warn("[pairing-code] generate failed:", err);
  }
}

export const usePairingCodeStore = create<PairingCodeState>()(() => ({
  codeInfo: null,
  generating: false,
  error: null,

  async ensure() {
    const { codeInfo, generating } = usePairingCodeStore.getState();
    if (generating) return;
    if (codeInfo !== null && !isExpired(codeInfo)) return;
    await doGenerate();
  },

  async regenerate() {
    if (usePairingCodeStore.getState().generating) return;
    await doGenerate();
  },

  clear() {
    clearTimer();
    usePairingCodeStore.setState({ codeInfo: null, error: null });
  },
}));

// paired-device-added 在 SwarmNote 表示"对端用我们的码完成 accept"。
// 仅在我们有 activeCode 时续生（无码 = 用户未在用配对码功能，不主动生成）。
let listenerSetup = false;
function setupListener() {
  if (listenerSetup) return;
  listenerSetup = true;
  void events.pairedDeviceAdded.listen(() => {
    if (usePairingCodeStore.getState().codeInfo !== null) {
      void doGenerate();
    }
  });
  void events.nodeStopped.listen(() => {
    usePairingCodeStore.getState().clear();
  });
}
setupListener();
