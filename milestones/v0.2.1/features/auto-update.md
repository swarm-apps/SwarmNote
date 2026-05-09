# 桌面端自动更新

## 用户故事

作为桌面端用户，我希望应用能自动检查更新并提示我安装，以便始终使用最新版本。

## 依赖

- 无依赖（L0，可独立开始）
- 参考实现：SwarmDrop（`D:\workspace\swarmdrop`）

## 需求描述

集成 `tauri-plugin-updater` 实现桌面端（Windows/macOS/Linux）自动更新。使用 UpgradeLink 作为主更新源，GitHub Releases 作为备用源。支持三种更新策略：强制更新（不可关闭）、可选更新（可稍后）、静默更新（后台自动）。

仅桌面端，Android 端推迟到 v0.5.0+。

## 交互设计

### 更新检查时机

- 应用启动后 3 秒自动检查（避免阻塞启动）
- 设置页「关于」Tab 中手动检查入口

### 强制更新 Dialog

```text
┌──────────────────────────────────────────┐
│  发现新版本                               │
│                                          │
│  SwarmNote v0.3.0 可用                   │
│  当前版本: v0.2.1                         │
│                                          │
│  更新内容:                                │
│  • E2E 内容加密                          │
│  • 权限体系                              │
│  • 重要安全修复                           │
│                                          │
│  此版本为必须更新。                        │
│                                          │
│  ████████████░░░░░░░░  60%  2.3MB/s     │
│                                          │
│              [更新中...]                  │
└──────────────────────────────────────────┘
```

- 不可关闭（无关闭按钮、无 Escape、无点击外部关闭）
- 自动开始下载，下载完成后提示重启

### 可选更新 Dialog

```text
┌──────────────────────────────────────────┐
│  发现新版本                         [✕]  │
│                                          │
│  SwarmNote v0.2.2 可用                   │
│  当前版本: v0.2.1                         │
│                                          │
│  更新内容:                                │
│  • 同步性能优化                           │
│  • Bug 修复                              │
│                                          │
│     [稍后提醒]           [立即更新]       │
└──────────────────────────────────────────┘
```

- 可关闭，「稍后提醒」关闭 Dialog
- 点击「立即更新」→ 显示下载进度 → 下载完成后提示重启

### 关于页手动检查

在设置窗口「关于」Tab 中增加「检查更新」按钮：

```text
┌──────────────────────────────────────────┐
│  SwarmNote v0.2.1                        │
│  ...                                     │
│                                          │
│  [检查更新]  ← 点击后显示 "检查中..."     │
│                  → "已是最新版本 ✓"        │
│                  → 弹出更新 Dialog        │
└──────────────────────────────────────────┘
```

## 技术方案

### 后端（Rust）

**添加依赖**：

```toml
# src-tauri/Cargo.toml
[dependencies]
tauri-plugin-updater = "2"
```

**注册插件**：

```rust
// src-tauri/src/lib.rs
app.plugin(tauri_plugin_updater::Builder::new().build())?;
```

**Tauri 配置**：

```jsonc
// src-tauri/tauri.conf.json
{
  "plugins": {
    "updater": {
      "pubkey": "<minisign 公钥>",
      "endpoints": [
        "https://api.upgrade.toolsetlink.com/v1/tauri/upgrade?tauriKey=<KEY>&versionName={{current_version}}&target={{target}}&arch={{arch}}",
        "https://github.com/swarm-apps/SwarmNote/releases/latest/download/latest.json"
      ]
    }
  }
}
```

**Capabilities**：

```jsonc
// src-tauri/capabilities/updater.json
{
  "identifier": "updater",
  "windows": ["*"],
  "permissions": ["updater:default"]
}
```

### 前端

**参考 SwarmDrop 的实现**，主要文件：

```text
src/commands/upgrade.ts              # 更新检查逻辑
src/stores/upgrade-store.ts          # Zustand 状态管理
src/components/upgrade/
├── ForceUpdateDialog.tsx            # 强制更新 Dialog
└── PromptUpdateDialog.tsx           # 可选更新 Dialog
```

**状态管理（Zustand）**：

```typescript
interface UpgradeStore {
  status: 'idle' | 'checking' | 'available' | 'force-required' | 'up-to-date' | 'downloading' | 'ready' | 'error';
  upgradeType: 'force' | 'prompt' | 'silent' | null;
  version: string | null;
  releaseNotes: string | null;
  progress: {
    downloaded: number;
    total: number;
    speed: number;       // bytes/s
    percentage: number;
  } | null;
  error: string | null;
  hasChecked: boolean;  // 防止重复检查

  checkForUpdate: () => Promise<void>;
  startDownload: () => Promise<void>;
  dismiss: () => void;
}
```

**更新检查逻辑**（参考 SwarmDrop `upgrade.ts`）：

```typescript
import { check } from '@tauri-apps/plugin-updater';

async function checkForUpdate() {
  const update = await check();
  if (!update) return null;

  // UpgradeLink 返回的 body 中包含 upgradeType
  const upgradeType = parseUpgradeType(update.body);

  return {
    version: update.version,
    releaseNotes: update.body,
    upgradeType,  // 'force' | 'prompt' | 'silent'
    update,       // 保存 update 对象用于后续下载
  };
}

async function downloadAndInstall(update: Update) {
  await update.downloadAndInstall((progress) => {
    // 更新进度
  });
  // 下载完成，提示重启
  await relaunch();
}
```

**启动时触发**（`__root.tsx`）：

```typescript
useEffect(() => {
  const timer = setTimeout(() => {
    upgradeStore.checkForUpdate();
  }, 3000);
  return () => clearTimeout(timer);
}, []);
```

### 构建与分发

- 生成 minisign 密钥对：`tauri signer generate -w ~/.tauri/swarmnote.key`
- CI 中设置环境变量 `TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- 构建产物自动签名并上传到 GitHub Releases
- 配置 UpgradeLink 关联 GitHub 仓库

## 验收标准

- [ ] 应用启动 3 秒后自动检查更新
- [ ] 有可用更新时根据 upgradeType 弹出对应 Dialog
- [ ] 强制更新 Dialog 不可关闭，自动下载并显示进度
- [ ] 可选更新 Dialog 可关闭，支持「稍后提醒」和「立即更新」
- [ ] 下载完成后提示重启并执行 relaunch
- [ ] 设置 > 关于页面有「检查更新」按钮
- [ ] 无更新时显示「已是最新版本」
- [ ] 更新失败时显示错误信息
- [ ] UpgradeLink 不可用时 fallback 到 GitHub Releases
- [ ] Windows / macOS / Linux 三平台更新正常
- [ ] `cargo clippy -- -D warnings` 无警告
- [ ] `pnpm lint:ci` 通过

## 开放问题

- UpgradeLink 的 tauriKey 需要注册获取，是否已有账号？
- minisign 密钥对的管理：存在哪里？CI secrets？
- 静默更新（upgradeType = silent）是否需要实现？还是 v0.2.1 只做强制和可选？
- 更新检查的频率：仅启动时一次？还是每 N 小时自动检查？
