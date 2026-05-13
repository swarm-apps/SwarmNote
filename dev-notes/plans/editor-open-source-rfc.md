# RFC: 将 SwarmNote 编辑器重构为可开箱即用的开源 Markdown 编辑器

> **更新（2026-05-12）**：本 RFC 仍是顶层路线说明。具体的 plugin 架构、包边界、`EditorPluginContext` 形状、内置 plugin 清单与分阶段实施细节已在 [editor-plugin-architecture.md](./editor-plugin-architecture.md) 中收敛。下方"包结构建议"与"分阶段实施建议"两节按该决定对齐。

## 背景

当前 SwarmNote 编辑器已经具备一套比较完整的能力：

- CodeMirror 6 Live Preview
- Markdown-first 文档模型
- 图片 / 表格 / 代码块 / admonition 等 richer preview
- 选区格式命令与快捷键
- 基于 Yjs 的协作能力
- 桌面端 React 宿主接入
- React Native WebView 宿主接入

但从代码组织上看，这套编辑器仍然主要服务于 SwarmNote 主应用本身。

当前实现已经证明这条路线成立，但如果后续希望：

1. 将编辑器能力持续独立演进
2. 降低主应用与编辑器内核的耦合
3. 对外开源并提供开箱即用的接入体验
4. 同时支持 React Web / 桌面端与 React Native 宿主

那么现在需要开始按“产品化编辑器”的目标重构边界，而不是继续仅按应用内需求堆叠能力。

---

## 目标

本 RFC 的目标是明确一条编辑器重构路线，使其最终可以作为一个对外开源、可直接嵌入的 Markdown 编辑器产品发布。

目标包括：

- 将编辑器从 SwarmNote 应用逻辑中抽出为更清晰的产品分层
- 明确 editor core、host bridge、平台 UI、交互层之间的职责边界
- 为后续加入更现代的编辑交互做准备（slash command、浮动工具栏、`[[` note link 插入）
- 保持桌面端与移动端共享尽可能多的编辑器语义
- 保持 Markdown-first 路线，不转向完整 block-based editor

---

## 非目标

本次重构**不**追求：

- 将编辑器改造成 Notion 式 block tree 文档模型
- 为第一版开源包一次性实现所有高级 UI
- 立即抽成多个独立 npm 包并对外发布
- 兼容所有历史内部 API

本次更关注的是：先把边界和产品形态理顺。

---

## 产品定位

建议将未来开源编辑器的定位明确为：

> A Markdown-first, live-preview editor built on CodeMirror 6, with optional rich interactions and collaboration support.

也就是：

- **Markdown-first**：Markdown 纯文本是文档真相
- **Live Preview**：通过 decoration / widget 提供更接近富文本的编辑体验
- **Host-friendly**：链接打开、资源解析、上传、搜索、协作 provider 等能力由宿主注入
- **Optional rich interactions**：slash command、浮动工具栏、wikilink 等不是内核强绑定，而是可选交互层
- **Cross-platform**：同一套核心语义可运行于 React Web / Tauri / React Native WebView

这个定位和以下几类方案区分开：

- 不走 Notion / BlockNote 那种 block-based editor 路线
- 不只是纯 markdown 源码编辑器
- 不只是一个 textarea 封装

---

## 为什么现在要重构

当前架构已经出现几个很清晰的信号：

### 1. 编辑器能力已经超过“应用内一个组件”

`packages/editor/` 已经不是一个轻量 wrapper，而是包含：

- 编辑器创建入口
- commands
- decoration / widget / preview 体系
- collaboration 绑定
- host event bridge
- 各种 feature extension

这说明它已经具备独立产品雏形。

### 2. 桌面端和 RN 端已经在共享一套内核

当前已经有：

- `packages/editor/`：共享 CM6 内核
- 桌面端 React 宿主
- RN `packages/editor-web/` + `MarkdownEditor.tsx` + bridge

这意味着现在正适合把“共享的是什么、平台差异在哪”正式收束出来。

### 3. 即将引入更复杂的交互层

后续计划加入：

1. slash command
2. 选区浮动工具栏
3. `[[` note link 插入

这些能力如果继续散落在应用侧临时实现，未来开源时会很难整理。

---

## 核心设计原则

### 1. 文档真相保持 Markdown 纯文本

编辑器继续坚持：

- CM6 doc 是文本
- Y.Text 是文本
- 持久化 Markdown 文件也是文本

所有 richer preview 都是渲染层投影，而不是新的文档真相。

### 2. 宿主边界必须明确

编辑器内核只负责：

