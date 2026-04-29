---
name: dev-workflow
description: |
  项目开发工作流技能。在以下场景自动调用：
  (1) 编写或修改任何 src/ 或 src-tauri/src/ 下的代码
  (2) 添加新依赖或修改配置文件
  (3) 完成一个 feature 或修复一个 bug
  触发关键词：组件开发、bug 修复、重构、新功能、依赖升级、配置变更
---

# Dev Workflow — 项目开发工作流

## 工作流程

### 1. 开发前：加载相关知识

根据当前任务，读取 `dev-notes/knowledge/` 下的相关主题文件：

| 任务类型 | 读取文件 |
| -------- | -------- |
| 主题/样式/UI 组件 / 窗口装饰 | `theme-and-styling.md` |
| 编辑器（CM6 / Y.Doc / y-codemirror.next / @swarmnote/editor） | `editor.md` |
| Rust 后端 / Tauri command / SeaORM / YDocManager / P2P | `rust-backend.md` |
| 构建 / 工具链 / i18n / Cargo workspace / Lefthook | `toolchain.md` |

**读取方式**：使用 Read 工具读取对应文件，遵循其中记录的最佳实践和注意事项。

如果不确定读哪个，读取 `dev-notes/knowledge/` 目录列表，根据文件名判断。

### 2. 开发中：遵循最佳实践

同时参考以下通用 skill（如果与当前任务相关，自动调用）：

- `/vercel-react-best-practices` — React 性能优化模式（re-render、bundle、waterfalls）
- `/tauri-v2` — Tauri v2 IPC、capabilities、配置模式
- `/rust-best-practices` — Rust 通用规范
- `/rust-async-patterns` — Tokio、异步事件循环、取消/并发模式
- `/lingui-best-practices` — i18n 国际化规范
- `/tailwindcss` — Tailwind CSS v4 工具类与配置
- `/tailwind-css-patterns` — Tailwind 响应式布局、Flexbox、Grid、间距、排版模式
- `/tailwind-design-system` — Tailwind 设计系统、设计令牌、组件库模式

**优先级**：项目知识库 > 通用 skill > Claude 自身知识。当项目知识库中有明确记录时，以项目知识库为准。

### 3. 开发后：更新知识库

完成代码修改后，**检查是否产生了新的项目知识**：

**需要记录的内容**：
- 新引入的依赖及其正确用法
- 发现的配置坑和 workaround
- 做出的架构决策及原因
- 与通用最佳实践不同的项目特定做法
- 解决的 bug 的根因（如果不明显的话）

**不需要记录的内容**：
- 代码本身能表达的东西（看代码就能懂）
- 通用编程知识（不特定于本项目）
- 临时性的调试信息

**更新方式**：
1. 判断属于哪个主题文件
2. 追加新条目到对应文件的合适分类下
3. 如果现有主题都不合适，创建新的主题文件
4. 如果发现已有条目过时，更新或删除它

**条目格式**：

```markdown
### 条目标题

简短描述做了什么、为什么这样做。

**正确做法**：
- 具体的代码模式或配置

**不要做**（如果有）：
- 错误的做法及原因

**相关文件**：`path/to/file`
```

### 4. 代码质量检查

开发完成后，运行 `/simplify` 检查代码质量。前后端分别运行对应的 lint/format：

```bash
# 前端
pnpm lint
pnpm exec tsc --noEmit

# Rust
cd src-tauri && cargo fmt
cd src-tauri && cargo clippy -- -D warnings
cd src-tauri && cargo test
```
