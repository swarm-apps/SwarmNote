import { check, type Update } from "@tauri-apps/plugin-updater";

export type UpgradeType = "force" | "prompt" | null;

export interface UpgradeCheckResult {
  hasUpdate: boolean;
  version: string | null;
  releaseNotes: string | null;
  upgradeType: UpgradeType;
  update: Update | null;
}

/**
 * 解析 UpgradeLink 返回的 upgradeType 数字
 * 0: 不升级, 1: 提示升级, 2: 强制升级
 */
function parseUpgradeType(value: unknown): UpgradeType {
  switch (value) {
    case 2:
      return "force";
    default:
      return "prompt";
  }
}

/**
 * 检查更新（Tauri 官方 updater，端点在 tauri.conf.json 中配置）
 */
export async function checkForUpdate(): Promise<UpgradeCheckResult> {
  const update = await check({ timeout: 10000 });

  if (!update?.available) {
    return { hasUpdate: false, version: null, releaseNotes: null, upgradeType: null, update: null };
  }

  const upgradeType = parseUpgradeType((update.rawJson as Record<string, unknown>)?.upgradeType);

  return {
    hasUpdate: true,
    version: update.version,
    releaseNotes: update.body ?? null,
    upgradeType,
    update,
  };
}

/**
 * 执行桌面端更新（下载并安装，完成后自动重启）
 */
export async function executeDesktopUpdate(
  update: Update,
  onProgress?: (downloaded: number, total: number) => void,
): Promise<void> {
  let downloadedBytes = 0;
  let totalBytes = 0;

  await update.downloadAndInstall((event) => {
    if (!onProgress) return;
    switch (event.event) {
      case "Started":
        totalBytes = event.data.contentLength ?? 0;
        break;
      case "Progress":
        downloadedBytes += event.data.chunkLength;
        onProgress(downloadedBytes, totalBytes);
        break;
    }
  });
}
