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