- 编辑体验
- 状态更新
- 语义事件
- 命令执行
- 预览渲染

宿主负责：

- 链接打开
- 图片 URL 解析
- 文件上传
- note 搜索
- slash item 数据源
- collaboration provider / awareness 生命周期
- 浮层、菜单、sheet 等平台 UI

### 3. 共享“交互语义”，而不是强行共享 UI

React Web / Tauri 与 RN 的差异主要在：

- tooltip / popover / context menu
- bottom sheet / keyboard-aware overlay
- 坐标系与输入体验

因此跨端应共享：

- trigger 检测
- interaction state schema
- command payload
- event types

而不是强行共享 React DOM 组件。

### 4. 功能应可组合、可开关

开源后，不同接入方需要的功能不同。

因此能力应尽量设计成：

- core 默认功能
- optional feature
- optional interaction layer
- optional collaboration layer

而不是把所有体验硬编码成一个 monolith。

---

## 建议的分层

```mermaid
graph TD
    A[@swarmnote/editor-core] --> B[Markdown / commands / preview / events]
    A --> C[interaction core]
    A --> D[collaboration hooks]

    C --> E[@swarmnote/editor-react]
    C --> F[@swarmnote/editor-react-native]

    E --> G[popover / floating toolbar / slash menu]
    F --> H[bottom sheet / mobile overlay / keyboard bar]
```

### 第一层：editor core

负责：

- `createEditor`
- `EditorControl`
- `EditorEventType`
- commands
- markdown parsing / preview / widget
- collaboration binding
- interaction core（语义层）

约束：

- 不依赖 React
- 不依赖 RN
- 不依赖 Tauri / Expo
- 不依赖具体 UI 组件库

### 第二层：web/react 适配层

负责：

- React wrapper
- 编辑器生命周期托管
- DOM 浮层 UI
- 平台级菜单 / toolbar / slash 面板
- host callback 与 React state 接线

定位上，这一层不只是“把 core 接进 React”，而是面向 **Web / 桌面端** 的官方交互壳。它应优先提供适合桌面端的组件与默认体验，例如：

- 右键菜单
- 选区浮动工具栏
- slash popover
- `[[` note link popover
- command palette
- hover actions

### 第三层：RN 适配层

负责：

- WebView runtime 接线
- bridge 与 editor API 对接
- 移动端工具栏 / sheet / overlay
- keyboard / safe-area 适配

定位上，这一层也不只是“把编辑器塞进 WebView”，而是面向 **移动端** 的官方交互壳。它应优先提供触控友好的组件与默认体验，例如：

- 底部 formatting toolbar
- 键盘上方 command bar
- slash command bottom sheet
- `[[` note link search sheet
- 选区命令条

### 第四层：应用集成层

负责：

- workspace 语义
- 笔记搜索
- 资源路径策略
- 协作 provider 生命周期
- 业务事件埋点
- 应用级快捷键/菜单整合

---

## 包结构建议

