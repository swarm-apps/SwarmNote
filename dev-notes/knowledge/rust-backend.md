# Rust 后端

## 架构概览

Cargo workspace 根在仓库根 `Cargo.toml`，成员：

- `crates/core` — `swarmnote-core`（跨平台业务层，零 Tauri 依赖）
- `crates/entity`、`crates/migration` — SeaORM entity + 迁移（独立 crate，两端共用）
- `src-tauri` — 桌面端 Tauri 壳（commands / platform impls / tray）
- `libs/core`、`libs/bootstrap` — submodule 引入的 `swarm-p2p-core` + bootstrap 二进制

进行中的 change `extract-swarmnote-core`：把业务层从 `src-tauri/src/` 逐步搬到 `crates/core/`，桌面壳只留 IPC + 平台 impl。Phase 1 PR 已落地 identity + config + fs traits + 事件骨架。

### `crates/core/` 模块（平台无关，全部业务逻辑）

| 模块 | 职责 |
| ---- | ---- |
| `app.rs` | `AppCore` 设备级单例（identity + config + event_bus + keychain + devices_db + net + sync_coordinator + workspaces 注册表） |
| `workspace/mod.rs` + `workspace/db.rs` | `WorkspaceCore`（per-workspace 资源容器）+ `WorkspaceInfo` DTO + DB 初始化 |
| `workspace/sync/coordinator.rs` | `AppSyncCoordinator`：全局 full-sync 去重、SV 补偿、ctrl topic 路由、入站请求分发 |
| `workspace/sync/workspace_sync.rs` | `WorkspaceSync`：per-workspace GossipSub 订阅/发布、pending buffer、asset check |
| `workspace/sync/{doc_sync,full_sync,asset_sync,pending_buffer}.rs` | 同步子模块（自由函数，接收 `&Arc<AppCore>` 或 workspace 参数） |
| `device/` | `DeviceManager`、`Device` DTO、连接类型推断 |
| `pairing/` | `PairingManager`、配对码、DHT 发布/查找 |
| `network/` | `NetManager`（P2P 会话包）、`AppNetClient` 类型别名、事件循环、DHT 在线宣告、节点配置 |
| `identity.rs` | `IdentityManager`、`DeviceInfo`、keypair protobuf 编码 |
| `config.rs` | `GlobalConfig`、`GlobalConfigState`、持久化 |
| `fs/mod.rs` + `fs/ops.rs` | `FileSystem` trait、`LocalFs`、`FileWatcher` trait、业务层操作（auto-numbering / sidecar / move） |
| `events.rs` | `EventBus` trait、`AppEvent` enum（14 个变体） |
| `keychain.rs` | `KeychainProvider` trait |
| `protocol/` | P2P 协议定义（AppRequest/AppResponse + os_info/pairing/sync/workspace 子协议） |
| `yjs/mod.rs` + `yjs/{manager,doc_state}.rs` | YDocManager（per-workspace Y.Doc 生命周期 + Notify-driven writeback）、hydrate、closed-doc merge |
| `document.rs` | `DocumentCrud`（per-workspace 文档/文件夹 CRUD） |
| `error.rs` | `AppError` / `AppResult` |

### `src-tauri/src/` 模块（桌面壳，薄封装）

| 模块 | 职责 |
| ---- | ---- |
| `commands/` | 8 个子模块（identity / workspace / document / fs / yjs / network / pairing / sync），每个是 `#[tauri::command]` 薄 wrapper → `Arc<AppCore>` / `WorkspaceMap` |
| `platform/` | `TauriEventBus`、`DesktopKeychain`、`NotifyFileWatcher`、`WorkspaceMap`（实现 core trait） |
| `tray.rs` | 系统托盘（桌面端 only） |
| `error.rs` | re-export `swarmnote_core::{AppError, AppResult}` |
| `lib.rs` | Tauri Builder setup、`generate_handler![]`、startup window dispatch |

## Rust 模块组织规范

