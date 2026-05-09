# 多工作区 per-window 状态管理

## 用户故事

作为用户，我希望能同时打开多个工作区窗口，每个窗口独立运行各自的工作区，互不干扰。

## 依赖

- 无依赖（L0，纯后端重构 + 前端 UI）
- **被依赖**：mDNS 局域网发现、编辑器 yjs 集成等后续功能均建立在 per-window 状态管理之上，应最先完成
- 详见 [GitHub Issue #22](https://github.com/swarm-apps/SwarmNote/issues/22)

## 需求描述

将后端的全局单例工作区状态改为 per-window 管理，同时新增 Workspace Picker UI 作为工作区选择/管理的统一入口：

### 后端改造
- `DbState.workspace_db` → `RwLock<HashMap<String, DatabaseConnection>>`
- `WorkspaceState` → `RwLock<HashMap<String, WorkspaceInfo>>`
- fs watcher 改为 per-window 实例
- 窗口关闭时自动清理资源
- 防止同一工作区在多个窗口中同时打开

### 前端 UI（新增）
- **Workspace Picker**：统一的工作区选择器
- **标题栏更新**：显示当前工作区名称
- **Sidebar Footer 工作区切换**：快速切换 Popover

## UI 设计决策

| 决策项 | 选择 | 理由 |
|--------|------|------|
| 工作区选择入口 | 独立全屏选择器（Workspace Picker） | 参考 Obsidian Vault Switcher，清晰独立 |
| Picker 显示时机 | 仅首次启动（无上次工作区时）；有上次则直接恢复 | 减少干扰，快速进入 |
| Picker 内容 | 最近工作区列表 + 创建新工作区 + 打开文件夹 | 简洁实用 |
| 管理 vs 选择 | 同一个组件，启动时全屏，已有窗口时弹窗显示 | 减少设计工作量 |
| 标题栏 | `Logo + 工作区名称`，替代原来的 `Logo + "笔记"` | 多窗口一眼区分 |
| 已有窗口切换 | Sidebar Footer Popover（最近列表 + "工作区管理" 入口） | 轻量快捷 |
| 快捷键 | `Ctrl+Shift+O` 打开 Workspace Picker | 对标 VS Code |
| 选择行为 | 总是打开新窗口（每个窗口 = 一个工作区） | 简单清晰 |

### UI 设计稿

参考 `milestones/v0.2.0/design/ui-design.pen` 中：
- **工作区选择器 Workspace Picker** — 全屏/弹窗复用
- **Sidebar 工作区 Popover** — 快速切换
- **完整布局 - 多工作区** — 标题栏 + Sidebar Footer 更新

## 技术方案

### 后端

- 状态结构改造：按窗口 label 索引的 HashMap
- 所有 workspace/fs 命令添加 `window: tauri::Window` 参数
- 新增 `open_workspace_window(path)` command
- 窗口关闭事件监听 → 清理 DB 连接、WorkspaceInfo、fs watcher
- 同一工作区路径互斥检查 → focus 已有窗口

### 前端

- 实现 Workspace Picker 组件（全屏 + 弹窗模式）
- 标题栏显示工作区名称
- Sidebar Footer 添加工作区切换行 + Popover
- 适配新的 window-scoped API
- Capabilities 更新支持动态窗口（`ws-*` 通配）

### 启动流程

```mermaid
flowchart TD
    A[App 启动] --> B{config.json 存在?}
    B -- 否 --> C[Onboarding 3步流程]
    C --> D[Workspace Picker 全屏]
    B -- 是 --> E{有上次工作区?}
    E -- 是 --> F[直接恢复上次工作区]
    E -- 否 --> D
    D --> G[选择/创建工作区 → 新窗口]
    G --> H[进入编辑器]
```

## 验收标准

- [ ] `DbState` 和 `WorkspaceState` 使用 `HashMap<String, T>` 按窗口 label 管理
- [ ] 所有 workspace/fs 相关 command 操作对应窗口的状态
- [ ] `open_workspace_window(path)` 可创建新窗口并预绑定工作区
- [ ] 窗口关闭时自动清理后端资源
- [ ] 同一工作区路径不能在多个窗口中同时打开
- [ ] Workspace Picker 显示最近工作区列表，支持创建/打开
- [ ] 标题栏显示当前工作区名称
- [ ] Sidebar Footer 支持工作区切换 Popover
- [ ] `Ctrl+Shift+O` 打开 Workspace Picker
- [ ] `cargo clippy -- -D warnings` 无警告

## 开放问题

- 详细设计见 `openspec/changes/multi-workspace-windows/design.md`（如有）