> **2026-05-12 收敛**：v0.1 选择**单包 + subpath export** 路线——所有内置插件（math/table/mermaid 等）以 `@swarmnote/editor-core/plugins/<name>` 形式提供，未来抽成独立 npm 包仅需改 import 路径。`editor-react` / `editor-react-native` 仍作为后续 sibling 包推进。详见 [editor-plugin-architecture.md](./editor-plugin-architecture.md#subpath-export-策略)。

第一阶段不一定立即拆成独立仓库，但建议先按以下逻辑收敛目录边界。

### 候选结构

```text
packages/
  editor-core/
  editor-react/
  editor-react-native/
  editor-web-runtime/
```

### 方案说明

#### `editor-core`

包含：

- CM6 内核装配
- commands
- core facets / state / events
- preview extensions
- collaboration extension
- interaction core

#### `editor-react`

包含：

- React wrapper component
- desktop/web 浮层 UI
- toolbar / slash menu / wikilink UI
- React hooks

#### `editor-react-native`

包含：

- RN `MarkdownEditor`
- bridge + host integration
- RN-specific toolbar/sheet/overlay

#### `editor-web-runtime`

包含：

- WebView 中运行的 runtime
- Comlink / endpoint / editor runtime
- 给 RN 宿主用的 HTML runtime

---

## 交互能力建议拆法

后续计划新增的 3 个交互：

1. slash command
2. 选区浮动工具栏
3. `[[` note link 插入

建议不要一开始分别在 Web 和 RN 各写一套完整逻辑，而是先抽出共享的 interaction core。

### interaction core 负责什么

- trigger 检测
- query / match 提取
- selection-based state
- command payload 结构
- 文本替换范围计算
- 编辑器事件格式

例如：

```ts
type SlashTriggerMatch = {
  from: number;
  to: number;
  query: string;
};
```

```ts
type WikiLinkTriggerMatch = {
  from: number;
  to: number;
  query: string;
};
```

```ts
type SelectionToolbarState = {
  visible: boolean;
  anchor: number;
  head: number;
};
```

### Web / RN 层分别负责什么

#### Web
- 浮动 toolbar
- slash popover
- link suggestion popover
- 右键菜单
- tooltip 定位与关闭策略

#### RN
- bottom sheet / keyboard bar
- anchored overlay
- 底部 toolbar
- 软键盘适配
- WebView 坐标桥接

### 平台交互原则

不追求 Web/桌面端与移动端的 UI 强一致，而追求**交互语义一致、表现形态因平台而异**。

也就是：

- 同一个 command 系统可以在不同平台有不同入口
- 同一个 interaction state 可以驱动不同形态的 UI
- 桌面端优先使用 popover、context menu、floating toolbar
- 移动端优先使用 bottom toolbar、keyboard accessory、bottom sheet

例如：

- 桌面端的格式命令可以通过右键菜单、选区浮动工具栏、快捷键触发
- 移动端的格式命令则更适合通过底部 toolbar 或键盘上方工具栏触发

这两者不需要视觉一致，但应该共享同一套命令和状态语义。

---

## 事件模型演进建议

当前编辑器已经有：

- `SelectionChange`
- `SelectionFormattingChange`
- `Change`
- `Focus` / `Blur`
- `LinkOpen`
- `SearchStateChange`

未来建议按“语义事件”继续扩展，而不是直接把 UI 逻辑塞进 editor core。

### 候选新增事件

- `SlashTriggerChange`
- `WikiLinkTriggerChange`
- `SelectionToolbarChange`
- `CommandPaletteRequest`

### 设计原则

- 事件描述“编辑器当前发生了什么”
- 宿主决定“要不要显示什么 UI”
- 事件尽量平台无关
- 不在 event payload 中携带 React / RN 专属对象

---

## 对外 API 草图

### core API

```ts
createEditor(parent, {
  initialText,
  initialSelection,
  settings,
  onEvent,
  imageResolver,
  uploadFile,
  collaboration,
});
```

### React API（目标形态）

```tsx
<SwarmnoteEditor
  initialText=""
  features={{
    slashCommand: true,
    floatingToolbar: true,
    wikilink: true,
    collaboration: false,
  }}
  onLinkOpen={...}
  resolveImage={...}
  uploadFile={...}
  searchNotes={...}
  getSlashItems={...}
/>
```

默认情况下，React 包可以内置一套适合桌面/Web 的官方交互组件，例如：

- 右键菜单
- 选区浮动工具栏
- slash popover
- `[[` note link popover

### RN API（目标形态）

```tsx
<SwarmnoteMarkdownEditor
  initialText=""
  features={{
    slashCommand: true,
    floatingToolbar: false,
    wikilink: true,
  }}
  onLinkOpen={...}
  searchNotes={...}
  getSlashItems={...}
/>
```

默认情况下，RN 包则提供移动端更合适的官方交互组件，例如：

- 底部 formatting toolbar
- 键盘上方 command bar
- slash command sheet
- `[[` note link 搜索 sheet

说明：

- RN 上不一定要照搬“浮动在选区上方”的桌面工具栏
- 同一语义能力可在不同平台采用不同 UI 呈现
- 目标不是 UI 统一，而是用户能力统一

---

## 官方组件策略

建议 `editor-react` 和 `editor-react-native` 都提供两层能力：

### 1. 开箱即用层

直接提供官方默认交互组件，让接入方不需要从零搭建常见编辑体验。

#### `editor-react` 默认组件候选
- ContextMenu
- FloatingToolbar
- SlashMenu
- WikiLinkMenu
- CommandPalette

#### `editor-react-native` 默认组件候选
- BottomFormattingToolbar
- KeyboardAccessoryBar
- SlashCommandSheet
- WikiLinkSearchSheet
- SelectionCommandBar

### 2. Headless / 可组合层

除了默认组件，还应暴露 hooks、状态与 command 接口，便于高级使用方替换 UI。

例如：

- `useSlashCommandState()`
- `useWikiLinkState()`
- `useSelectionToolbarState()`
- command schema / interaction payload

这样既满足“开箱即用”，也保留“高度可定制”。

---

## 默认能力与可选能力

建议将未来开源版本的 feature 分成两类。

### 默认内置能力

- Markdown 语言支持
- 基础命令系统
- selection formatting
- inline rendering
- 基础 markdown decoration
- link open event
- search

### 可选增强能力

- block images
- block tables
- code block preview
- admonition
- math rendering
- collaboration
- slash command
- floating toolbar
- wikilink 插入

这样更利于开源后的使用方按需组合。

---

## 对现有代码的影响

### 1. `packages/editor` 不应继续承载所有宿主 UI 假设

例如：

- 不应直接绑定桌面端右键菜单 UI
- 不应把 Tauri / Expo 具体 API 混进 core
- 不应把 note 搜索等业务数据源写进内核

### 2. `EditorEventType` 将成为更重要的稳定契约

如果后续要跨端共享交互层，事件语义需要尽早稳定。

### 3. RN 端 bridge 需要被视作正式适配层，而不是临时方案

当前 RN 方案本质上已经是：

- shared editor core
- web runtime
- native host shell

这本身就是一个合理的产品架构，不应被视为纯过渡代码。

---

## 分阶段实施建议

> **2026-05-12 收敛**：原 Phase 1-5 描述偏边界整理路线；实际选择走 **Model B 全量 + 内置 plugin 全部重写** 的更激进路线。最终 Phase 表见 [editor-plugin-architecture.md 后续路线](./editor-plugin-architecture.md#后续路线均为无破坏性扩展)。本节保留为思考记录。

### Phase 1：清理边界

目标：先让 core / host / UI 的职责更干净。

内容：

- 清点现有 `packages/editor` 中哪些逻辑属于宿主层
- 明确 `EditorEventType` 的职责边界
- 清点哪些 command/feature 已可稳定对外暴露
- 明确桌面端和 RN 端目前各自的集成方式

输出：

- 一份边界清单
- 一份 API 稳定性清单

### Phase 2：抽交互语义层

目标：为 slash / toolbar / wikilink 建立共享 interaction core。

内容：

- 抽 trigger 检测逻辑
- 抽 interaction state schema
- 定义新增 event payload
- 抽通用插入/替换 command helper

输出：

- interaction core 初版
- selection/slash/wikilink 的统一协议

### Phase 3：桌面端先落地

目标：在桌面端先验证这套交互模型。

内容：

- 实现 slash command
- 实现选区浮动工具栏
- 实现 `[[` note link 插入
- 验证事件模型是否足够

输出：

- 一套桌面端完整交互
- 对 interaction core 的修正反馈

### Phase 4：RN 端复用

目标：在 RN 端复用同一套交互语义。

内容：

- 对接 RN WebView bridge
- 将 slash / wikilink / formatting 适配到移动端 UI
- 用 bottom sheet / keyboard bar 替代不适合移动端的浮层形态

输出：

- 跨端统一语义、平台差异化呈现的交互层

### Phase 5：开源打包与文档化

目标：整理成真正可对外发布的 editor package。

内容：

- 重命名与包边界收束
- 补 README / examples / API docs
- 提供最小接入 demo
- 提供 feature 组合示例

输出：

- 第一版对外可消费包
- 示例项目与文档站

---

## 开放问题

1. ~~`packages/editor` 是否继续沿用 submodule 形式，还是在开源前迁移为独立 monorepo/workspace？~~ **已闭环**：迁移为 sibling 仓 `swarmnote-editor` 的 pnpm workspace monorepo（见 [README](../../../swarmnote-editor/README.md)）。
2. ~~interaction core 是放进 `editor-core`，还是单独拆成 `editor-interactions`？~~ **已闭环（2026-05-12）**：作为 first-party plugin 留在 `editor-core/plugins/interactions/`，与第三方插件共用 `EditorPlugin` API。详见 [editor-plugin-architecture.md](./editor-plugin-architecture.md)。
3. React Web 默认 UI 是否要内置一套"官方工具栏/菜单"，还是只提供 hooks 与 headless 状态？
4. RN 端是否需要与 Web 保持相同交互形态，还是只共享语义、不共享视觉表现？
5. collaboration 是否作为第一版开源能力公开，还是放到第二阶段？
6. 是否需要在第一版就提供受控（controlled）模式与非受控（uncontrolled）模式两套 API？

---

## 建议的下一步

建议下一步先做一件事：

> **列出当前 `packages/editor` 中“属于 core”与“属于 host”的边界清单。**

因为不先把这条线理清，后续加 slash command、wikilink、floating toolbar 时，新的交互仍然容易继续长在错误位置。

完成这一步后，再进入 interaction core 设计，会稳很多。
