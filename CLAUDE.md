# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 开发工作流

**IMPORTANT**：执行任何开发任务（编写代码、修改配置、添加依赖）前，必须先调用 `/dev-workflow` skill。它会加载项目知识库（`dev-notes/knowledge/`）中的最佳实践和踩坑记录，并在开发完成后引导更新知识库。

知识库主题：

- `dev-notes/knowledge/theme-and-styling.md` — shadcn/ui、主题变量、窗口装饰
- `dev-notes/knowledge/editor.md` — CM6、Y.Doc、y-codemirror.next、@swarmnote/editor submodule
- `dev-notes/knowledge/rust-backend.md` — Tauri command、SeaORM、YDocManager、P2P
- `dev-notes/knowledge/toolchain.md` — Biome、Lefthook、Lingui、Cargo workspace、Vite

## Project Overview

SwarmNote is a decentralized, local-first, peer-to-peer note-taking app built with Tauri v2 + React 19 + Rust. Notes sync between devices via P2P networking (libp2p) without a central server. Targets desktop (Windows/macOS/Linux) and Android.

## Development Commands

```bash
# First-time setup: init git submodule (libs/core) + install deps
git submodule update --init --recursive
pnpm install

# Launch full Tauri desktop app (starts frontend + Rust backend)
pnpm tauri dev

# Frontend dev server only (Vite on port 1420)
pnpm dev

# Build frontend (TypeScript compile + Vite build)
pnpm build

# Build Tauri app for distribution
pnpm tauri build

# Frontend lint (Biome check)
pnpm lint

# Frontend lint CI mode (no auto-fix, exits non-zero on errors)
pnpm lint:ci

# Frontend format (Biome auto-fix)
pnpm format

# Rust format + lint
cd src-tauri && cargo fmt
cd src-tauri && cargo clippy -- -D warnings

# Rust backend tests
cd src-tauri && cargo test

# Run a single Rust test
cd src-tauri && cargo test <test_name>

# yrs-blocknote crate tests (independent of Tauri)
cd crates/yrs-blocknote && cargo test

# i18n: extract messages from source
pnpm lingui extract

# Generate CHANGELOG from conventional commits
pnpm changelog

# Show unreleased changes
pnpm changelog:latest
```

No test framework is configured for the frontend yet.

**Linux 构建系统依赖**（Ubuntu/Debian）：`libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libdbus-1-dev`

**工具版本**：Node 22+、pnpm 10+、Rust stable

## Code Quality Toolchain

- **Biome** (`biome.json`): 前端 lint + format（替代 ESLint + Prettier），`recommended` 规则集，自动 organize imports，2空格缩进，行宽 100。排除 `routeTree.gen.ts`、`src/locales/**`、`src/components/ui/**`（部分 a11y 规则关闭）
- **rustfmt** (`src-tauri/rustfmt.toml`): Rust 代码格式化，4空格缩进（`.editorconfig` 控制）
- **Clippy**: Rust 静态分析，CI 中以 `-D warnings` 运行
- **Lefthook** (`lefthook.yml`): Git hooks 管理
  - `pre-commit`: Biome check（前端）+ cargo fmt --check（Rust），并行执行
  - `commit-msg`: commitlint 校验 Conventional Commits 格式
