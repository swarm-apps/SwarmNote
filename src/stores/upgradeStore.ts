import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import type { Update } from "@tauri-apps/plugin-updater";
import { create } from "zustand";
import { checkForUpdate, executeDesktopUpdate, type UpgradeType } from "@/lib/upgrade";

export type UpgradeStatus =
  | "idle"
  | "checking"
  | "up-to-date"
  | "available"
  | "force-required"
  | "downloading"
  | "ready"
  | "error";

interface DownloadProgress {
  downloaded: number;
  total: number;
  speed: number;
  percent: number;
}

interface UpgradeState {
  status: UpgradeStatus;
  upgradeType: UpgradeType;
  currentVersion: string | null;
  latestVersion: string | null;
  releaseNotes: string | null;
  progress: DownloadProgress | null;
  error: string | null;
  /** 防止重复检查 */
  hasChecked: boolean;

  checkForUpdate: (force?: boolean) => Promise<void>;
  startDownload: () => Promise<void>;
}

// 缓存 Update 对象，避免序列化问题
let _pendingUpdate: Update | null = null;

// 速度计算辅助变量
let _lastDownloaded = 0;
let _lastSpeedUpdate = 0;
let _currentSpeed = 0;
let _totalSize = 0;

export const useUpgradeStore = create<UpgradeState>()((set, get) => ({
  status: "idle",
  upgradeType: null,
  currentVersion: null,
  latestVersion: null,
  releaseNotes: null,
  progress: null,
  error: null,
  hasChecked: false,

  async checkForUpdate(force = false) {
    const { status, hasChecked } = get();
    if (!force && hasChecked) return;
    if (status === "checking" || status === "downloading") return;

    set({ status: "checking", error: null });

    try {
      const currentVersion = await getVersion();
      set({ currentVersion });

      const result = await checkForUpdate();

      if (!result.hasUpdate) {
        set({ status: "up-to-date", hasChecked: true });
        return;
      }

      _pendingUpdate = result.update;

      set({
        status: result.upgradeType === "force" ? "force-required" : "available",
        latestVersion: result.version,
        upgradeType: result.upgradeType,
        releaseNotes: result.releaseNotes,
        hasChecked: true,
      });
    } catch (err) {
      console.error("[upgrade] check failed:", err);
      set({
        status: "error",
        error: err instanceof Error ? err.message : String(err),
        hasChecked: true,
      });
    }
  },

  async startDownload() {
    const { status } = get();
    if (status !== "available" && status !== "force-required") return;
    if (!_pendingUpdate) {
      set({ status: "error", error: "No pending update" });
      return;
    }

    set({
      status: "downloading",
      progress: { downloaded: 0, total: 0, speed: 0, percent: 0 },
    });

    _lastDownloaded = 0;
    _lastSpeedUpdate = Date.now();
    _currentSpeed = 0;
    _totalSize = 0;

    try {
      await executeDesktopUpdate(_pendingUpdate, (downloaded, total) => {
        if (total > 0) _totalSize = total;

        const now = Date.now();
        if (now - _lastSpeedUpdate > 500) {
          const elapsed = (now - _lastSpeedUpdate) / 1000;
          _currentSpeed = elapsed > 0 ? (downloaded - _lastDownloaded) / elapsed : 0;
          _lastDownloaded = downloaded;
          _lastSpeedUpdate = now;
        }

        const percent = _totalSize > 0 ? Math.round((downloaded / _totalSize) * 100) : 0;
        set({
          progress: { downloaded, total: _totalSize, speed: _currentSpeed, percent },
        });
      });

      set({ status: "ready" });
      await relaunch();
    } catch (err) {
      console.error("[upgrade] download failed:", err);
      set({
        status: "error",
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },
}));