SwarmNote 遵循 Rust 2018+ 社区惯例（对照 tokio / reqwest / sea-orm 等成熟 crate）：

### 单文件用 `foo.rs` 平铺，多文件用 `foo.rs + foo/bar.rs`

```text
✗ 避免                          ✓ 推荐
─────────                      ──────────
identity/mod.rs                identity.rs
config/mod.rs                  config.rs

protocol/mod.rs   (500 行)     protocol.rs      (薄顶层)
                               protocol/
                                 ├── os_info.rs
                                 ├── pairing.rs
                                 └── sync.rs
```

- **不要**：为单文件模块创建 `foo/mod.rs` 目录（编辑器 tab 一堆 `mod.rs` 混乱）
- **推荐**：`foo/mod.rs` 模式只在 Rust 2015 edition 合法且目录多文件时用
- **拆分阈值**：单模块超过 300 行考虑拆子模块；`protocol.rs` 按子协议拆是典型例子

### 按领域组织，不按机制

```text
✗ 机制导向（Java/OOP 风）      ✓ 领域导向（std / tokio 风）
─────────────────────────     ──────────────────────────
traits/                        fs.rs         (FileSystem + LocalFs + FileTreeNode)
  ├── filesystem.rs            events.rs     (EventBus + AppEvent)
  ├── event_bus.rs             keychain.rs   (KeychainProvider)
  ├── keychain.rs
  └── file_watcher.rs

model.rs                       identity.rs   (DeviceInfo 归 identity)
  ├── DeviceInfo               workspace.rs  (WorkspaceInfo 归 workspace)
  └── WorkspaceInfo
```

- `std::io::Read` 不在 `std::traits::Read`；`tokio::io::AsyncRead` 不在 `tokio::traits::*`
- DTO 归所属领域模块，不要集中在 `model.rs` / `types.rs`（Django/Rails 风在 Rust 不常见）
- trait + 它的 DTO 放同文件：`fs.rs` 里同时定义 `FileSystem` + `FileTreeNode` + `FileEvent`

### `lib.rs` 顶层 flat re-export 面向消费者

```rust
// crates/core/src/lib.rs
pub mod app;
pub mod fs;
pub mod events;
// ...

// 消费者高频 API 顶层扁平 re-export
pub use app::AppCore;
pub use error::{AppError, AppResult};
pub use events::{AppEvent, EventBus, NetworkStatus};
pub use fs::{FileSystem, LocalFs, FileWatcher, FileTreeNode, FileEvent};
pub use identity::{DeviceInfo, IdentityManager};
pub use keychain::KeychainProvider;
```

host 用 `use swarmnote_core::{AppCore, FileSystem, EventBus};` 而不是 `swarmnote_core::traits::FileSystem`。对照 `tokio::spawn` 和 `reqwest::Client` 这样的扁平入口。

### 跨 crate 迁移时的 nominal type 去重

把模块从 `src-tauri/src/foo/` 搬到 `crates/core/src/foo.rs` 时，**不要保留两份**——Rust 视为不同 nominal type，会在后续 PR 编译炸锅。正确做法：

```rust
// src-tauri/src/protocol/mod.rs（shim）
pub use swarmnote_core::protocol::*;
```

把旧位置改成薄 re-export shim，所有 `use crate::protocol::X` 调用点零改动继续工作。`DeviceInfo` / `GlobalConfig` / DB helpers 同理。

### `mod.rs` 里不要堆逻辑

`mod.rs` / `foo.rs`（作为模块入口）应该只含：`pub mod`、`pub use`、少量顶层声明（`pub const`）。具体实现放子模块。例子：`protocol.rs` 只 20 行声明 + re-export，所有 struct 在子模块。

## Tauri command 约定

### 使用 `#[tauri::command]` + `AppResult<T>`

```rust
#[tauri::command]
pub async fn my_command(
    state: State<'_, AppState>,
    arg: String,
) -> AppResult<MyResponse> {
    // ...
    Ok(response)
}
```

