# 主题与样式

## 总览

- **UI 库**：shadcn/ui（Radix + Tailwind CSS 4 + cva），通过 CLI 添加到 `src/components/ui/`
- **配色**：CSS 变量（`--primary` / `--muted` / `--destructive` ...），通过 Tailwind 工具类消费
- **主题切换**：`uiStore.resolvedTheme`（light / dark），在 `<html>` 上切 class
- **窗口装饰**：平台感知——macOS Overlay、Windows/Linux 自定义标题栏

## UI 组件

### 优先使用 shadcn/ui

能用 shadcn 就用：Button、Dialog、AlertDialog、Select、Switch、InputOTP、Sidebar、Popover、Tooltip 等。避免自己写原生 `<button>` / `<input>` 再套 Tailwind 拼凑。

**正确做法**：
- 缺组件时用 `npx shadcn@latest add <component> -y -o` 生成到 `src/components/ui/`
- 生成的组件源码可以改（与 npm 包区别），但尽量通过 `cva` 的 variants 扩展，不动基础结构
- 组合组件（如带图标的 Button）建议封装到 `src/components/` 下，而不是直接 inline 重复

**不要做**：
- 直接用 `<button className="px-4 py-2 rounded...">` 手搓，丢掉无障碍属性
- 在 `src/components/ui/` 内硬写项目特定逻辑，那是通用层

**相关文件**：`src/components/ui/`、`components.json`

### 生成组件 Biome 白名单

`biome.json` 已把 `src/components/ui/**` 部分 a11y 规则关闭（因为是自动生成代码）。修改这些组件时如果触发 lint，不要去调 `biome.json`，而是遵守现有生成组件的结构。

## 颜色与主题

### 颜色只用主题变量

不硬编码颜色值。所有颜色必须走 CSS 变量，这样后续换主题时自动适配。

**正确做法**：
```tsx
<div className="text-primary bg-muted border-destructive">
<button className="text-sidebar-foreground hover:bg-sidebar-accent/50">
```

**不要做**：
```tsx
<div className="text-indigo-600 bg-green-500">   // ❌ 硬编码
<div style={{ color: '#4f46e5' }}>                // ❌ inline 字面量
```

变量定义见 `src/App.css` 和 shadcn/ui 默认主题（nova 预设）。

### 暗/亮主题切换

主题由 `useUIStore((s) => s.resolvedTheme)` 提供（已经做过 system 解析）。CodeMirror 编辑器的暗亮切换通过 `control.updateSettings({ theme: { appearance: ... } })` 响应式下发。

**相关文件**：`src/stores/uiStore.ts`、`src/App.css`、`src/components/editor/NoteEditor.tsx`

## 窗口装饰

### 平台感知的标题栏

- **macOS**：`hiddenTitle: true` + `titleBarStyle: "Overlay"` + 自定义 `trafficLightPosition`
- **Windows / Linux**：用自定义 HTML 标题栏（`decorations: false`），组件在 `src/components/layout/TitleBar.tsx`

动态创建窗口时**必须**走 `with_platform_decorations()` 辅助函数，避免各入口复制粘贴平台分支。

**相关文件**：`src-tauri/src/windowing.rs`（辅助函数）、`src/components/layout/TitleBar.tsx`

### 设置窗口独立于主窗口

`/settings/*` 是独立的 Tauri 窗口，通过 `openSettingsWindow(tab)` 打开。它和主窗口共享同一个 React 运行时（但 URL 路由不同），状态通过 tauriStore 持久层 + Tauri event 广播跨窗口同步。

**相关文件**：`src/commands/workspace.ts` 中的 `openSettingsWindow`、`src-tauri/capabilities/`

## 其他

### 字体与 Tailwind 4

Tailwind CSS 4 采用 CSS-first 配置（`@theme` 块），不用 `tailwind.config.js`。新增主题 token 直接改 `src/App.css` 的 `@theme` 块。

### 浮层依赖 Portal

shadcn/ui 的 Dialog / Popover / Tooltip / Select 使用 Radix Portal，默认挂到 body。桌面端没有 PortalHost 配置需求（RN 才需要）。