- **commitlint** (`commitlint.config.js`): 提交信息必须遵循 [Conventional Commits](https://www.conventionalcommits.org/)（`feat:`, `fix:`, `docs:`, `chore:` 等）
- **git-cliff** (`cliff.toml`): 基于 Conventional Commits 自动生成 CHANGELOG
- **GitHub Actions** (`.github/workflows/ci.yml`): PR/push 到 main/develop 时自动跑 lint + build + test

## Architecture

### High-Level Structure

```text
swarmnote/
├── src/                  # React + TypeScript frontend
├── src-tauri/            # Rust backend (Tauri v2, Cargo workspace)
│   ├── entity/           # SeaORM entity 定义（独立 crate）
│   ├── migration/        # SeaORM 数据库迁移（独立 crate）
│   └── src/              # Tauri commands + 业务逻辑
├── crates/
│   └── yrs-blocknote/    # 独立通用 crate：BlockNote Y.Doc ↔ Markdown 双向转换
├── libs/core/            # swarm-p2p-core (git submodule, libp2p 封装)
├── docs/                 # Astro + Starlight 文档站
├── openspec/             # Spec-driven change management (OpenSpec)
├── dev-notes/            # Planning docs, PRD, tech selection
└── milestones/           # Version planning: requirements + design per version
```

### yrs-blocknote crate

独立通用库（`crates/yrs-blocknote/`），实现 BlockNote JSON、Markdown、Y.Doc 三种格式的双向转换。不依赖 Tauri/SwarmNote，可发布到 crates.io。

依赖：`yrs 0.25`（yjs Rust 实现）+ `comrak 0.51`（GFM Markdown 解析/渲染）。Block 数据模型为中心枢纽，serde 序列化与 BlockNote JSON 格式一致。

公开 API：`markdown_to_blocks` / `blocks_to_markdown` / `doc_to_blocks` / `blocks_to_doc` / `markdown_to_doc` / `doc_to_markdown` / `replace_doc_content`（在现有 Doc 上清空并重写内容，CRDT 历史连续）。

**重要约定**：所有 `Doc` 必须以 `OffsetKind::Utf16` 创建（与前端 JS yjs 一致）。yrs 默认的 `OffsetKind::Bytes` 会导致 CJK 字符 `block_offset` 溢出 panic。

SwarmNote 通过 path 依赖引用：`yrs-blocknote = { path = "../crates/yrs-blocknote", features = ["uuid"] }`。

### Backend Modules (src-tauri/src/)

| 模块 | 职责 |
|------|------|
| `identity/` | 设备身份（PeerId）、OS keychain 持久化、设备名管理（device_name 通过 agent_version 传播到 P2P 网络） |
| `workspace/` | 多窗口工作区管理、per-window DB 绑定（RwLock<HashMap>）、最近工作区 |
| `document/` | 文档 & 文件夹 CRUD（通过 SeaORM 操作 workspace DB） |
| `fs/` | 文件系统 I/O、workspace 目录扫描、文件监听（notify debounce）、媒体保存 |
| `network/` | P2P 节点生命周期（NetManager）、事件循环分发、DHT 在线宣告 |
| `pairing/` | 设备配对码生成/验证、配对请求/响应流程（PairingManager） |
| `protocol/` | 自定义 P2P 协议定义（AppRequest/AppResponse）、OsInfo（设备信息通过 agent_version 编解码，含 name/hostname） |
| `device/` | DeviceManager——追踪在线设备信息 |
| `yjs/` | YDocManager——per-doc Y.Doc 生命周期、yrs ↔ DB 持久化、debounce 自动保存、外部 .md 变更检测与重载 |
| `config/` | 全局配置持久化（最近工作区列表等） |
| `tray.rs` | 系统托盘（仅桌面端，最后一个窗口隐藏到托盘而非退出） |
| `error.rs` | 统一错误类型 AppError，序列化为 `{ kind, message }` 供前端消费 |

### Database Architecture

**双数据库设计**：
- **devices.db**（全局，app data 目录）：存储配对设备信息
- **workspace.db**（per-workspace，工作区根目录 `.swarmnote/`）：存储文档、文件夹、工作区元数据

`DbState` 通过 `RwLock<HashMap<String, DatabaseConnection>>` 管理多窗口的工作区 DB 连接。ORM 使用 SeaORM 2.0-rc + SQLite。主键和外键统一使用 `Uuid`（v7）。

### Frontend Architecture

**路由**（TanStack Router，文件路由自动生成 `routeTree.gen.ts`）：
- `/` — 主页面：Onboarding → WorkspacePicker → AppLayout（Sidebar + EditorPane）
- `/settings/*` — 设置窗口（独立窗口打开）：general / sync / devices / about

**状态管理**（9 个 Zustand stores）：
- 持久化到 tauriStore（plugin-store）：`onboardingStore`、`preferencesStore`、`uiStore`
- 纯内存：`workspaceStore`、`editorStore`、`fileTreeStore`、`networkStore`、`pairingStore`、`notificationStore`

**Tauri 事件桥接**：`networkStore` 和 `pairingStore` 在模块级监听 Rust 端 emit 的事件（`peer-connected`、`pairing-request-received` 等），自动更新前端状态。

**i18n**：Lingui（zh 为源语言，en 异步加载），BlockNote 编辑器有独立的字典映射。

**UI 组件**：shadcn/ui（Radix + Tailwind CSS 4 + cva）+ sonner toast。平台感知布局——macOS Overlay 标题栏（hidden_title + traffic_light_position），Windows/Linux 自定义标题栏。动态创建窗口统一使用 `with_platform_decorations()` 辅助函数。

### Frontend-Backend Bridge

Frontend 通过 `src/commands/*.ts` 封装 `invoke()` 调用，对应 Rust 端 `#[tauri::command]`。命令在 `src-tauri/src/lib.rs` 的 `generate_handler![]` 中注册。Capabilities 在 `src-tauri/capabilities/` 中声明（Tauri v2 安全模型）。

### Rust Library Naming

The Rust lib is named `swarmnote_lib` (not `swarmnote`) to avoid a Windows naming conflict between the lib and bin targets.

### Version Planning (milestones/)

每个版本在 `milestones/vX.Y.Z/` 下管理需求和设计。模板和规范由 `/project` skill 管理（`.claude/skills/project/`），milestones 目录只保留实际内容。

工作流：`/project explore` 讨论需求生成文档 → `/project plan` 拆分为 GitHub Issues + Milestone → `/project sprint` 管理开发迭代。

## Documentation Conventions

- **图表统一使用 Mermaid**：文档中的流程图、架构图、时序图等一律使用 Mermaid 语法（```mermaid），不使用 ASCII art 画图

## Code Conventions

- **TypeScript**: strict mode, `noUnusedLocals`/`noUnusedParameters` enforced, ESNext modules, `react-jsx` transform, path alias `@/` → `src/`
- **React**: functional components with hooks, PascalCase filenames
- **UI 组件优先使用 shadcn/ui**：能用 shadcn 组件就用（Button、Dialog、AlertDialog、Select、Switch、InputOTP 等），避免自定义原生元素
- **颜色统一使用主题变量**：`text-primary`、`bg-muted`、`border-destructive` 等 CSS 变量，不硬编码颜色值（如 `text-indigo-600`、`bg-green-500`），后续换主题时自动适配
- **Rust**: standard rustfmt, `#[tauri::command]` pattern, snake_case, tracing（非 log）记录日志
- **Error handling**: Rust 端统一使用 `AppResult<T>` / `AppError`，序列化为 `{ kind, message }` JSON
- **Package manager**: pnpm (not npm/yarn)
- **Git flow**: `main` → `develop` → `feature/*` branches, PRs required
- **Git commits**: 不要添加 `Co-authored-by` trailer

## Key Config Files

- `vite.config.ts` — dev server fixed to port 1420 (required by Tauri), excludes `src-tauri/` from watch, plugins: TanStack Router + React + Lingui + Tailwind CSS
- `src-tauri/tauri.conf.json` — runs `pnpm dev` before dev, `pnpm build` before build, frontend dist at `../dist`
- `src-tauri/Cargo.toml` — Cargo workspace（root + entity + migration），swarm-p2p-core 通过 path 依赖引入
- `tsconfig.json` — target ES2020, bundler module resolution
- `biome.json` — Biome lint + format config, scoped to `src/` and root config files
- `lingui.config.ts` — i18n: zh 源语言, en 翻译, catalogs in `src/locales/`
- `lefthook.yml` — pre-commit and commit-msg hooks
- `cliff.toml` — git-cliff changelog generation config
- `.editorconfig` — cross-editor formatting: UTF-8, LF, 前端 2空格 / Rust 4空格

## Sister Project: SwarmDrop & swarm-p2p-core

SwarmDrop 是同作者的去中心化文件传输工具，已验证完整 libp2p 链路（v0.4.4）。其核心 P2P 网络层已抽离为 `swarm-p2p-core` 库（`libs/core/`，git submodule）。

### swarm-p2p-core 公开 API

```rust
// 启动节点，返回命令发送端 + 事件接收端
let (client, receiver) = swarm_p2p_core::start::<AppRequest, AppResponse>(keypair, config).await;

// NetClient<Req, Resp> — 发送网络命令
client.dial(peer_id)                    // 连接节点
client.send_request(peer_id, req)       // 发送请求
client.send_response(pending_id, resp)  // 回复请求
client.bootstrap()                      // 加入 DHT
client.start_provide(key) / get_providers(key)  // DHT provider
client.put_record(record) / get_record(key)     // DHT 记录

// EventReceiver<Req> — 接收网络事件
receiver.recv() -> NodeEvent<Req>
// 事件类型: PeersDiscovered, PeerConnected, InboundRequest, NatStatusChanged, HolePunchSucceeded...
```

### swarm-p2p-core 内置能力

| 层面     | 能力                                          |
| -------- | --------------------------------------------- |
| 传输     | TCP + QUIC + Noise 加密 + Yamux 多路复用      |
| 发现     | mDNS（局域网）+ Kademlia DHT（跨网络）        |
| NAT 穿透 | AutoNAT v2 + DCUtR 打洞 + Relay 中继          |
| 协议     | Request-Response + CBOR，泛型 `<Req, Resp>`   |

### SwarmNote 复用策略

SwarmNote 通过 git submodule 引入 `swarm-p2p-core`（`libs/core/`），已完成：

1. 定义 `AppRequest` / `AppResponse`（`protocol/` 模块）
2. 复用 `NetClient` / `EventReceiver` / `NodeConfig` 架构
3. 事件循环（`network/event_loop.rs`）将 NodeEvent 分发到 DeviceManager、PairingManager 和 Tauri 前端事件

## Current Development Status

### 已实现

- **工作区管理**：打开/创建工作区、多窗口、最近工作区、自动恢复
- **文档编辑**：BlockNote 富文本编辑器、Markdown 持久化、自动保存（1.5s debounce）、媒体上传、外部 .md 变更自动同步到 Y.Doc
- **文件树**：react-arborist、增删改查、文件监听
- **P2P 网络**：节点启停、DHT 发现、对等连接、NAT 检测、事件循环
- **设备配对**：配对码、请求/响应、配对设备列表、解除配对
- **身份管理**：keychain 持久化、设备信息
- **UI/UX**：自定义标题栏、暗/亮主题、i18n（中/英）、命令面板、系统托盘

### 待实现（DB schema 骨架已有）

- **协作编辑**：yjs CRDT 同步（documents 表有 yjs_state/state_vector 字段）
- **工作区共享**：share_invites、workspace_keys、permissions 表已定义
- **全文搜索**：无搜索 UI 或后端索引
