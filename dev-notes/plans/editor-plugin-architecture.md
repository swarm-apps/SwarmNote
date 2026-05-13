# Editor Plugin Architecture (v0.1)

> **状态**：✅ 已实施于 OpenSpec change `add-editor-plugin-sdk-v01`（2026-05-13）。本文档的设计决定已在 sibling 仓 [`swarmnote-editor`](https://github.com/swarm-apps/swarmnote-editor) 与本仓 host 中落地。本文档仍保留为权威设计说明，实施细节以 `openspec/changes/add-editor-plugin-sdk-v01/` 的 proposal / design / specs 为准。

## 目的

本文档是 2026-05-12 一次 explore 讨论后收敛出的 `@swarmnote/editor-core` v0.1 插件架构决定。

它是本目录里**最权威**的 plugin 架构来源，统一了原本散落在四份草案中的设计意图，并对 RFC 的若干开放问题给出了明确答案。

| 文档 | 与本文档的关系 |
|---|---|
| `editor-open-source-rfc.md` | 顶层 RFC；其"包结构"与"分阶段实施"两节按本文档对齐 |
| `editor-core-host-boundary.md` | 边界归类参考；interaction 归属问题在本文档闭环 |
| `editor-event-model-draft.md` | 事件演进路径；interaction events 的最终类型见本文档 |
| `editor-extension-points-draft.md` | **superseded**；六类扩展点的抽象描述被 `EditorPluginContext` 的具体接口取代，保留为 thinking trail |

---

## TL;DR

- v0.1 直接走 **Model B 全量**：编辑器同时是 plugin host，并提供完整 plugin SDK
- 内置 `math / table / mermaid / admonition / codeBlock / blockImage / rawHtml / smartPaste` 全部按 `EditorPlugin` 重写，与未来第三方插件共用同一 API
- 这些"会被拆出去"的能力以 **subpath export** 形式留在 `editor-core`（`@swarmnote/editor-core/plugins/math` 等），未来抽成独立 npm 包仅需"换 import 路径"，不破坏 plugin API
- interaction core（slash / wikilink / selection toolbar）作为 **first-party plugin** 留在 `editor-core`，与第三方走同一 plugin API → RFC 开放问题 2 闭环
- `EditorSettings.features` 仅保留**真留在 core 的能力**（`markdownHighlight / markdownDecorations / inlineRendering / search / collaboration`），其他开关删除——是否启用 = 是否在 `plugins: []` 中传入
- Plugin metadata 只锁 `id` + 可选 `version`，其余字段留给 Model C
- `EditorCommandSpec` 精简版：`{ id, title?, description?, icon?, when?, run }`
- 冲突策略：**last-wins + dev warning**

---

## 选型决策与 why

| 决策 | 选择 | Why |
|---|---|---|
| 预留深度 | 深度② 类型 + 空架子 | 锁住 API 形状，但不写 runtime 用不到的代码 |
| 插件化档位 | Model B 全量 | A 太浅（plugin API 没被验证过），C 太重（manifest/sandbox 不是 v0.1 问题）。B 是 Tiptap / BlockNote / Lexical 的成熟形态 |
| 内置功能怎么写 | 全部按 plugin 重写 | "形状被锁定"的唯一可信信号是有真实使用者。math / table / mermaid 同时验证 7-8 类不同的扩展面，覆盖度足够 |
| math/table/mermaid 与 core 的关系 | core 中立，subpath 留位 | core 完全不知道这些能力是什么；host 显式 `plugins: [math(), table(), ...]`。未来抽出去时改 import path 不是破坏性变更 |
| interaction core 归属 | first-party plugin in editor-core | 双重背书：plugin API 既被 math/table（功能型）验证，也被 interaction（交互型）验证。避免"内置交互"与"第三方插件"双 API |
| metadata 锁多少 | 只 id + version | `dependencies` / `settings schema` / `permission` 是 Model C 才需要的字段，v0.1 锁了会形似实虚 |
| 冲突策略 | last-wins + warning | first-wins 会"后装的默默无效"；throw 会卡死内置 plugin 上线流程 |
| `features` 开关 | 删除"被拆"的、保留"留在 core"的 | core 不知道 math 是什么、却仍要为它留 `mathRendering: boolean` 字段是个怪局面 |

---

## Plugin SDK 形状（v0.1 锁定）

```ts
export interface EditorPlugin {
  /** 唯一标识。v0.1 唯一硬约束字段。 */
  id: string;
  /** 可选版本号。仅作展示用，v0.1 不参与解析。 */
  version?: string;
  /** 装载时调用。 */
  setup(ctx: EditorPluginContext): EditorPluginInstance | void;
}

export interface EditorPluginInstance {
  /** 编辑器销毁时调用。setup 内 register 返回的 Disposable 也会被自动释放。 */
  dispose?(): void;
}

export interface EditorPluginContext {
  // ─── stable surface (v0.1 锁定) ────────────────────────────
  registerCommands(specs: EditorCommandSpec[]): Disposable;
  registerCmExtensions(extensions: Extension[]): Disposable;
  registerMarkdownRenderer(rule: MarkdownRenderRule): Disposable;
  host: EditorHostCapabilities;

  // ─── @unstable (v0.1 类型先行，runtime 后续) ─────────────────
  /** @unstable v0.1：slash command 数据源；接口可能在 v0.2 调整。 */
  registerSlashItems?(provider: SlashItemProvider): Disposable;
  /** @unstable v0.1：自定义 trigger 检测；接口可能在 v0.2 调整。 */
  registerTrigger?(spec: EditorTriggerSpec): Disposable;
  /** @unstable v0.1：事件订阅；后续可能合并到统一 lifecycle。 */
  on?(event: EditorEventType, listener: EditorEventListener): Disposable;
}

export interface EditorCommandSpec {
  id: string;
  title?: string;
  description?: string;
  icon?: string;
  /** 该命令在当前上下文是否可用。 */
  when?: (ctx: EditorCommandContext) => boolean;
  run: (ctx: EditorCommandContext) => void | Promise<void>;
}

export interface Disposable {
  dispose(): void;
}
```

### EditorHostCapabilities 聚合协议

```ts
export interface EditorHostCapabilities {
  resolveImage?: (src: string) => string | Promise<string>;
  uploadFile?: UploadFileHandler;
  openLink?: (url: string) => void | Promise<void>;
  // ─── @unstable 后续 ──
  searchNotes?: (query: string) => Promise<NoteSearchItem[]>;
  getSlashItems?: (ctx: SlashItemsContext) => Promise<SlashItem[]>;
}
```

`EditorProps` 上 `imageResolver` / `uploadFile` 仍保留（标 `@deprecated`），同时新增 `host?: EditorHostCapabilities`。两者并存到 v0.2。

---

## EditorEvent 三层分类（v0.1）

```ts
/** Core event：长期稳定契约。 */
export type EditorCoreEvent =
  | EditorChangeEvent
  | EditorSelectionChangeEvent
  | EditorSelectionFormattingChangeEvent
  | EditorFocusEvent | EditorBlurEvent
  | EditorSearchStateChangeEvent
  | EditorCollaborationUpdateEvent
  | EditorLinkOpenEvent;

/** @unstable Interaction event：v0.1 类型先行，trigger runtime 后续。 */
export type EditorInteractionEvent =
  | EditorSlashTriggerChangeEvent
  | EditorWikiLinkTriggerChangeEvent
  | EditorSelectionToolbarChangeEvent;

/** Platform convenience event：含平台耦合字段（DOM 坐标/HTML 字符串等），不保证跨端语义稳定。 */
export type EditorPlatformEvent =
  | EditorTableContextMenuEvent
  | EditorMermaidZoomRequestEvent
  | EditorRemoveEvent;

export type EditorEvent =
  | EditorCoreEvent | EditorInteractionEvent | EditorPlatformEvent;
```

---

## 内置 plugin 清单

| Plugin | 当前所在 | v0.1 后形态 | 备注 |
|---|---|---|---|
| math | `extensions/renderBlockMath.ts` + `markdownMathExtension.ts` | `plugins/math/` first-party plugin | 默认不启用 |
| table | `extensions/renderBlockTables.ts` | `plugins/table/` first-party plugin | 默认不启用；`TableContextMenu` 事件保留为 platform event |
| mermaid | `extensions/renderBlockMermaid.ts` | `plugins/mermaid/` first-party plugin | 默认不启用；`MermaidZoomRequest` 事件保留为 platform event |
| admonition | `extensions/admonition/` | `plugins/admonition/` first-party plugin | 默认不启用 |
| codeBlock | `extensions/renderBlockCode.ts` | `plugins/codeBlock/` first-party plugin | `CodeBlockMode` 配置由 plugin 自己持有 |
| blockImage | `extensions/renderBlockImages.ts` | `plugins/blockImage/` first-party plugin | `imageResolver` 通过 `ctx.host.resolveImage` 注入 |
| rawHtml | `extensions/renderRawHtml.ts` | `plugins/rawHtml/` first-party plugin | DOMPurify 仅在该 plugin 内部依赖 |
| smartPaste | `extensions/smartPasteExtension.ts` | `plugins/smartPaste/` first-party plugin | `uploadFile` 通过 `ctx.host.uploadFile` 注入 |
| **interactions** | _尚未实现_ | `plugins/interactions/{slash,wikilink,selectionToolbar}/` | runtime 后续；v0.1 仅类型层 |

### 留在 core 不拆的能力

- `markdownDecorationExtension` / `markdownHighlightExtension`：Markdown 语义本身
- `inlineRendering/`：编辑器最基本的渲染语义
- `links/` + `LinkOpen` event：链接打开是核心交互
- `markdownFrontMatterExtension`：core 语法
- `editorSettingsExtension` / `lineAwareClipboardExtension`：内部基础设施
- `searchExtension`：搜索面板（structural feature，不是可选插件）
- `collaborationExtension`：Y.Doc / `y-codemirror.next` 绑定

对应 `EditorSettings.features` 保留：`markdownHighlight`、`markdownDecorations`、`inlineRendering`、`search`、`collaboration`。其余 features 字段删除。

---

## Subpath export 策略

```jsonc
// @swarmnote/editor-core/package.json
{
  "exports": {
    ".":                            { /* 主入口，不含任何 plugin */ },
    "./plugins/math":               { /* 仅 math plugin */ },
    "./plugins/table":              { /* 仅 table plugin */ },
    "./plugins/mermaid":            { /* 仅 mermaid plugin */ },
    "./plugins/admonition":         { /* 仅 admonition plugin */ },
    "./plugins/codeBlock":          { /* 仅 codeBlock plugin */ },
    "./plugins/blockImage":         { /* 仅 blockImage plugin */ },
    "./plugins/rawHtml":            { /* 仅 rawHtml plugin */ },
    "./plugins/smartPaste":         { /* 仅 smartPaste plugin */ },
    "./plugins/interactions/slash": { /* @unstable */ },
    "./plugins/interactions/wikilink":         { /* @unstable */ },
    "./plugins/interactions/selectionToolbar": { /* @unstable */ }
  }
}
```

**Host 接入示例**：

```ts
import { createEditor } from '@swarmnote/editor-core';
import { mathPlugin } from '@swarmnote/editor-core/plugins/math';
import { tablePlugin } from '@swarmnote/editor-core/plugins/table';
import { mermaidPlugin } from '@swarmnote/editor-core/plugins/mermaid';

createEditor(parent, {
  initialText,
  settings,
  host: { resolveImage, uploadFile },
  plugins: [
    mathPlugin(),
    tablePlugin(),
    mermaidPlugin({ /* options */ }),
  ],
});
```

未来这些 plugin 抽成独立 npm 包后，host 仅需把 import 路径从 `@swarmnote/editor-core/plugins/math` 改为 `@swarmnote/editor-plugin-math`——这是**纯文本替换**，不是破坏性变更。

---

## 冲突策略

- 多个 plugin 注册同 `id` 的 command：**last-wins**，前一个 spec 被悄悄替换，`console.warn` 提示。
- 多个 plugin 注册 Markdown renderer 抢同一节点：**last-wins**，同样 warn。
- 多个 plugin 注册 CM6 extension：按 `plugins: []` 数组顺序追加，由 CM6 自身的 facet/priority 决定最终行为。

未来 Model C 引入 dependency 解析时再考虑更复杂的冲突处理。

---

## v0.1 实施清单（高层）

1. **类型层先行**：把 `EditorPlugin` / `EditorPluginContext` / `EditorCommandSpec` / `EditorHostCapabilities` 类型加入 `editor-core/src/types.ts`
2. **EditorEvent 分层**：拆 `events.ts` 为三个 sub-union，标 `MermaidZoomRequest` / `TableContextMenu` 为 platform，定义 `SlashTriggerChange` / `WikiLinkTriggerChange` / `SelectionToolbarChange` 类型并标 `@unstable`
3. **Plugin host runtime**：在 `createEditor` 内实现 plugin 加载循环、registry、Disposable 跟踪
4. **内置功能 plugin 化**：把 `extensions/` 下 8 个功能模块按 `EditorPlugin` 接口重写为 `plugins/` 下的 first-party plugin
5. **Subpath export**：更新 `package.json` exports / `tsdown.config.ts` 多入口构建
6. **EditorProps 调整**：新增 `host?: EditorHostCapabilities` 与 `plugins?: EditorPlugin[]`；将 `imageResolver` / `uploadFile` 标 `@deprecated`（v0.2 移除）
7. **features 字段收敛**：删除 `mathRendering` / `mermaidRendering` / `blockImageRendering` / `rawHtmlRendering` / `codeBlockMode` / `smartPaste` / `admonition` 七个字段
8. **SwarmNote host 同步**：`src/components/editor/NoteEditor.tsx` 改用 `plugins: []` 显式启用；设置 UI 中对应 toggle 改为驱动 `plugins[]` 组合而非 `features.*`

---

## 后续路线（均为无破坏性扩展）

| 阶段 | 内容 |
|---|---|
| v0.2 | interaction core runtime：真做 slash / wikilink trigger detection，把 `@unstable` 标签去掉 |
| v0.3 | `editor-react` 包：消费 interaction events，提供桌面端默认 popover/toolbar 组件 |
| v0.4 | `editor-react-native` 包：消费同一套 event/command，提供移动端 sheet/toolbar 组件 |
| v0.5+ | `@swarmnote/editor-plugin-math` 等独立 npm 包，替代 subpath（subpath 同期标 `@deprecated`） |
| v1.0+ | Model C：runtime plugin marketplace，加 manifest / loader / sandbox / settings UI |

每一步都不破坏前一步的 plugin API。

---

## 与原四份文档的关系（再次确认）

- 本文档为 plugin 架构的**最终决定**来源
- `editor-open-source-rfc.md` 的"包结构"与"分阶段实施"按本文档对齐
- `editor-core-host-boundary.md` 的"interaction 归属"问题在本文档闭环
- `editor-event-model-draft.md` 的三层分类已被本文档具象化
- `editor-extension-points-draft.md` 的"六类扩展点"被本文档的 `EditorPluginContext` 取代，文档保留为思考过程

---

## v0.1 BREAKING change 摘要

下列改动相对 v0.0.x 是破坏性，列出供 PR description / CHANGELOG / release notes 引用。

### `@swarmnote/editor-core` — sibling 仓 `swarmnote-editor`

1. **`EditorFeatureToggles` 删除 7 个字段**：`mathRendering` / `mermaidRendering` / `blockImageRendering` / `rawHtmlRendering` / `codeBlockMode` / `smartPaste` / `admonition`。这些能力迁到 plugin，通过 `createEditor(..., { plugins: [...] })` 显式启用。保留：`markdownHighlight` / `markdownDecorations` / `inlineRendering` / `search` / `collaboration`。
2. **主入口不再 re-export 已迁 plugin 工厂**：`createBlockMermaidExtension` / `createRawHtmlExtension` / `createAdmonitionExtension` / `createSmartPasteExtension` / `setTableSourceMode` / `setCodeBlockSourceMode` / `refreshBlockImagesEffect` / `refreshRawHtmlEffect` / `clearMermaidCacheEffect` / `GFM_TYPES` / `OBSIDIAN_TYPES` / `DEFAULT_ADMONITION_TYPE` 等全部仅通过 `@swarmnote/editor-core/plugins/<name>` subpath 暴露。
3. **`EditorProps.imageResolver` / `EditorProps.uploadFile` 标 `@deprecated`**：仍可用并被透明桥接到 `host.resolveImage` / `host.uploadFile`；同时提供 `host.*` 时 `host.*` 优先，触发一次 `console.warn`。
4. **`EditorEvent` 拆分三层 union**：`EditorCoreEvent` / `EditorInteractionEvent`（`@unstable`）/ `EditorPlatformEvent`。聚合 `EditorEvent` 与 v0.0.x 同 shape，向后兼容；按层 import 是新增能力。

### SwarmNote host — 本仓

1. **`preferencesStore` 新增 `enabledPlugins: EditorPluginId[]` + `codeBlockMode: 'inline' | 'auto' | 'toggle'`**：默认 8 个 plugin 全开。带 v0.0.x 旧 `features.*` 键的持久化偏好通过 `migrateLegacyFeatures` 翻译并清除旧键（idempotent）。
2. **设置 UI**：`routes/settings/general.tsx` 新增"编辑器插件"区，含 8 个 plugin Switch + codeBlock 模式 Select。无原 7 toggle 可删（host 历史上从未暴露过）。
3. **`NoteEditor.tsx`**：构造 `plugins[]` 数组传入 `createEditor`；`imageResolver` / `uploadFile` 改走 `host` 对象；plugin 切换需新开文档或重启应用才生效（CM6 扩展集 boot-time freeze）。
