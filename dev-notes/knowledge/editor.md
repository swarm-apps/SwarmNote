# 编辑器

## 架构概览

桌面端编辑器采用 **CodeMirror 6 Live Preview** 方案，通过 `y-codemirror.next` 绑定 Y.Doc 实现协作。

调用链：`React (NoteEditor) → createEditor() → CM6 EditorView → ySync extension ↔ Y.Text`

- 编辑器核心：`packages/editor/`（submodule：`yexiyue/swarmnote-editor`），桌面端和移动端共享
- 桌面端 React 容器：`src/components/editor/NoteEditor.tsx`
- 文档大纲：`src/components/editor/DocumentOutline.tsx`（基于 `extractHeadings`）

## @swarmnote/editor 是 git submodule

`packages/editor/` 有独立 Git 仓库。修改编辑器核心代码的流程：

```bash
# 1. 在 submodule 内修改、提交、推送
cd packages/editor
git add .
git commit -m "feat: ..."
git push origin main

# 2. 回到主仓库，更新 submodule 引用
cd ../..
git add packages/editor
git commit -m "chore: update editor submodule"
```

**关键注意**：
- 主仓库只记录 submodule 指向的 commit hash。**子模块 push 必须先于主仓库 push**，否则远端 submodule 指针指向不存在的 commit
- 不要在主仓库层面直接改 `packages/editor/` 内的文件然后在主仓库提交——那样不会推到 submodule 仓库
- 拉取最新 submodule：`git submodule update --remote packages/editor`

**相关文件**：`packages/editor/`（submodule）、`.gitmodules`

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

**相关文件**：`packages/editor/src/extensions/renderBlockImages.ts`、`renderBlockCode.ts`、`renderBlockTables.ts`

### Collaboration 模式初始化时必须 seed 文档

`y-codemirror.next` 的 ySync 扩展只桥接 **observer 事件**。如果挂载时 Y.Text 已经有内容，ySync 不会把这些历史内容回填到 CM6 —— CM6 会显示空文档。

**正确做法**（`createEditor.ts`）：
```typescript
let initialDoc = initialText;
if (collaboration) {
  initialDoc = ydoc.getText(collaboration.fragmentName ?? 'document').toString();
}
```

**相关文件**：`packages/editor/src/createEditor.ts`

### 禁用 EDIT_CONTEXT

Android WebView（移动端场景）上必须禁用，桌面端也一并禁用避免 IME 异常：

```typescript
(EditorView as unknown as { EDIT_CONTEXT: boolean }).EDIT_CONTEXT = false;
```

不要删掉这行。

**相关文件**：`packages/editor/src/createEditor.ts`

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

**相关文件**：`src/components/editor/NoteEditor.tsx`、`packages/editor/src/extensions/renderBlockImages.ts`

### P2P 媒体到达后刷新 widget

当 P2P 同步到新的媒体文件，后端 emit `yjs:assets-updated`，前端捕获并 dispatch `refreshBlockImagesEffect.of(null)`，让已有 widget 重新 resolveAndAssign。

**相关文件**：`src/components/editor/NoteEditor.tsx` 的 listen 块

### 粘贴/拖放图片通过 saveMedia 命令

`saveMedia(relPath, fileName, bytes)` 返回 workspace 相对路径（通常是 `images/xxxxxx.png`），然后调 `control.execCommand("insertImage", savedRel, fileName)` 插入。

## 大纲提取

`extractHeadings(state)` 使用 CM6 `syntaxTree` + ATX 正则，正确排除 fenced code block 里的伪 heading。

- 使用 `ensureSyntaxTree(state, state.doc.length, 500)` 带 500ms 预算
- 返回 `HeadingItem[]`：`{ level, text, offset }`
- 订阅 `editorChangeTick`（zustand）做 debounce re-parse，默认 300ms

**相关文件**：`packages/editor/src/utils/extractHeadings.ts`、`src/components/editor/DocumentOutline.tsx`

## 修改编辑器包后的构建

桌面端通过 pnpm workspace 直接 symlink `packages/editor/`，**不需要手动 build bundle**（和移动端的 WebView 方案不同）。但：

- 修改 `packages/editor/src/**/*.ts` 后，TypeScript 检查跑 `pnpm --filter @swarmnote/editor typecheck`
- Vite dev server 能直接 hot-reload。不需要 rebuild

**相关文件**：`packages/editor/package.json`、`pnpm-workspace.yaml`

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