- 返回类型统一 `AppResult<T>` = `Result<T, AppError>`
- `AppError` 序列化为 `{ kind, message }` JSON 给前端消费
- 参数使用 snake_case，前端 `invoke()` 传参自动 camelCase → snake_case 转换
- `State<'_, T>` 注入共享状态，`AppHandle` 注入应用句柄

**相关文件**：`src-tauri/src/error.rs`、`src-tauri/src/lib.rs`（`generate_handler![]` 注册）

### Capability 声明

所有 command 必须在 `src-tauri/capabilities/*.json` 中 allow 才能被前端调用。Tauri v2 的安全模型，默认拒绝。

**相关文件**：`src-tauri/capabilities/`

### Rust lib 命名避免冲突

Rust lib 名称是 **`swarmnote_lib`** 而不是 `swarmnote`。Windows 下 lib 和 bin 同名会冲突，所以必须加 `_lib` 后缀。

**相关文件**：`src-tauri/Cargo.toml`（`[lib] name`）

## 双数据库

### devices.db (全局) + workspace.db (per-workspace)

- **devices.db**：app data 目录，存配对设备
- **workspace.db**：每个工作区根目录 `.swarmnote/`，存文档/文件夹/工作区元数据

`DbState` 通过 `RwLock<HashMap<String, DatabaseConnection>>` 管理多窗口。每个窗口绑定一个 workspace DB 连接。

**正确做法**：不要假设全局唯一 DB 句柄，用 window label 或 workspace id 做 key 查询。

**相关文件**：`src-tauri/src/workspace/state.rs`

### SeaORM + Uuid v7 主键

所有主键和外键统一 `Uuid`（v7）。ORM 版本 `sea-orm 2.0-rc`。迁移脚本在 `src-tauri/migration/`。

使用规范见 `sea-orm-2` skill。

## 日志

使用 **tracing**，不用 `log`。

```rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(self))]
async fn my_fn(&self, arg: String) -> AppResult<()> {
    info!(%arg, "starting");
    // ...
}
```

**不要做**：`log::info!` / `println!` 在生产代码。

**相关文件**：`src-tauri/src/lib.rs`（tracing_subscriber 初始化）

## P2P 网络（swarm-p2p-core）

### 公开 API 快速参考

```rust
// 启动节点
let (client, mut receiver) = swarm_p2p_core::start::<AppRequest, AppResponse>(
    keypair,
    config,
).await;

// NetClient
client.dial(peer_id);
client.send_request(peer_id, req);
client.send_response(pending_id, resp);
client.bootstrap();
client.start_provide(key);
client.get_providers(key);
client.put_record(record);
client.get_record(key);

// 事件循环
while let Some(event) = receiver.recv().await {
    match event {
        NodeEvent::PeerConnected { peer_id, .. } => { /* ... */ }
        NodeEvent::InboundRequest { request, pending_id, .. } => { /* ... */ }
        // ...
    }
}
```

### 内置能力

- 传输：TCP + QUIC + Noise + Yamux
- 发现：mDNS + Kademlia DHT
- NAT：AutoNAT v2 + DCUtR 打洞 + Relay
- 协议：Request-Response + CBOR

### 事件循环在 `network/event_loop.rs`

`NodeEvent` 被分发给 `DeviceManager`、`AppSyncCoordinator`，并通过 `EventBus` 广播到前端。event_loop 的 `select!` 必须用 `biased;` 保证 `cancel_token.cancelled()` 优先。

**相关文件**：`crates/core/src/network/event_loop.rs`、`libs/core/`（submodule）

### Wizard 同步流程的 `WorkspaceCore` lifetime

`AppCore.workspaces: Mutex<HashMap<Uuid, Weak<WorkspaceCore>>>` 只持 Weak —— 调用方负责持 Strong。`create_workspace_for_sync` 命令 `open_workspace` 拿到 Arc 后**不能** `let _ws_core = ...`：函数 return 时 Arc drop，Weak 立刻 dangling。紧跟着的 `trigger_workspace_sync` 命令在后台 spawn 的 `full_sync` 跑到 `build_local_doc_list` 时 `get_workspace(uuid)` 返回 None，actions=0 早退，UI 看起来 done 但实际没拉任何文档。

