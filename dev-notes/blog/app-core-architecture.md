# App Core 架构设计：跨平台 Rust 核心抽象

> SwarmNote 桌面端 (Tauri) 和移动端 (Expo RN) 共享一个平台无关的 Rust 核心库 `app-core`，通过 trait 注入平台差异（文件系统、事件通知、密钥存储）。

## 目录

1. [动机与约束](#1-动机与约束)
2. [分层架构总览](#2-分层架构总览)
3. [Platform Traits 设计](#3-platform-traits-设计)
   - 3.1 [FileSystem trait](#31-filesystem-trait)
   - 3.2 [FileWatcher trait（仅桌面端）](#32-filewatcher-trait仅桌面端)
   - 3.3 [EventBus trait](#33-eventbus-trait)
   - 3.4 [KeychainProvider trait](#34-keychainprovider-trait)
4. [app-core crate 结构](#4-app-core-crate-结构)
5. [YDocManager 迁移设计（CM6 后）](#5-ydocmanager-迁移设计cm6-后)
6. [各端集成方式](#6-各端集成方式)
   - 6.1 [桌面端 (Tauri)](#61-桌面端-tauri)
   - 6.2 [移动端 (uniffi + Expo)](#62-移动端-uniffi--expo)
7. [文件系统策略：混合模式](#7-文件系统策略混合模式)
8. [双端差异矩阵](#8-双端差异矩阵)
9. [实施路径](#9-实施路径)

---

## 1. 动机与约束

### 现状问题

桌面端 `src-tauri/src/` 中的 Rust 后端与 Tauri 深度耦合：

- `YDocManager` 依赖 `tauri::State<DbState>` 和 `tokio::fs`
- `SyncManager` 通过 `app.emit()` 发送事件
- `identity` 模块绑定桌面端 `keyring` crate
- 文件监听 (`notify`) 是桌面端特有的需求

移动端的 `mobile-core` 目前只有一个 `greet()` 演示函数，无法复用桌面端任何业务逻辑。

### 设计约束

1. **CM6 迁移后**：Y.Doc schema 从 XML 树变为 Y.Text，yrs-blocknote crate (~5000 行) 将被删除
2. **移动端 MVP 仅沙盒**：笔记只存放在 app documentDirectory 内，Android/iOS 沙盒 Rust `std::fs` 直接可用，无需权限申请。外部目录（Android SAF / iOS File Provider）作为 v2 迭代
3. **无需兼容老版本 / 无数据迁移**：BlockNote 时代的 Y.Doc(XML) 数据不做迁移——CM6 切换是硬升级，直接替换
4. **移动端无外部编辑器**：App 沙盒内文件不会被外部修改，不需要 FileWatcher
5. **swarm-p2p-core 已独立**：P2P 网络层零 Tauri 依赖，可直接复用

---

## 2. 分层架构总览

```
┌─────────────────────────────────────────────────┐
│                   app-core crate                │
│  (纯业务逻辑，零平台依赖)                         │
│                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│  │ YDocMgr  │  │ SyncMgr  │  │ Workspace│      │
│  │ (Y.Text) │  │          │  │ Manager  │      │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘      │
│       │              │              │            │
│  ┌────┴──────────────┴──────────────┴────┐      │
│  │         Platform Traits                │      │
│  │  FileSystem / EventBus / Keychain      │      │
│  └────────────────────────────────────────┘      │
├─────────────────────────────────────────────────┤
│  swarm-p2p-core    │   yrs   │   sea-orm        │
│  (已独立)           │         │   (SQLite)       │
└─────────────────────────────────────────────────┘

        ▲                           ▲
        │ impl traits               │ impl traits
   ┌────┴────────┐           ┌──────┴───────┐
   │ desktop-impl│           │ mobile-impl  │
   │ (Tauri)     │           │ (uniffi)     │
   │ tokio::fs   │           │ std::fs      │
   │ notify      │           │ (sandbox)    │
   │ tauri::emit │           │ uniffi CB    │
   └─────────────┘           └──────────────┘
```

**核心原则**：`app-core` 不知道自己运行在哪个平台。所有平台差异通过泛型参数 / trait object 注入。

---

## 3. Platform Traits 设计

### 3.1 FileSystem trait

所有路径都是相对于 workspace root 的相对路径。

```rust
/// 平台文件系统抽象
/// 所有 rel_path 都是相对于 workspace root 的路径
#[async_trait]
pub trait FileSystem: Send + Sync + 'static {
    // --- 文本文件 ---
    async fn read_text(&self, rel_path: &str) -> Result<String>;
    async fn write_text(&self, rel_path: &str, content: &str) -> Result<()>;

    // --- 二进制文件 ---
    async fn read_bytes(&self, rel_path: &str) -> Result<Vec<u8>>;
    async fn write_bytes(&self, rel_path: &str, data: &[u8]) -> Result<()>;

    // --- 元信息 ---
    async fn exists(&self, rel_path: &str) -> bool;

    // --- 文件操作 ---
    async fn remove_file(&self, rel_path: &str) -> Result<()>;
    async fn rename(&self, from: &str, to: &str) -> Result<()>;

    // --- 目录操作 ---
    async fn create_dir(&self, rel_path: &str) -> Result<()>;
    async fn remove_dir(&self, rel_path: &str) -> Result<()>;
    async fn list_dir(&self, rel_path: &str) -> Result<Vec<DirEntry>>;
    async fn scan_tree(&self, rel_path: &str) -> Result<Vec<FileNode>>;

    // --- 媒体（内容寻址存储）---
    /// 保存媒体文件到 `{note_rel_path}.assets/{stem}-{blake3_short}.{ext}`
    /// 相同内容自动去重，返回最终相对路径
    async fn save_media(
        &self,
        note_rel_path: &str,
        file_name: &str,
        data: &[u8],
    ) -> Result<String>;
}
```

**关键设计决策（MVP 阶段）**：双端 FileSystem 实现代码几乎一样（都是 `tokio::fs` / `std::fs`），差异只在 root 路径来源。因此提供一个通用实现 `LocalFs`，桌面端和移动端（沙盒）共用。

> **面向未来的扩展位**：移动端 v2 要支持外部目录（Android SAF / iOS File Provider）时，Rust `std::fs` 无法直接访问——必须通过 `ContentResolver` 或 Document Picker。届时会增加 `SafFs` / `FileProviderFs` 等实现，它们通过 JS 回调访问文件内容。**`FileSystem` 作为 trait 存在的价值正在于此——MVP 只有 LocalFs 一个实现，但插槽留好了。**

```rust
/// 通用本地文件系统 — 双端共享
pub struct LocalFs {
    root: PathBuf,
}

impl LocalFs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[async_trait]
impl FileSystem for LocalFs {
    async fn read_text(&self, rel_path: &str) -> Result<String> {
        let full = self.root.join(rel_path);
        tokio::fs::read_to_string(full).await.map_err(Into::into)
    }

    async fn write_text(&self, rel_path: &str, content: &str) -> Result<()> {
        let full = self.root.join(rel_path);
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(full, content).await.map_err(Into::into)
    }

    // ... 其他方法同理
}
```

桌面端和移动端构造时只是传不同的 root：

```rust
// 桌面端：用户选择的工作区路径
let fs = LocalFs::new("/Users/alice/Documents/my-notes");

// 移动端：expo documentDirectory
let fs = LocalFs::new("/data/data/com.swarmnote/files/swarmnote");
```

### 3.2 FileWatcher trait（仅桌面端）

文件监听是桌面端独有需求——移动端 app 沙盒内没有"外部编辑器修改 .md 文件"的场景。

```rust
/// 文件变更监听（可选，仅桌面端实现）
#[async_trait]
pub trait FileWatcher: Send + Sync + 'static {
    async fn watch(
        &self,
        callback: Arc<dyn Fn(Vec<FileEvent>) + Send + Sync>,
    ) -> Result<()>;
    async fn unwatch(&self) -> Result<()>;
}

pub enum FileEvent {
    Created(String),   // rel_path
    Modified(String),
    Deleted(String),
    Renamed { from: String, to: String },
}
```

`app-core` 中 FileWatcher 是 `Option` 依赖——有就用，没有就跳过外部重载逻辑。

### 3.3 EventBus trait

替代 `tauri::AppHandle::emit()`，让 core 层能向上层通知事件。

```rust
/// 平台事件总线
pub trait EventBus: Send + Sync + 'static {
    fn emit(&self, event: AppEvent);
}

pub enum AppEvent {
    /// Y.Doc 已持久化到磁盘
    DocFlushed { doc_id: Uuid },
    /// 收到远端 P2P 同步 update（需要通知前端刷新）
    RemoteUpdate { doc_id: Uuid, update: Vec<u8> },
    /// 外部文件变更（桌面端独有）
    ExternalFileChange { rel_path: String, kind: FileEvent },
    /// 外部文件冲突（用户有未保存编辑 + 文件被外部修改）
    ExternalConflict { doc_id: Uuid, rel_path: String },
    /// P2P 同步状态变更
    SyncStateChanged { peer_id: String, state: SyncState },
    /// 设备上线/下线
    PeerStatusChanged { peer_id: String, online: bool },
}
```

双端实现：

| 平台 | 实现 |
|------|------|
| 桌面端 | `TauriEventBus` → `app.emit("event-name", payload)` |
| 移动端 | `UniffiEventBus` → uniffi callback interface → JS 回调 |

### 3.4 KeychainProvider trait

设备身份 (Ed25519 keypair) 的存储方式因平台而异。

```rust
/// 密钥存储抽象
#[async_trait]
pub trait KeychainProvider: Send + Sync + 'static {
    /// 获取或创建设备密钥对
    async fn get_or_create_keypair(&self) -> Result<Ed25519Keypair>;
    /// 获取设备名称
    async fn get_device_name(&self) -> Result<String>;
    /// 设置设备名称
    async fn set_device_name(&self, name: &str) -> Result<()>;
}
```

| 平台 | 实现 |
|------|------|
| 桌面端 | `keyring` crate (系统钥匙串) |
| 移动端 | Android Keystore / iOS Keychain (通过 uniffi 或 expo-secure-store) |

---

## 4. app-core crate 结构

```
libs/app-core/
├── Cargo.toml
└── src/
    ├── lib.rs                  # Core<F, E, K> 主入口
    │
    ├── traits/
    │   ├── mod.rs
    │   ├── filesystem.rs       # FileSystem trait + LocalFs 通用实现
    │   ├── file_watcher.rs     # FileWatcher trait (可选)
    │   ├── event_bus.rs        # EventBus trait + AppEvent enum
    │   └── keychain.rs         # KeychainProvider trait
    │
    ├── yjs/
    │   ├── mod.rs
    │   ├── manager.rs          # YDocManager (持有 Arc<dyn FileSystem/EventBus>)
    │   └── doc_entry.rs        # DocEntry (防抖写回、blake3 hash)
    │
    ├── workspace/
    │   ├── mod.rs
    │   └── manager.rs          # WorkspaceManager (持有 Arc<dyn FileSystem>)
    │
    ├── sync/
    │   ├── mod.rs
    │   ├── manager.rs          # SyncManager (从 src-tauri 抽出)
    │   ├── doc_sync.rs         # 单文档状态向量交换
    │   ├── full_sync.rs        # 全量文档拉取
    │   └── asset_sync.rs       # 资源文件分块传输
    │
    ├── document/
    │   ├── mod.rs
    │   └── crud.rs             # 文档 DB CRUD (sea-orm)
    │
    ├── identity/
    │   ├── mod.rs
    │   └── manager.rs          # IdentityManager (持有 Arc<dyn KeychainProvider>)
    │
    ├── protocol/
    │   └── mod.rs              # AppRequest / AppResponse (从 src-tauri 搬入)
    │
    ├── model.rs                # 共享数据结构 (DirEntry, FileNode, etc.)
    └── error.rs                # AppError 统一错误类型
```

**Cargo.toml 依赖**（零平台绑定）：

```toml
[dependencies]
swarm-p2p-core = { path = "../core" }
yrs = { version = "0.25", features = ["sync"] }
sea-orm = "~2.0.0-rc"
tokio = { version = "1", features = ["sync", "time", "macros", "fs"] }
blake3 = "1"
dashmap = "6"
serde = { version = "1", features = ["derive"] }
uuid = { version = "1", features = ["v4", "serde"] }
async-trait = "0.1"
thiserror = "2"
tracing = "0.1"
```

---

## 5. YDocManager 迁移设计（CM6 后）

CM6 迁移后，Y.Doc schema 从 XML 树退化为单个 Y.Text。YDocManager 大幅简化：

```rust
pub struct YDocManager {
    docs: DashMap<Uuid, Arc<DocEntry>>,
    fs: Arc<dyn FileSystem>,
    event_bus: Arc<dyn EventBus>,
    db: DatabaseConnection,
}

impl YDocManager {
    /// 打开文档 — Y.Text 模型
    pub async fn open(&self, rel_path: &str) -> Result<OpenedDoc> {
        let doc = Doc::new();

        // 优先从 DB 加载 yjs state
        if let Some(state) = self.load_from_db(rel_path).await? {
            doc.transact_mut()
                .apply_update(Update::decode_v1(&state)?);
        }
        // fallback: 从 .md 文件加载
        else if self.fs.exists(rel_path).await {
            let md = self.fs.read_text(rel_path).await?;
            let mut txn = doc.transact_mut();
            let text = txn.get_or_insert_text("document");
            text.insert(&mut txn, 0, &md);
        }
        // else: 空文档

        // ...创建 DocEntry, 启动防抖写回任务
    }

    /// 写回 .md — 一行代码替代 5000 行转换
    async fn writeback(&self, entry: &DocEntry) -> Result<()> {
        let txn = entry.doc.transact();
        let text = txn.get_text("document").unwrap();
        let md = text.get_string(&txn);

        // 写文件
        self.fs.write_text(&entry.rel_path, &md).await?;

        // blake3 hash (自写检测)
        let hash = blake3::hash(md.as_bytes());
        entry.set_file_hash(hash.as_bytes().to_vec());

        // 持久化 yjs state 到 DB
        let state = txn.encode_state_as_update_v1(&StateVector::default());
        self.persist_to_db(&entry.doc_id, &state).await?;

        self.event_bus.emit(AppEvent::DocFlushed {
            doc_id: entry.doc_id,
        });

        Ok(())
    }
}
```

**对比**：

| | 迁移前 | 迁移后 |
|---|---|---|
| 写回一个文档 | XML 解码 → Block 树 → mdast AST → Handler → 转义 → Markdown | `text.get_string()` |
| 依赖 crate | yrs-blocknote + mdast-util-to-markdown (~5000 行) | 无（yrs 原生 API） |
| Y.Doc fragment name | `"document-store"` (BlockNote 约定) | `"document"` |

### 为什么用 `Arc<dyn>` 而不是泛型参数

YDocManager 这里选 trait object 而非 `YDocManager<F: FileSystem, E: EventBus>`，原因：

| 维度 | 泛型 `<F, E>` | Trait object `Arc<dyn>` |
| ---- | ------------- | ----------------------- |
| 性能 | 编译期单态化，零开销 | 动态分发（vtable 查表） |
| 调用方签名 | `State<YDocManager<F, E>>` 泛型参数向上传染（Tauri command、SyncManager、event_loop 全部要跟着改） | `State<YDocManager>`，签名干净 |
| 二进制体积 | 每种具体组合生成一份代码 | 一份代码 |
| MVP 实现数 | 只有一种（LocalFs + 具体 EventBus） | 同 |
| 未来扩展 | 每加一种实现多一次单态化 | 零成本 |

**判断依据**：YDocManager 的热点操作是 `fs.write_text()` / `event_bus.emit()`，前者是毫秒级的磁盘 I/O，后者是跨 IPC 消息发送——vtable 查表的几十纳秒开销对比之下完全不可见。泛型化的**唯一收益是"理论上零开销"**，但在 I/O 密集路径上这个收益不成立；而签名传染的成本是真实的可读性/维护性负担。

同样的判断也适用于 `WorkspaceManager`、`SyncManager`、`IdentityManager` 等 core 层类型——**全部持有 `Arc<dyn>`**，而非泛型参数。

---

## 6. 各端集成方式

### 6.1 桌面端 (Tauri)

```rust
// src-tauri/src/platform/mod.rs

use app_core::traits::*;

/// 桌面端文件系统 — 复用 LocalFs + 额外的 FileWatcher
pub struct DesktopPlatform {
    pub fs: LocalFs,
    pub watcher: NotifyFileWatcher,  // notify crate
    pub event_bus: TauriEventBus,
    pub keychain: DesktopKeychain,   // keyring crate
}

pub struct TauriEventBus {
    app: tauri::AppHandle,
}

impl EventBus for TauriEventBus {
    fn emit(&self, event: AppEvent) {
        match &event {
            AppEvent::DocFlushed { doc_id } => {
                self.app.emit("yjs:flushed", doc_id).ok();
            }
            AppEvent::RemoteUpdate { doc_id, update } => {
                self.app.emit("yjs:remote-update", (doc_id, update)).ok();
            }
            // ...
        }
    }
}

// Tauri setup
fn setup(app: &mut tauri::App) {
    let ws_path = /* 用户选择的工作区路径 */;
    let core = app_core::Core::new(
        Arc::new(LocalFs::new(ws_path)),
        Arc::new(TauriEventBus { app: app.handle().clone() }),
        Arc::new(DesktopKeychain),
    );
    app.manage(core);
}
```

### 6.2 移动端 (uniffi + Expo)

**JS 侧**（React Native）：

```typescript
import * as FileSystem from 'expo-file-system';

// 初始化时把沙盒路径传给 Rust
const dataDir = FileSystem.documentDirectory + 'swarmnote/';
await FileSystem.makeDirectoryAsync(dataDir, { intermediates: true });

const core = createCore(dataDir, {
    // uniffi callback interface — 接收 Rust 事件
    onEvent(event: AppEvent) {
        switch (event.type) {
            case 'remote_update':
                editorApi.applyYjsUpdate(event.update);
                break;
            case 'doc_flushed':
                // 更新 UI 状态
                break;
        }
    },
});
```

**Rust 侧**（uniffi 导出）：

```rust
// packages/swarmnote-core/rust/mobile-core/src/lib.rs

#[uniffi::export(callback_interface)]
pub trait AppEventListener: Send + Sync {
    fn on_event(&self, event: AppEvent);
}

struct UniffiEventBus {
    listener: Arc<dyn AppEventListener>,
}

impl EventBus for UniffiEventBus {
    fn emit(&self, event: AppEvent) {
        self.listener.on_event(event);
    }
}

#[uniffi::export]
pub fn create_core(
    data_dir: String,
    listener: Arc<dyn AppEventListener>,
) -> Arc<MobileCore> {
    let core = app_core::Core::new(
        Arc::new(LocalFs::new(data_dir)),        // MVP：沙盒 only；v2 可替换为 SafFs / FileProviderFs
        Arc::new(UniffiEventBus { listener }),
        Arc::new(MobileKeychain::new()),         // 或通过 expo-secure-store callback
    );
    Arc::new(MobileCore { inner: core })
}
```

---

## 7. 文件系统策略：混合模式

移动端采用**混合模式**——Rust 直接操作沙盒文件系统，Expo FS 处理用户交互。

```
┌─────────────────────────────────────────┐
│            JS / React Native            │
│                                         │
│  expo-file-system                       │
│  ├── documentDirectory → 传给 Rust      │
│  ├── 导入/导出/分享 (用户触发, 低频)      │
│  └── SAF picker (Android 外部存储)       │
│                                         │
│  uniffi init:                           │
│  create_core(documentDirectory, ...)    │
│         │                               │
└─────────┼───────────────────────────────┘
          │ 只传一次路径
          ▼
┌─────────────────────────────────────────┐
│         Rust (app-core)                 │
│                                         │
│  LocalFs { root: documentDirectory }    │
│                                         │
│  所有核心 I/O 在 Rust 内完成:            │
│  ├── YDocManager writeback (高频)       │
│  ├── P2P sync → 写 .md (高频)          │
│  ├── 打开文档 → 读 .md (中频)           │
│  └── 保存媒体 → 写 assets (低频)        │
│                                         │
│  无需跨 bridge，无需权限申请             │
└─────────────────────────────────────────┘
```

**为什么不全走 expo-file-system**：

| I/O 场景 | 频率 | 走 Expo FS | 走 Rust std::fs |
|----------|------|-----------|-----------------|
| writeback .md (防抖 1.5s) | 高 | 每次跨 bridge，延迟大 | 直接写，零开销 |
| P2P sync → 写文件 | 高 | 同上 | 同上 |
| 打开文档读 .md | 中 | 可以但没必要 | 直接读 |
| 用户导入文件 | 低 | 需要 SAF/picker | 不适用 |
| 用户导出/分享 | 低 | 需要 Intent/Share | 不适用 |

**结论**：Rust 高频 I/O 直接用 `std::fs`，JS 低频用户操作用 `expo-file-system`。

---

## 8. 双端差异矩阵

| 维度 | 桌面端 (Tauri) | 移动端 (Expo RN) |
|------|---------------|-----------------|
| **FS root 来源** | 用户通过 dialog 选择任意路径 | `expo-file-system` documentDirectory |
| **多工作区** | 每个工作区独立目录 | 同一沙盒下子目录 |
| **FileWatcher** | `notify` crate (检测外部编辑) | 不需要（沙盒隔离） |
| **外部可访问** | 是 (VS Code/Obsidian 可编辑) | 否 |
| **EventBus** | `tauri::AppHandle::emit()` | uniffi callback interface |
| **Keychain** | `keyring` crate (系统钥匙串) | Android Keystore / iOS Keychain |
| **编辑器加载** | 直接 import `@swarmnote/editor` | WebView bundle + Comlink RPC |
| **yjs update 通道** | Tauri IPC (invoke/listen) | uniffi 函数调用 + callback |
| **数据库** | SQLite (sea-orm) | 同（路径在沙盒内） |
| **P2P 网络** | swarm-p2p-core (QUIC + TCP) | 同（可能限制部分传输） |
| **FileSystem impl (MVP)** | `LocalFs::new(user_chosen_path)` | `LocalFs::new(document_directory)` |
| **FileSystem impl (v2)** | 同 MVP | `SafFs` (Android) / `FileProviderFs` (iOS) |

---

## 9. 实施路径

实施顺序采用「**先简化，后拆分，再扩展**」：先让 core 层变轻（CM6 迁移），再把它搬出去（app-core 抽取），最后给它加一个新平台（移动端）。每一步的变化面都尽量局部化，避免"大爆炸式"重构。

```text
Phase 1                    Phase 2                    Phase 3
先简化                      后拆分                      再扩展
┌────────────────┐         ┌────────────────┐         ┌────────────────┐
│ 桌面端 CM6     │    →    │ 抽 app-core    │    →    │ 移动端接入     │
│ + YDocManager  │         │ crate          │         │ (uniffi + Expo)│
│ 重写 Y.Text    │         │ + 定义 traits  │         │                │
│                │         │ + LocalFs impl │         │                │
│ 直接替换       │         │                │         │ MVP 沙盒 only  │
│ (不留 BlockNote)│        │ 纯搬家         │         │                │
│ (无数据迁移)   │         │ 零功能变化     │         │                │
└────────────────┘         └────────────────┘         └────────────────┘
   前端+后端                架构搬家                   新平台落地
   中等变动                 中等变动                   较大变动
```

### Phase 1 — 桌面端 CM6 迁移 + YDocManager 重写

> 目标：让 Rust 后端的 core 层变得小到值得跨平台抽取。

1. 桌面端前端从 BlockNote 切换到 `@swarmnote/editor` (CM6)，删除 `@blocknote/*` 系列依赖
2. YDocManager 改用 Y.Text 模型，fragment name `"document-store"` → `"document"`
3. 删除 `yrs-blocknote` + `mdast-util-to-markdown` crate (~5000 行)
4. **硬升级**：不兼容旧版 BlockNote Y.Doc 数据，不做数据迁移——版本滚动由发布策略保证（同设备组一起升）
5. 适配周边组件：命令面板、大纲、快捷键、格式化工具栏
6. 回归测试：P2P sync、外部 .md 修改、媒体上传

### Phase 2 — 抽 app-core crate + 定义 traits

> 目标：把已经变轻的 core 层从 `src-tauri/` 搬到独立 crate，为移动端复用做准备。桌面端零功能变化。

1. 在 `libs/` 下创建 `app-core` crate
2. 定义 Platform Traits：`FileSystem`、`FileWatcher`（可选）、`EventBus`、`KeychainProvider`
3. 实现 `LocalFs`（通用本地文件系统，桌面 / 移动沙盒共用）
4. 把 `protocol/`、`error.rs`、`model`、`YDocManager`、`SyncManager`、`WorkspaceManager`、`DocumentCrud`、`IdentityManager` 从 `src-tauri` 搬入
5. core 层内部统一使用 `Arc<dyn FileSystem>` / `Arc<dyn EventBus>` / `Arc<dyn KeychainProvider>`（trait object，避免泛型参数向上传染）
6. 桌面端保留 Tauri 相关的 `TauriEventBus`、`NotifyFileWatcher`、`DesktopKeychain` 实现
7. 桌面端所有命令重新走 `app-core` 注入的依赖，回归验证与 Phase 1 功能一致

### Phase 3 — 移动端接入 app-core（MVP 沙盒 only）

> 目标：移动端首发版本，笔记仅存放在 app documentDirectory。外部目录（Android SAF / iOS File Provider）作为后续迭代。

1. `mobile-core` 依赖 `app-core`，实现 `UniffiEventBus` + `MobileKeychain`
2. 移动端 `FileSystem` 直接复用 `LocalFs`（沙盒路径下 `std::fs` 即可）
3. uniffi 导出 `create_core()`、文档操作、同步控制等 API
4. JS 侧初始化：`expo-file-system` documentDirectory → Rust
5. 编辑器接入 yjs update 流：WebView ↔ Comlink ↔ RN ↔ uniffi ↔ Rust ↔ P2P
6. 端到端验证：移动端编辑 → Rust writeback → P2P sync → 桌面端更新

### 后续迭代（不在首发范围）

- **v2 — 移动端外部目录**：新增 `SafFs`（Android ContentResolver）/ `FileProviderFs`（iOS Document Picker），允许用户把笔记放到 Obsidian 目录或云盘同步目录
- **v2 — 结构化检索**：按需用 `@lezer/markdown` 解析 AST，支持"所有未完成待办"这类结构化查询
