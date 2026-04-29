# 工具链与构建

## 工具版本

- **Node** 22+
- **pnpm** 10+（**固定使用 pnpm**，不要用 npm / yarn）
- **Rust** stable

## 常用命令

```bash
# 初始化
git submodule update --init --recursive
pnpm install

# 开发
pnpm tauri dev        # 完整 Tauri 桌面应用（前端 + Rust）
pnpm dev              # 只起前端 Vite dev server（port 1420）

# 构建
pnpm build            # 前端 TS 编译 + Vite 构建
pnpm tauri build      # 分发版 Tauri app

# 质量检查
pnpm lint             # Biome check
pnpm lint:ci          # CI 模式，不自动修复
pnpm format           # Biome auto-fix
cd src-tauri && cargo fmt
cd src-tauri && cargo clippy -- -D warnings
cd src-tauri && cargo test
cd crates/yrs-blocknote && cargo test   # （已删除，保留记录）

# i18n
pnpm lingui extract

# CHANGELOG
pnpm changelog
pnpm changelog:latest
```

## Linux 构建系统依赖

```
libwebkit2gtk-4.1-dev
libappindicator3-dev
librsvg2-dev
patchelf
libdbus-1-dev
```

## Biome

`biome.json` 替代 ESLint + Prettier。

### 配置要点

- `recommended` 规则集
- 自动 organize imports
- 2 空格缩进，行宽 100
- 排除 `routeTree.gen.ts`、`src/locales/**`、`src/components/ui/**`（部分 a11y 规则关闭）

### 常见抑制注释放置

`// biome-ignore lint/correctness/useExhaustiveDependencies: reason` 必须**紧贴**被抑制的行的上一行，中间不能有空行。否则 Biome 会把它当成 dead comment。

**相关文件**：`biome.json`

## Rust 格式化与 Clippy

- **rustfmt** 配置在 `src-tauri/rustfmt.toml`，4 空格缩进
- **clippy** CI 以 `-D warnings` 运行，本地也应该这样跑

缩进由 `.editorconfig` 统一：前端 2 空格、Rust 4 空格。

## Lefthook — Git hooks

`lefthook.yml`：

- `pre-commit`：**并行**跑 Biome check（前端）+ cargo fmt --check（Rust）
- `commit-msg`：commitlint 校验 Conventional Commits

### 不要绕过 hooks

**不要**使用 `--no-verify` 跳过 pre-commit hook。如果 hook 失败，说明代码有问题，先修复。

## commitlint — Conventional Commits

`commitlint.config.js` 要求提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/)：

```
feat: 新功能
fix: bug 修复
docs: 文档
chore: 维护、重构、工具链
refactor: 重构（非新功能非 fix）
test: 测试
ci: CI 配置
perf: 性能优化
```

**不要**加 `Co-authored-by` trailer。

## git-cliff — CHANGELOG

`cliff.toml` 基于 Conventional Commits 自动生成 CHANGELOG。跑 `pnpm changelog:latest` 看未发布的变更。

## GitHub Actions

`.github/workflows/ci.yml`：PR 和 push 到 main/develop 时自动跑：

- Biome lint
- TypeScript compile
- Cargo fmt check
- Cargo clippy (`-D warnings`)
- Cargo test

## Git flow

- `main` → `develop` → `feature/*`
- PR 必须从 feature 分支到 develop，从 develop 到 main
- 子模块（`packages/editor`、`libs/core`）修改要先 push 子模块再 push 主仓库，保证 submodule 指针不悬空

## Vite + 路由

- dev server 固定端口 **1420**（Tauri 要求）
- 排除 `src-tauri/` 的文件监听
- 插件：TanStack Router（文件路由自动生成 `routeTree.gen.ts`，**不要手改**）+ React + Lingui + Tailwind CSS

**相关文件**：`vite.config.ts`

## Lingui i18n

- 源语言：**zh**（中文）
- 异步加载：**en**
- catalog 位于 `src/locales/`
- 用 `useLingui().t` / `<Trans>` 翻译，详见 `lingui-best-practices` skill

**相关文件**：`lingui.config.ts`、`src/locales/`

## Tauri 配置

- `src-tauri/tauri.conf.json` 的 `beforeDevCommand: "pnpm dev"`、`beforeBuildCommand: "pnpm build"`
- 前端 dist 在 `../dist`（相对 src-tauri）
- 动态窗口必须走 `with_platform_decorations()`（见 `theme-and-styling.md`）

## Cargo workspace

`src-tauri/Cargo.toml` workspace 包含：

- root crate（swarmnote_lib + 二进制）
- `entity`（SeaORM entity 定义）
- `migration`（SeaORM 迁移）

`swarm-p2p-core` 通过 submodule + path 依赖引入（`libs/core/`）。

## 文档约定

### Mermaid 图表

`dev-notes/` 下的文档（blog、design 等）中，所有图表统一使用 **Mermaid** 语法（` ```mermaid `），不要用 ASCII art。流程图、时序图、架构图、状态机都走 Mermaid。

此规则仅限写入文件的文档，对话中可以用 ASCII art。

### Mermaid 节点文本坑

节点文本里**不支持 Markdown 列表语法**：`1.` + 空格 或 `-` + 空格 会被 Mermaid 误解析。解决：去掉空格，写成 `1.内容`。

## TypeScript 约定

- `strict: true`、`noUnusedLocals` / `noUnusedParameters` 开启
- `target`: ESNext
- `module`: bundler 模式解析
- 路径别名：`@/*` → `src/*`
- React 用 `react-jsx` transform

**相关文件**：`tsconfig.json`

## OpenSpec

OpenSpec change 目录 `openspec/` 被 `.gitignore` 排除（`**/openspec`），**不会**进 commit。它是本地的 change management 工具，通过 `/opsx:*` 系列 skill 使用。