**修复（双层）**：

1. **`coordinator.rs spawn_full_sync`**（共享 crate）启动时 upgrade Weak 拿 Strong Arc，move 进 spawn closure (`let _ws_pin = ws_arc;`) 持有到 sync 结束。
2. **`platform::SyncPendingMap`**（桌面端 Tauri state）`Mutex<HashMap<Uuid, Arc<WorkspaceCore>>>` keep-alive 表。`create_workspace_for_sync` `stash(uuid, ws_core)`，`trigger_workspace_sync` 调完 `spawn_full_sync` 后 `release(uuid)`。Mobile-core 端是同名机制（`UniffiAppCore.sync_pending` 字段）。

**为什么不让 frontend 持 Arc？** WorkspaceMap 已经按 window label 持 Arc，但 wizard 流程**没有打开窗口**——只到 dialog 显示 done，用户点"打开"才创建窗口。两次 IPC 之间没人持 Strong 是个真空窗口期。

**踩坑现象**：之前桌面端 wizard 看起来"成功"是因为 dialog 不依赖 `SyncCompleted` 事件（fire-and-forget UI），即便实际同步失败也显示 done。一接到对方设备的真同步流程才暴露：14 篇笔记 0 篇拉到。

**不要做**：不要把 keep-alive 时间窗放在 frontend（让 RN/Tauri 端持 ws Arc 跨 wizard）—— wizard 多 item 模式下这套不 scale，且与 single-active workspace 设计冲突。

**相关文件**：`crates/core/src/workspace/sync/coordinator.rs::spawn_full_sync`、`src-tauri/src/platform/workspace_map.rs::SyncPendingMap`、`src-tauri/src/commands/{workspace,sync}.rs`

### `full_sync` try/finally — `SyncCompleted` 必须 always-emit

`AppEvent::SyncCompleted` 是 frontend wizard 唯一的 terminal 信号。早退路径（`request_doc_list` timeout、`NoWorkspaceDb` 等）漏 emit 会让 UI 卡死无限 spinner。

```rust
pub async fn full_sync(...) -> AppResult<()> {
    emit(SyncStarted);
    let result = run_full_sync(...).await;  // 内层 ? 自由用
    let cancelled = cancel.is_cancelled();
    let error = match &result { Ok(_) => None, Err(e) => Some(e.to_string()) };
    emit(SyncCompleted { workspace_id, peer_id, cancelled, error });
    result
}
```

**协议演进**：`AppEvent::SyncCompleted` 加 `error: Option<String>` 字段。`Tauri sync-completed` payload 同步加 `result: "success" | "cancelled" | "error"` + `error: string | null`，frontend `SyncResult` union 扩展 `"error"`。

**不要做**：不要在 `full_sync` 主体里用 `?` 早退然后忘记 emit `SyncCompleted` —— 任何新增早退分支必须放进 `run_full_sync` inner fn 让 outer 接管 emit。

**相关文件**：`crates/core/src/workspace/sync/full_sync.rs`、`crates/core/src/events.rs`、`src-tauri/src/platform/event_bus.rs`、`src/stores/syncStore.ts`

### Sync 两层拆分

同步模块拆为 AppCore 层和 WorkspaceCore 层：

- **`AppSyncCoordinator`**（全局）：full-sync 去重（`DashMap<(PeerId, Uuid), CancellationToken>`）、SV 补偿定时任务、ctrl topic 消息路由、入站 sync RPC 分发。跟随 P2P 生命周期（start_network 创建、stop_network 销毁）。
- **`WorkspaceSync`**（per-workspace）：GossipSub topic 订阅/退订、pending buffer（closed-doc 缓冲 + 3s debounce flush）、asset check handles。跟随 WorkspaceCore 生命周期。

**正确做法**：

