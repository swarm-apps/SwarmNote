# 编辑器

## 架构概览

桌面端编辑器采用 **CodeMirror 6 Live Preview** 方案，通过 `y-codemirror.next` 绑定 Y.Doc 实现协作。

调用链：`React (NoteEditor) → createEditor() → CM6 EditorView → ySync extension ↔ Y.Text`

- 编辑器内核：`@swarmnote/editor-core`（独立仓 [`swarm-apps/swarmnote-editor`](https://github.com/swarm-apps/swarmnote-editor)，pnpm workspace monorepo），桌面端和移动端共享
- 桌面端 React 容器：`src/components/editor/NoteEditor.tsx`
- 文档大纲：`src/components/editor/DocumentOutline.tsx`（基于 `extractHeadings`）

## @swarmnote/editor-core 是外部 sibling 仓 + pnpm link

`@swarmnote/editor-core` 不再是主仓的 submodule，而是独立 repo `swarm-apps/swarmnote-editor`。本地通过 `pnpm.overrides` 把包名解析到 sibling 路径 `../swarmnote-editor/packages/editor-core`，所以 sibling 必须 clone 到与 SwarmNote 同级目录。

### Local development with editor-core

修改编辑器内核代码的流程：

```bash
# 1. 在 sibling 仓启动 watch（每次改源码自动 rebuild dist）
cd ../swarmnote-editor
pnpm dev    # tsdown --watch on @swarmnote/editor-core

# 2. 主仓另开终端跑 dev，dist 变化会自动反映
cd ../SwarmNote
pnpm tauri dev
```

提交流程：

```bash
# 1. 在 sibling 仓内分支提交、push、开 PR
cd ../swarmnote-editor
git checkout -b feat/...
git add . && git commit -m "feat: ..."
git push -u origin feat/...
# → swarm-apps/swarmnote-editor PR

# 2. sibling PR 合并后，主仓的 link target 即指向新版 main，无需主仓任何改动
#    （未来 npm 化后，主仓 package.json 改版本号 + 删除 pnpm.overrides 即可切换）
```

**关键注意**：

- sibling 必须 clone 到 `../swarmnote-editor`（package.json 的 `pnpm.overrides` 锚定该相对路径）
- 主仓不再有 submodule 链路——把"先 push submodule 再 bump 主仓 pointer"那套规则忘掉
- CI 通过 `git clone` 把 sibling 拉到 `${{ github.workspace }}/../swarmnote-editor` 然后 build；主仓 install 自动用 link

**相关文件**：主仓 `package.json::pnpm.overrides`、`.github/workflows/ci.yml`、sibling 仓 `packages/editor-core/`

## Y.Doc 关键约束

### 必须使用 OffsetKind::Utf16

所有 `yrs::Doc` 必须以 `OffsetKind::Utf16` 创建（与前端 JS yjs 一致）。yrs 默认的 `OffsetKind::Bytes` 会导致 CJK 字符 `block_offset` 溢出 panic。

**正确做法**（Rust 端）：
```rust
pub(crate) fn create_doc() -> Doc {
    let opts = yrs::Options {
        offset_kind: OffsetKind::Utf16,
        ..Default::default()
    };
    let doc = Doc::with_options(opts);
    doc.get_or_insert_text(FRAGMENT_NAME);
    doc
}
```

**相关文件**：`src-tauri/src/yjs/mod.rs`

### 文档顶层是 Y.Text，fragment 名为 "document"

前端和后端共享常量：`FRAGMENT_NAME = "document"`。前端 `createEditor` 调 `ydoc.getText("document")`，后端同名。

### 外部 .md 变更通过 text-diff 合并

Rust 端监听文件系统，检测到自己没写过的 .md 改动时，用 `similar::capture_diff_slices(Myers, &old_u16, &new_u16)` 在 UTF-16 code-unit 层面做 diff，再把 DiffOps **反向**应用到 Y.Text。反向顺序保证前段偏移不受后段 insert/delete 影响。

**相关文件**：`src-tauri/src/yjs/mod.rs` → `replace_doc_content`

## CodeMirror 6 关键约束

### 块装饰（block decorations）必须从 StateField 提供

CM6 规定：`Decoration.replace({ block: true })` / `Decoration.widget({ block: true })` **只能**通过 `StateField` 注册，不能用 `ViewPlugin.fromClass(..., { decorations })`。用 ViewPlugin 会抛 "Block decorations may not be specified via plugins"。

**正确做法**：
```typescript
const field = StateField.define<DecorationSet>({
  create: buildDecorations,
  update: (set, tr) => buildDecorations(tr.state),
  provide: (f) => EditorView.decorations.from(f),
});
```

图片、代码块、表格这些跨行 widget 都遵循这个模式。

**相关文件**：`../swarmnote-editor/packages/editor-core/src/extensions/renderBlockImages.ts`、`renderBlockCode.ts`、`renderBlockTables.ts`

### Collaboration 模式初始化时必须 seed 文档

`y-codemirror.next` 的 ySync 扩展只桥接 **observer 事件**。如果挂载时 Y.Text 已经有内容，ySync 不会把这些历史内容回填到 CM6 —— CM6 会显示空文档。

**正确做法**（`createEditor.ts`）：
```typescript
let initialDoc = initialText;
if (collaboration) {
  initialDoc = ydoc.getText(collaboration.fragmentName ?? 'document').toString();
}
```

**相关文件**：`../swarmnote-editor/packages/editor-core/src/createEditor.ts`

### 禁用 EDIT_CONTEXT

Android WebView（移动端场景）上必须禁用，桌面端也一并禁用避免 IME 异常：

```typescript
(EditorView as unknown as { EDIT_CONTEXT: boolean }).EDIT_CONTEXT = false;
```

不要删掉这行。

**相关文件**：`../swarmnote-editor/packages/editor-core/src/createEditor.ts`

### Ctrl+B 快捷键冲突

app 层 `useKeyboardShortcuts` 绑定 `Ctrl+B` 到 toggleSidebar（document 级）；CM6 的 format keymap 绑定 `Mod-b` 到 toggleBold。

**正确做法**：app 层 handler 必须在开头检查 `e.defaultPrevented` 并提前 return。CM6 keymap 处理后会 `preventDefault`，app 层看到即跳过。

```typescript
const handler = (e: KeyboardEvent) => {
  if (e.defaultPrevented) return;  // ← 关键
  // ... 原有逻辑
};
```

**相关文件**：`src/hooks/useKeyboardShortcuts.ts`

## 图片与媒体

### imageResolver 映射 workspace 相对路径

Markdown 里的图片 `![](images/foo.png)` 是 workspace 相对路径。CM6 widget 需要解析成 Tauri `asset://` URL 才能加载。

**正确做法**（`NoteEditor.tsx`）：
```typescript
const imageResolver = useCallback(
  (url: string): string => {
    if (/^(https?|data|blob|asset|tauri):/.test(url)) return url;
    return convertFileSrc(`${wsPath}/${url}`);
  },
  [wsPath],
);
```

**相关文件**：`src/components/editor/NoteEditor.tsx`、`../swarmnote-editor/packages/editor-core/src/extensions/renderBlockImages.ts`

### P2P 媒体到达后刷新 widget

当 P2P 同步到新的媒体文件，后端 emit `yjs:assets-updated`，前端捕获并 dispatch `refreshBlockImagesEffect.of(null)`，让已有 widget 重新 resolveAndAssign。

**相关文件**：`src/components/editor/NoteEditor.tsx` 的 listen 块

### 粘贴/拖放图片通过 saveMedia 命令

`saveMedia(relPath, fileName, bytes)` 返回 workspace 相对路径（通常是 `images/xxxxxx.png`），然后调 `control.execCommand("insertImage", savedRel, fileName)` 插入。

### 图片渲染：block / inline 双路径

`renderBlockImages.ts` 同时处理两类 markdown 图片：

- **Block solo**（独占一行）：使用 `Decoration.replace({block: true})`，widget 自带 `<img>` 居中、选中态边框、`</>` 源码切换图标
- **Inline**（嵌套在标题/强调/引用/列表/表格/同行多图）：`Decoration.replace`（非 block），widget 是行内 `<span><img/></span>`

判定逻辑：检查 lezer `Image` 节点所在行除自身外是否还有非空白文本——是 → inline path。

光标进入图片范围（block 是整行，inline 是 `[node.from, node.to]`）→ 跳过 emission，露出原始 markdown 源码可编辑。

**相关文件**：`../swarmnote-editor/packages/editor-core/src/extensions/renderBlockImages.ts`

### Lezer `Image.parent === Link` = 图片链接

`[![alt](img)](href)` 在 lezer 树里是 `Link > Image`。`getLinkedImageInfo()` 用 `parent.getChild('URL')`/`parent.getChild('LinkTitle')` 提取链接 href + title（不要手写 nextSibling 走，已踩坑过）。命中后用 Link 的整个范围作 `Decoration.replace`，避免外层 `[` 和 `](url)` 露出。widget 加底部右下/右上的外链按钮，Ctrl/Cmd-click 图片或点按钮触发跳转。

**相关文件**：`../swarmnote-editor/packages/editor-core/src/extensions/renderBlockImages.ts::getLinkedImageInfo`

### 跳转链接：走 `editorEventCallback` Facet 派发 LinkOpen

Tauri webview 默认安全策略屏蔽 `window.open`——所有链接打开都走 `editorEventCallback` Facet 派发 `EditorEventType.LinkOpen` 事件，由 host (`NoteEditor.tsx::onEvent`) 调 `@tauri-apps/plugin-opener` 的 `openUrl()`。markdown link 的 ctrl-click、图片链接按钮、HTML `<a>` 内嵌链接 ctrl-click 全部统一这条管道。

**相关文件**：`../swarmnote-editor/packages/editor-core/src/extensions/renderBlockImages.ts::dispatchLinkOpen`、`../swarmnote-editor/packages/editor-core/src/extensions/renderRawHtml.ts::attachLinkInterceptor`、`src/components/editor/NoteEditor.tsx::onEvent` 的 `LinkOpen` 分支

## 原生 HTML 渲染（renderRawHtml.ts）

通过 `dompurify` 渲染 `HTMLBlock`（块级）和自闭合/配对 `HTMLTag`（内联）。覆盖 `<img>`/`<picture>`/`<figure>`/`<details>`/`<u>`/`<s>`/`<a>`/`<span style>` 等所有 DOMPurify 默认放行的标签。

- 配对标签（`<u>...</u>` 等）：通过 lezer `nextSibling` + 深度计数器找匹配 close
- 排除 `<mark>`/`<kbd>`/`<sup>`/`<sub>`——它们由 `inlineRendering/replaceInlineHtml.ts` 的 class-based 系统处理（避免双重渲染）
- `<br>` 包装的 span 必须 `display: inline`（不是 inline-block），否则 `<br>` 的换行被困在 wrapper 内不传播到外层段落
- 表格单元格 `<br>`：`renderInlineMarkdown.ts` 用 PUA 占位符在 escapeHtml 之前抽出 `<br>`，结尾还原——不抽出会被 `&lt;br&gt;` 转义掉

**相关文件**：`../swarmnote-editor/packages/editor-core/src/extensions/renderRawHtml.ts`、`../swarmnote-editor/packages/editor-core/src/utils/renderInlineMarkdown.ts`

## Admonition / Callout

GFM `> [!NOTE]` 与 Obsidian `> **NOTE**` 双语法兼容（`../swarmnote-editor/packages/editor-core/src/extensions/admonition/`）。Obsidian 风格圆角填充盒，Lucide SVG 图标 + label，cursor 进入块内任意位置 → 整块切源码模式。

### 关键约束

1. **正则中第二个 `\s*` 必须用 `[ \t]*`**——`\s` 匹配 `\n` 会让 `(.*)` 跨行捕获 body 当成 customTitle，渲染出 `[icon] > body文本` 的 bug。
2. **必须显式 `background-image: none !important`**——`markdownDecorationExtension` 给所有 blockquote 加 `cm-blockQuote-d0` 类，用 `background-image: linear-gradient(...)` 画金黄色 2px 竖线。这是独立 CSS 属性，`backgroundColor` 不会覆盖它，必须显式清空。
3. **标题 widget 用 `block: true` Decoration.replace**——之前用 inline replace + line decoration 重叠，CodeMirror reconciliation 在「装饰从无到有再到无」循环中可能掉装饰。block widget 自带完整 DOM（含 bg/圆角），独立行，不依赖 line decoration。
4. **行号迭代用 `state.doc.line(n)`**——之前用 `rawText.split(/\n>/)` + cursor offset 推导每行偏移，在 CJK + 行边界 `\n` 上有 off-by-one。直接 `lineAt(node.from).number` 起、`lineAt(node.to).number` 止逐号取行最稳。

**相关文件**：`../swarmnote-editor/packages/editor-core/src/extensions/admonition/admonitionExtension.ts`、`presets.ts`（Lucide SVG icons）

## 大纲提取

`extractHeadings(state)` 使用 CM6 `syntaxTree` + ATX 正则，正确排除 fenced code block 里的伪 heading。

- 使用 `ensureSyntaxTree(state, state.doc.length, 500)` 带 500ms 预算
- 返回 `HeadingItem[]`：`{ level, text, offset }`
- 订阅 `editorChangeTick`（zustand）做 debounce re-parse，默认 300ms

**相关文件**：`../swarmnote-editor/packages/editor-core/src/utils/extractHeadings.ts`、`src/components/editor/DocumentOutline.tsx`

## 修改编辑器包后的构建

主仓通过 `pnpm.overrides` 把 `@swarmnote/editor-core` link 到 `../swarmnote-editor/packages/editor-core/dist/`——所以**必须有 dist 产物**才能跑。日常开发用 watch 模式：

- 在 sibling 仓跑 `pnpm dev`（= `tsdown --watch`），修改 `../swarmnote-editor/packages/editor-core/src/**/*.ts` 后 dist 自动重建
- 主仓 Vite dev server 监听 `node_modules/@swarmnote/editor-core` symlink target 的 dist 变化，HMR 自动 reload
- TypeScript 检查在 sibling 仓跑：`(cd ../swarmnote-editor && pnpm -r typecheck)`

**相关文件**：`../swarmnote-editor/packages/editor-core/package.json`、`../swarmnote-editor/packages/editor-core/tsdown.config.ts`、主仓 `package.json::pnpm.overrides`

## 协作光标 (Awareness) — 生命周期与去重

### `TauriYjsProvider.destroy` 顺序敏感

`setLocalState(null)` 是 awareness 的主动下线 API：调用后 awareness 内部会发一个 `update` 事件，把本地 clientID 加入 `removed` 列表。我们的 listener 接到这个事件后才会调 `invoke("broadcast_awareness", encodeAwarenessUpdate(awareness, [clientID]))`，对端**1 秒内**收到「这个 clientID 下线了」并 GC 掉。

**正确顺序**（`src/lib/TauriYjsProvider.ts::destroy`）：

```ts
this.awareness.setLocalState(null);   // ① listener 仍在 → broadcast 触发
this._destroying = true;               // ② 拦截后续异步 update
this.doc.off("update", ...);
this.awareness.off("update", ...);
this.awareness.destroy();
```

**不要**：先 `off("update")` 再 `setLocalState(null)`。listener 已经被摘掉，下线事件就只是 awareness map 的本地 mutation，不会广播 → 对端等 30s `outdated` 超时才 GC → 用户切 doc / hot-reload 高频时，stale clientID 与新 clientID 短暂并存，对端 PresenceAvatars 看到「同设备 2 个头像」。

崩溃 / 断网路径仍由 30s 超时兜底，不需要额外防护。

### PresenceAvatars 按 `deviceId` 去重

awareness 的 clientID 是 Y.Doc 实例级别的，不是设备级别。**绝对不要尝试自定义 clientID 让多 client 共享**（`new Y.Doc({ clientID })` 这个参数存在但只是测试用） — Yjs CRDT 协议要求 `(clientID, clock)` 全局唯一，复用会让 op 被静默丢弃。

正确做法是在**展示层**按 `user.deviceId` 折叠：

```ts
// PresenceAvatars.tsx::readRemoteUsers
const byDevice = new Map<string, RemoteUser>();
for (const [clientId, raw] of awareness.getStates()) {
  if (clientId === awareness.clientID) continue;
  const u = (raw as { user?: ... }).user;
  if (!u || byDevice.has(u.deviceId)) continue;
  byDevice.set(u.deviceId, { name, platform, deviceId, color });
}
```

React key 用 `u.deviceId` 而不是 clientID — 暂态 stale 期间代表性 clientID 切换时不会触发头像 remount。

**caret 标签去不掉**：y-codemirror.next 按 clientID 渲染 `.cm-ySelection*` decorations，无法干预。但只要 destroy 顺序是对的，正常使用路径下 stale clientID 不会积累，caret 自动只有 1 个。偶发网络丢包导致的 1-30s 暂态多 caret 接受为已知 limit。

**相关文件**：`src/lib/TauriYjsProvider.ts`、`src/components/editor/PresenceAvatars.tsx`

## 桌面端右键菜单

### 直接用 Radix `ContextMenuTrigger asChild`，**不**走 CM6 `domEventHandlers`

shadcn 的 `<ContextMenu>` 已经处理了 `oncontextmenu` 拦截、Portal 位置、焦点管理与 ESC 关闭。CM6 的 `EditorView.domEventHandlers({ contextmenu })` + 自管 React state 是重复发明轮子。

**正确做法**：
```tsx
<ContextMenu onOpenChange={syncFormatting}>
  <ContextMenuTrigger asChild>
    <div ref={containerRef} />  {/* CM6 mounts here */}
  </ContextMenuTrigger>
  <ContextMenuContent>...</ContextMenuContent>
</ContextMenu>
```

`asChild` 走 Radix `Slot` 模式，把 trigger 的 `onContextMenu` 等 props 合到 `<div>` 上，并合并 ref —— `containerRef` 仍指向那个 div，CM6 mount 不受影响。

**相关文件**：`src/components/editor/EditorContextMenu.tsx`、`src/components/editor/NoteEditor.tsx`

### 选区状态用 `onOpenChange` 同步而**不**订阅事件

桌面端右键菜单是按需弹出，菜单关闭时不需要维护订阅。在菜单 `onOpenChange(true)` 瞬间一次性调 `control.getSelectionFormatting()` + 读 `view.state.selection.main.empty`，写入 React state 冻结住——子菜单悬停期间状态不会跟光标移动飘。

订阅 `EditorEventType.SelectionFormattingChange` 是移动端工具栏的方案（常驻 UI），桌面端**不要**这么做：菜单 99% 时间不可见时仍跑订阅是浪费，且引入"菜单先打开后选区变了"的同步窗口边界。

**相关文件**：`src/components/editor/EditorContextMenu.tsx::handleOpenChange`

### 不要嵌套 ContextMenu —— 最内层赢，外层项不可达

`EditorPane` 之前在编辑区外层包过一个只有"可读行宽"一项的 `<ContextMenu>`。如果 NoteEditor 内部再加一层 `<ContextMenu>`，Radix 会让最内层的 trigger 赢，外层那一项**永远点不到**。

**正确做法**：把视图设置项合并到内层菜单的"视图"分组里，外层 `EditorPane` 只剩 `<main><div>{NoteEditor}</div><StatusBar /></main>` 的纯布局结构。

**相关文件**：`src/components/layout/EditorPane.tsx`

### 插入图片：隐藏 `<input type="file">` 复用 `handleFiles`

仓库**没有** `@tauri-apps/plugin-fs`。Tauri Dialog `open()` 只返回路径，没有读字节能力，要真的要走 plugin-fs 路径需要新装一整套（npm + cargo + capability + lib.rs 注册）。

最干净的做法：在 `NoteEditor` 渲染一个隐藏的 `<input type="file" accept="image/*" hidden>`，菜单"插入图片"项只做 `fileInputRef.current?.click()`。`onChange` 拿到 `FileList` 后调用现有 `handleFiles`（drag/drop + paste 共用），完整复用 `arrayBuffer → saveMedia → execCommand('insertImage')` 链路。

`onChange` 必须 `e.target.value = ""` 复位，否则连续选同一张图不会再次触发。

**相关文件**：`src/components/editor/NoteEditor.tsx::handleInsertImageFromMenu / handleFileInputChange`

### `toggleHighlight` / `toggleBlockquote` 在编辑器子仓 — 跨仓提交序

两个命令现位于 `../swarmnote-editor/packages/editor-core/src/editorCommands/markdown.ts`（highlight 与 strike 同 helper）和 `../swarmnote-editor/packages/editor-core/src/editorCommands/blockquote.ts`（独立文件，行级前缀切换，模式照搬 `list.ts`）。`@swarmnote/editor-core` 是独立仓，改动走 sibling 仓 PR；本地 link 模式下主仓自动反映最新 dist，不需要主仓任何 commit。

新键位：
- `Mod-Shift-=` → `toggleHighlight`
- `Mod-Shift-q` → `toggleBlockquote`

`Mod-Shift-h` 已被 `cycleHeading` 占用，不要重用。

**相关文件**：`../swarmnote-editor/packages/editor-core/src/editorCommands/markdown.ts`、`../swarmnote-editor/packages/editor-core/src/editorCommands/blockquote.ts`、`../swarmnote-editor/packages/editor-core/src/createEditor.ts::buildFormatKeymap`

## Live Preview 装饰层

### 双层装饰机制

CM6 的 markdown live preview 由两套互补扩展实现：

| 扩展 | 职责 | reveal 行为 |
|---|---|---|
| `markdownDecorationExtension` | 永久样式装饰（行/区间挂 CSS class） | 不分 reveal 状态 |
| `makeInlineReplaceExtension`（`inlineRendering/`） | conceal 标记字符 + 给内容挂 class | 按 spec 的 `getRevealStrategy` 决定 |

`makeInlineReplaceExtension` 在 `shouldReveal` 返回 `true` 时**直接 return 不挂任何装饰**——这是关键：reveal 状态下用户看到纯 markdown 源码，离开后看到装饰版本。所以"光标进入 strikethrough 时不带 line-through"是天然行为，不需要单独处理。

### Reveal 策略语义

```ts
type RevealStrategy = 'line' | 'active' | 'select';
```

- `'line'`：光标所在行内的所有同节点装饰均 reveal（HeaderMark / CodeMark / QuoteMark 用此）
- `'active'`：光标 head 落在节点 `[from, to]` 内才 reveal（EmphasisMark / StrikethroughMark / HighlightMarker / LinkMark / URL / Emphasis / StrongEmphasis / Strikethrough 用此）
- `'select'`：选区 `[from, to)` 与节点相交才 reveal（Link / URL 在 `addFormattingClasses` 中的 link 装饰用此）

### lezer markdown 节点名速查

| 节点 | 含义 | 典型用法 |
|---|---|---|
| `StrongEmphasis` | `**bold**` 整段（含 marker） | 加粗装饰 |
| `Emphasis` | `*italic*` 整段（含 marker） | 斜体装饰 |
| `EmphasisMark` | 单个 `*` / `**` 字符 | conceal |
| `Strikethrough` | `~~xxx~~` 整段 | 删除线装饰 |
| `StrikethroughMark` | 单个 `~~` | conceal |
| `Highlight` | `==xxx==` 整段（GFM 扩展） | 黄色背景 |
| `HighlightMarker` | `==` | conceal |
| `Link` | `[text](url)` 整段 | 链接样式 |
| `LinkMark` | `[` `]` `(` `)` 单字符 | conceal |
| `URL` | `(...)` 内的 url 文本 | conceal（光标外完全隐藏） |
| `HeaderMark` | `#` `##` ... | mark 装饰 + opacity 0.35 |
| `QuoteMark` | `>` | mark 装饰 + opacity 0.45 |
| `CodeMark` | `` ` `` `` ``` `` | conceal |

### 添加新装饰的 checklist

1. **选哪个扩展**：始终装饰 → `markdownDecorationExtension`；需要 conceal/reveal → `addFormattingClasses` / 新增独立 spec。
2. **加节点名**：在 `nodeNames` 数组追加。
3. **写 createDecoration 分支**：返回 `Decoration.mark({ class })` 或 `Decoration.line({ attributes: { class } })`。
4. **声明 reveal 策略**：`getRevealStrategy(node)` 返回 `'line' | 'active' | 'select'`，与既有同类节点对齐。
5. **加 CSS**：`addFormattingClasses` 的样式放 `formattingClassesTheme`，`markdownDecorationExtension` 的样式放 `markdownTheme`，与 `createTheme.ts` 的 `c.link` / `c.foreground` 等主题变量绑定。

### Selection 与 activeLine 的设计反转

CM6 默认配置 + 暖色品牌系统会导致 selection 与 activeLine 撞色：

- `lightDefaults.activeLine` 原 `hsl(40, 18%, 96%)`（暖米色高亮当前行）+ `selection` 原 alpha `0.20` → 拖选时几乎看不出选中范围
- 修复方案：参照 Obsidian Live Preview 设计——**activeLine 完全透明，selection 独享主题色**
- 现在的值：`activeLine: 'transparent'`、`.cm-activeLineGutter: { backgroundColor: 'transparent' }`、`selection alpha 0.30 (light) / 0.35 (dark)`
- 取舍：用户失去"光标在哪行"的视觉锚点，依赖 caret 自身。Live Preview 模式下用户感知主要靠光标本身，可接受

**相关文件**：`../swarmnote-editor/packages/editor-core/src/extensions/inlineRendering/addFormattingClasses.ts`、`../swarmnote-editor/packages/editor-core/src/extensions/inlineRendering/replaceFormatCharacters.ts`、`../swarmnote-editor/packages/editor-core/src/extensions/markdownDecorationExtension.ts`、`../swarmnote-editor/packages/editor-core/src/theme/createTheme.ts`