- `WorkspaceCore::set_sync()` 在替换时先 close 旧实例，避免 flush task + subscription 泄漏
- `handle_gossip_update` 接受 `&Arc<WorkspaceCore>` 参数而非内部重新 lookup，避免每条 gossip 消息走全局 Mutex
- `pending_buffer::push()` 溢出时返回 `(source_peer, updates)` 给 caller，确保 overflow 路径也能触发 asset sync

**不要做**：把 per-workspace 状态（pending_buffer、asset_check_handles）放在 AppCore 级别用 workspace_uuid key 区分——workspace 关闭时不会自然清理。

**相关文件**：`crates/core/src/workspace/sync/{coordinator,workspace_sync,pending_buffer}.rs`

## YDocManager — Y.Doc 生命周期

### per-manager 单 loop writeback（Notify-driven）

`YDocManager` 维护 `DashMap<DocUuid, Arc<DocEntry>>`，**全 manager 共享一个 writeback loop**，在 `YDocManager::new` 里 eager spawn。不用 per-doc `interval(500ms)`——那样开 50 tab 就有 50 个独立 wake-up 且 `DocEntry` 被 `JoinHandle` 反向持有，形成循环引用。

**调度机制**：

- `apply_update` / `apply_sync_update` 调 `entry.mark_dirty()` 后 `writeback_notify.notify_one()` 唤醒 loop
- loop 用 `tokio::select!` 在 `cancelled() | notified() | sleep(FALLBACK_TICK_MS=500ms)` 之间切换，`biased` 让 cancel 优先
- `FALLBACK_TICK_MS` 是**信号丢失兜底**，不是轮询节拍——idle 时 loop 阻塞在 `notified()` 上，零 wake-up
- 防抖：loop 被唤醒后 `flush_dirty_debounced()` 只刷 `now - last_update_ms >= DEBOUNCE_MS(1500)` 的 entry，窗口内的留到下次

### 关窗完整 flush 保证

`WorkspaceCore::close()` → `YDocManager::close_all()` 的三步序列**保证所有 dirty doc 在返回前落盘完整**：

1. `writeback_cancel.cancel()` 通知 loop 退出
2. `await writeback_loop` JoinHandle——loop 的 cancel 分支跑 `flush_all_dirty`（**无视防抖窗口**）后才 break
3. `docs.clear()`

**不要做**：`JoinHandle::abort()` 取消 writeback task。abort 只在下个 `.await` 点生效，可能打断 `fs.write_text` / `db.update` 造成 `.md` 半写入。曾经用过 `per-doc handle.abort()`，在 close_doc 里偶尔丢最后一次编辑。

**相关文件**：`crates/core/src/yjs/manager.rs` 的 `writeback_loop` / `flush_entry` / `close_all`

### 外部 .md 变更检测

使用 `notify_debouncer_mini`（100ms debounce）监听 workspace 目录。检测到自己没写过的 .md 改动时：
1. 读取文件内容
2. 调用 `replace_doc_content(&doc, new_md)` 用 `similar` 做 text-diff
3. yjs update origin = "remote"，不标 dirty

**自写检测**：写完 .md 后记录 blake3，notify 触发时如果 hash 一致则忽略。

**reload_lock**：`DocEntry.reload_lock: tokio::sync::Mutex<()>` 互斥 writeback 和外部 reload——loop 的 `flush_entry` 和 `do_reload` 都会 acquire。本次重构保留该锁，新增的 `flush_all_dirty`（cancel 路径）同样 per-entry 获取，不会死锁。

**相关文件**：`crates/core/src/yjs/manager.rs`、`src-tauri/src/platform/file_watcher.rs`

### Schema 容错 restore

`open_doc()` 先 apply 持久化的 `yjs_state` bytes，如果 Y.Text 为空（老版本 BlockNote schema，字段名不同），fallback 用 .md 文件内容 seed Y.Text 并重写 yjs_state。

**相关文件**：`crates/core/src/yjs/manager.rs` 的 `open_doc`

## 错误处理

- Rust 端统一 `AppResult<T>` + `AppError { kind, message }`
- `AppError` 用结构化变体（`YjsDecode { context, reason }` / `SwarmIo { context, reason }` / `DocRowMissing(Uuid)` 等），不要新增 `Xxx(String)` 扁平变体——前端要能按 `kind` 做分支
- `AppError::Yjs` / `Identity` / `Network` / `Pairing` / `Config` 等旧扁平变体**已删除**，不要回退
- 不要 `.unwrap()` / `.expect()` 在 production path（测试除外）
- 外部 I/O 错误用 `?` + `From` 实现自动转换（`sea_orm::DbErr` / `std::io::Error` 已有 `#[from]`）

**相关文件**：`crates/core/src/error.rs`、`src-tauri/src/error.rs`（只是 re-export）

### thiserror 的 `source` 字段是保留名

结构化错误变体里的自由文本字段**不要命名为 `source`**：

```rust
// ❌ 编译失败 —— thiserror 推断 source 字段为 #[source]，要求实现 Error
#[error("yjs decode ({context}): {source}")]
YjsDecode { context: &'static str, source: String },

// ✅ 用 reason（或 detail / message / info）
#[error("yjs decode ({context}): {reason}")]
YjsDecode { context: &'static str, reason: String },
```

thiserror 会把名为 `source` 的字段自动视为 `#[source]` 并要求它实现 `std::error::Error`——`String` 不满足，编译报 `method as_dyn_error not found`。SwarmNote 约定用 `reason: String` 统一存原始错误 `to_string()`，不保留 `Error::source()` 链（FFI 侧拿不到 source chain）。

**相关文件**：`crates/core/src/error.rs`

## API surface 分层：`api::` vs `internal::`

`swarmnote_core` 的 `lib.rs` 把 re-export 分两层：

- **`pub mod api`** — host 面向 API（`AppCore` / `AppCoreBuilder` / `WorkspaceCore` / `YDocManager` / `EventBus` / `AppEvent` / `AppError` 等）。桌面 + mobile-core 都从这里 import
- **`pub mod internal`** — 仅桌面 command 层用的深层访问（`AppNetClient` / `NetManager` / `PairingManager` / `AppSyncCoordinator` / `WorkspaceSync` / `fs::ops` / `yjs::doc_state` / `ensure_workspace_row`）。mobile-core 不应依赖

根级**没有**扁平 re-export，强制 src-tauri 显式写 `use swarmnote_core::api::...` / `internal::...`，让 FFI 层接入时能清楚看到边界。

**相关文件**：`crates/core/src/lib.rs`

## AppCore 通过 `AppCoreBuilder` 构造 + factory 注入

host 通过 factory 闭包注入 per-workspace 的 fs / watcher，而不是在每个 `open_workspace` 调用点手工构造：

```rust
// 桌面
AppCoreBuilder::new(keychain, event_bus, app_data_dir)
    .with_watcher_factory(|p| Arc::new(NotifyFileWatcher::new(p)))
    .build().await?
// 之后:
core.open_workspace(path).await?  // 单参数, fs/watcher 自动用 factory
```

`fs_factory` 默认是 `LocalFs`；`watcher_factory` 默认 `None`（mobile 沙盒不用 watcher）。桌面调用 `.with_watcher_factory` 注册 `NotifyFileWatcher`。

**不要做**：给 `open_workspace` 传 `fs: Arc<dyn FileSystem>, watcher: Option<Arc<dyn FileWatcher>>` 参数——那是旧签名，每个 command 调用点都会重复 `Arc::new(LocalFs::new(path))` 样板，mobile wrapper 会痛苦。

**相关文件**：`crates/core/src/app.rs` 的 `AppCoreBuilder`、`src-tauri/src/lib.rs` setup、`src-tauri/src/platform/workspace_map.rs` 的 `start_core_workspace`

## `AppCore::start_network` / `stop_network` 不跨 await 持锁

`net: Mutex<Option<Arc<NetManager>>>` 锁**不允许**跨 `libp2p` 启动 / GossipSub subscribe / `NetManager::shutdown()` 这种几百毫秒的 await 持有——否则 `network_status()` / `pairing()` / `devices()` 等只读调用全部被阻塞。

正确形态（三段式 CAS）：

```rust
// 1. 短持锁存在性检查
if self.net.lock().await.is_some() { return Err(NetworkAlreadyRunning); }

// 2. I/O 跑在无锁状态
let (client, receiver) = swarm_p2p_core::start(...)?;
let net_manager = Arc::new(NetManager::new(...));
// ... spawn_event_loop / subscribe ...

// 3. 再次拿锁 CAS 安装
{
    let mut guard = self.net.lock().await;
    if guard.is_some() {
        // 竞态输 — shutdown 刚建的 NetManager
        drop(guard);
        net_manager.shutdown().await;
        return Err(NetworkAlreadyRunning);
    }
    *guard = Some(net_manager.clone());
}
```

`stop_network` 同理——先短持锁 `take()` 出 `net_manager`，释放锁后再 `manager.shutdown().await`。

**相关文件**：`crates/core/src/app.rs` 的 `start_network` / `stop_network`

## `WorkspaceCore::close` / `YDocManager::close_all` 结构化错误

两者都不吞错：

- `YDocManager::close_all(&self) -> Vec<(Uuid, AppError)>`：cancel → await loop → `flush_all_dirty` 做 authoritative 最终 sweep → 返回每个 dirty doc 的持久化错误列表
- `WorkspaceCore::close(&self) -> AppResult<()>`：若 ydoc 返回非空 failures，聚合为 `AppError::WorkspaceCloseFailed { workspace_id, failures }`

host（桌面 `cleanup_window` / `AppCore::close_workspace`）拿到 `Err` 后 tracing::warn! + toast 提示用户，不允许继续静默——之前的设计会丢数据。

**不要做**：`close_all` 返回 `()` 然后用 tracing::warn 吞错。那种实现让用户无感知的失败不可接受。

**相关文件**：`crates/core/src/yjs/manager.rs` 的 `close_all` / `flush_all_dirty`、`crates/core/src/workspace/mod.rs` 的 `close`

## Tauri IPC 推送

### Event emit 约定

事件名使用 kebab-case，payload 结构化：

```rust
app.emit("yjs:flushed", FlushedPayload { doc_uuid })?;
app.emit("peer-connected", PeerPayload { ... })?;
```

前端 `listen<Payload>(eventName, handler)` 订阅。

**约定**：事件名以模块前缀命名（`yjs:*`、`network:*`、`pairing:*` 等）。

## 密码学（E2E sharing，`crates/core/src/crypto/`）

E2E 分享的密码学地基在 `crypto/`（entry `crypto.rs` + 子模块 `kdf`/`aead`/`keyx`/`lockbox`/`password`）。设计见 `dev-notes/design/{08-e2e-encryption,04-permissions,05-sharing,11-threat-model}.md`。

### 依赖版本坑：hkdf 用 0.12 不用 0.13

`hkdf 0.13` 升级到 `digest 0.11`，与 workspace 钉死的 `sha2 0.10`（`digest 0.10`）类型不兼容（`Hkdf::<Sha256>` 会因 digest 版本不匹配编译失败）。

**正确做法**：`hkdf = "0.12"` 配 `sha2 = "0.10"`。要升 0.13 必须同时把 workspace `sha2` 升 0.11。

### 随机数：用项目 `rand 0.9`，不要喂 RNG 给 dalek

`x25519-dalek 2` / `ed25519-dalek 2` / `chacha20poly1305 0.10` 内部用 `rand_core 0.6`，与项目的 `rand 0.9`（`rand_core 0.9`）trait 不兼容——把 `rand 0.9` 的 RNG 传进 dalek 的 `*_from_rng` API 会编译失败。

**正确做法**：所有随机材料（key/nonce/secret/salt）用 `crypto::fill_random`（内部 `rand::rng().fill_bytes`，OS 种子 CSPRNG），自己填字节数组；DH/密钥派生全部走静态密钥，不需要给 dalek 喂 RNG。

**不要做**：`XChaCha20Poly1305::generate_key(&mut OsRng)` / dalek 的 `generate(&mut rng)`——会拉进 rand_core 0.6 冲突。

### Ed25519 → X25519 复用派生

设备只有一把 libp2p Ed25519 keypair（`IdentityManager`）。X25519（Lockbox 收发方）从它单层派生，不存第二把：

**正确做法**：
- 本机私钥：`keypair.clone().try_into_ed25519()?.to_bytes()` 取前 32B 作 seed → `ed25519_dalek::SigningKey::from_bytes(seed).to_scalar_bytes()` → `x25519_dalek::StaticSecret::from(..)`（`keyx::derive_x25519_secret`）。
- 对端公钥：从对端 Ed25519 公钥 `VerifyingKey::from_bytes()?.to_montgomery().to_bytes()` → `x25519_dalek::PublicKey::from(..)`（`keyx::ed25519_pub_to_x25519`）。配对时对端只需给 PeerId/Ed25519 公钥即可算出其 X25519 公钥。
- 全局钉死这一条 clamp 约定，**只单层、不层级派生**（层级派生需乘 cofactor，否则 hidden-number-problem）。`IdentityManager::x25519_secret()` / `x25519_public()` 是入口。

### key-committing AEAD（裸 XChaCha20-Poly1305 不是 key-committing）

链接分享是 partitioning-oracle / invisible-salamander 攻击靶心。所有对称封装（`aead::seal`、`lockbox::seal_lockbox`）都额外存一个 HKDF 多挤 32B 的 commitment，解密前用 `subtle::ConstantTimeEq` 常量时间比对——错 key 在 AEAD 之前就被拒。

**wire 帧**：`aead` = `[1B version][4B key_version BE][24B nonce][32B commitment][ciphertext]`，AAD 由调用方传（同步层用 `workspace_id||doc_uuid||key_version||msg_type`）；`lockbox` = `[1B version][24B nonce][32B commitment][ciphertext]`。`frame_key_version()` 先廉价读 key_version 再按 `{key_version→key}` 历史取 key 解密。

**相关文件**：`crates/core/src/crypto/`、`crates/core/src/identity.rs`（X25519 暴露）、`crates/core/src/error.rs`（`AppError::Crypto { context, reason }`）

### permission_ops 缺少 genesis owner op（Phase 5 前置）

`permissions.rs` 的 `materialize()` 要求第一条 op 是 owner 的 **genesis self-grant**（`prev_hash=None` + `Grant` + `new_role=Owner` + `issuer==target`）才能 bootstrap owner，后续非 genesis op 的 issuer 必须当前为 Owner 才生效。但当前**没有任何代码创建这条 genesis op**——`ensure_workspace_row` / `WorkspaceCore::new` / `create_workspace_for_sync` 都只建 workspace 行 + self-Lockbox key，从不调 `build_signed_op`/`save_op`。

**后果**：每个 workspace 的 `load_ops()` 返回空，`materialize()` 返回空 map，没有任何设备被认定为 Owner。Phase 5 把 `build_sealed_workspace_key` 从 `is_paired` 改成权限 gating **之前**，必须先在 owner 创建 workspace 时种下 genesis op，否则 key 分发会全部被拒。

**正确做法**：在 owner 首次创建 workspace（`init_keys=true` 路径，即 `WorkspaceCore::new` 里 key 刚 self-init 那一步）后，若 `load_ops` 为空则 `build_signed_op(identity, Grant, my_peer_id, Some(Owner), key_version=1, prev_hash=None)` + `save_op`。幂等。sync-joined workspace（`init_keys=false`）不种 genesis——它的 owner op 随 permission_ops 广播到达。

**相关文件**：`crates/core/src/workspace/permissions.rs`（`materialize` 三不变式）、`crates/core/src/workspace/mod.rs`（`WorkspaceCore::new` key-init 分支）
