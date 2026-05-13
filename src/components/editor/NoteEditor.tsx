import { useLingui } from "@lingui/react/macro";
import {
  createEditor,
  DEFAULT_SETTINGS,
  type EditorControl,
  EditorEventType,
  type EditorPlugin,
  type EditorSettings,
  type SelectionToolbarMatch,
  type SlashTriggerMatch,
  type WikilinkTriggerMatch,
} from "@swarmnote/editor-core";
import { admonitionPlugin } from "@swarmnote/editor-core/plugins/admonition";
import {
  blockImagePlugin,
  refreshBlockImagesEffect,
} from "@swarmnote/editor-core/plugins/blockImage";
import { codeBlockPlugin } from "@swarmnote/editor-core/plugins/codeBlock";
import { selectionToolbarPlugin } from "@swarmnote/editor-core/plugins/interactions/selectionToolbar";
import { slashCommandPlugin } from "@swarmnote/editor-core/plugins/interactions/slash";
import { wikilinkPlugin } from "@swarmnote/editor-core/plugins/interactions/wikilink";
import { mathPlugin } from "@swarmnote/editor-core/plugins/math";
import { mermaidPlugin } from "@swarmnote/editor-core/plugins/mermaid";
import { rawHtmlPlugin } from "@swarmnote/editor-core/plugins/rawHtml";
import { smartPastePlugin } from "@swarmnote/editor-core/plugins/smartPaste";
import { tablePlugin } from "@swarmnote/editor-core/plugins/table";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { type ChangeEvent, useCallback, useEffect, useRef, useState } from "react";
import * as Y from "yjs";
import { openYDoc, reloadYDocConfirmed, saveMedia } from "@/commands/document";
import { EditorContextMenu } from "@/components/editor/EditorContextMenu";
import {
  bumpSlashMru,
  getSlashItems,
  getWikilinkItems,
  resolveInternalLink,
} from "@/components/editor/interactionProviders";
import { SelectionToolbar } from "@/components/editor/SelectionToolbar";
import { SlashCommandPopover } from "@/components/editor/SlashCommandPopover";
import {
  initialTableContextMenuState,
  TableContextMenu,
  type TableContextMenuState,
} from "@/components/editor/TableContextMenu";
import { WikilinkPopover } from "@/components/editor/WikilinkPopover";
import { colorForDevice } from "@/lib/awareness-color";
import { TauriYjsProvider } from "@/lib/TauriYjsProvider";
import { useEditorStore } from "@/stores/editorStore";
import { type EditorPluginId, usePreferencesStore } from "@/stores/preferencesStore";
import { useUIStore } from "@/stores/uiStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

/**
 * 根据 preferencesStore 当前快照构造 `plugins[]` 数组。
 *
 * 由于 CM6 扩展集合在 `EditorState.create` 时一次性 freeze，本函数仅在
 * editor 挂载时读取 snapshot —— 用户切换 plugin 启用状态后需要新开
 * 编辑器（key remount）才能反映。
 */
function buildEditorPlugins(
  enabledPluginIds: readonly EditorPluginId[],
  codeBlockMode: "inline" | "auto" | "toggle",
): EditorPlugin[] {
  const enabled = new Set<EditorPluginId>(enabledPluginIds);
  const plugins: EditorPlugin[] = [];
  if (enabled.has("math")) plugins.push(mathPlugin());
  if (enabled.has("table")) plugins.push(tablePlugin());
  if (enabled.has("mermaid")) plugins.push(mermaidPlugin());
  if (enabled.has("admonition")) plugins.push(admonitionPlugin());
  if (enabled.has("codeBlock")) plugins.push(codeBlockPlugin({ mode: codeBlockMode }));
  if (enabled.has("blockImage")) plugins.push(blockImagePlugin());
  if (enabled.has("rawHtml")) plugins.push(rawHtmlPlugin());
  if (enabled.has("smartPaste")) plugins.push(smartPastePlugin());
  if (enabled.has("slash"))
    plugins.push(
      slashCommandPlugin({
        onItemConfirmed: (id) => bumpSlashMru(id),
      }),
    );
  if (enabled.has("wikilink")) plugins.push(wikilinkPlugin());
  if (enabled.has("selectionToolbar")) plugins.push(selectionToolbarPlugin());
  return plugins;
}

interface YjsContext {
  ydoc: Y.Doc;
  provider: TauriYjsProvider;
}

/**
 * Outer component: initializes Y.Doc from Rust backend, then renders the
 * inner editor once ready. Remounted per document via `key={currentDocId}`
 * in EditorPane.
 */
export function NoteEditor() {
  const { t } = useLingui();
  const docId = useEditorStore((s) => s.currentDocId);
  const relPath = useEditorStore((s) => s.relPath);
  const workspace = useWorkspaceStore((s) => s.workspace);

  const [yjsCtx, setYjsCtx] = useState<YjsContext | null>(null);

  useEffect(() => {
    if (!workspace || !docId) return;
    let cancelled = false;

    const wsId = workspace.id;

    async function init() {
      const result = await openYDoc(relPath, wsId);

      if (cancelled) return;

      // Store the stable UUID for subsequent IPC calls
      useEditorStore.getState().setDocUuid(result.doc_uuid);

      const ydoc = new Y.Doc();
      Y.applyUpdate(ydoc, new Uint8Array(result.yjs_state));

      const provider = new TauriYjsProvider(ydoc, result.doc_uuid);
      setYjsCtx({ ydoc, provider });
    }

    init();

    return () => {
      cancelled = true;
      setYjsCtx((prev) => {
        if (prev) {
          prev.provider.destroy();
          prev.ydoc.destroy();
        }
        return null;
      });
    };
  }, [docId, relPath, workspace]);

  if (!yjsCtx) {
    return (
      <div className="mx-auto flex w-full max-w-3xl flex-col gap-4">
        <div className="h-8" />
        <div className="animate-pulse text-sm text-muted-foreground">{t`加载中...`}</div>
      </div>
    );
  }

  return <NoteEditorInner ydoc={yjsCtx.ydoc} provider={yjsCtx.provider} />;
}

/**
 * Inner component: mounts `@swarmnote/editor-core` (CM6) bound to the given
 * Y.Doc via `y-codemirror.next`, wires up Tauri event bridges for flush,
 * external updates, external conflict, and asset refresh.
 *
 * The TauriYjsProvider is attached to the Y.Doc in the outer component and
 * is destroyed there on unmount — the inner component doesn't touch it.
 */
function NoteEditorInner({ ydoc, provider }: { ydoc: Y.Doc; provider: TauriYjsProvider }) {
  const { t } = useLingui();
  const resolvedTheme = useUIStore((s) => s.resolvedTheme);
  const readableLineLength = useUIStore((s) => s.readableLineLength);
  const markDirty = useEditorStore((s) => s.markDirty);
  const setCharCount = useEditorStore((s) => s.setCharCount);
  const docUuid = useEditorStore((s) => s.docUuid);

  // Stable refs for callbacks accessed in long-lived handlers
  const markDirtyRef = useRef(markDirty);
  markDirtyRef.current = markDirty;
  const setCharCountRef = useRef(setCharCount);
  setCharCountRef.current = setCharCount;

  const wsPath = useWorkspaceStore.getState().workspace?.path ?? "";

  const containerRef = useRef<HTMLDivElement>(null);
  const controlRef = useRef<EditorControl | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Reactive control instance for the right-click menu (re-renders when the
  // editor mounts/unmounts).
  const editorControl = useEditorStore((s) => s.editorControl);

  // Table widget cell right-click menu — driven by `EditorTableContextMenu`
  // events; the widget itself never paints menu DOM.
  const [tableMenuState, setTableMenuState] = useState<TableContextMenuState>(
    initialTableContextMenuState,
  );

  // Slash command popover state — driven by `SlashTriggerChange` events.
  const [slashMatch, setSlashMatch] = useState<SlashTriggerMatch | null>(null);
  // Wikilink popover state — driven by `WikilinkTriggerChange` events.
  const [wikilinkMatch, setWikilinkMatch] = useState<WikilinkTriggerMatch | null>(null);
  // Selection toolbar state — driven by `SelectionToolbarChange` events.
  const [selectionToolbarMatch, setSelectionToolbarMatch] = useState<SelectionToolbarMatch | null>(
    null,
  );
  const handleTableMenuOpenChange = useCallback((open: boolean) => {
    setTableMenuState((prev) => ({ ...prev, open }));
  }, []);

  // Shared "user supplied a File → save to workspace → insert into doc" path
  // used by drag/drop, clipboard paste, and the context menu's "插入图片" item.
  const handleFiles = useCallback(async (files: FileList | File[]) => {
    const control = controlRef.current;
    if (!control) return;
    const rel = useEditorStore.getState().relPath;
    if (!rel) return;
    for (const file of Array.from(files)) {
      if (!file.type.startsWith("image/")) continue;
      const buffer = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buffer));
      const savedRel = await saveMedia(rel, file.name, bytes);
      control.execCommand("insertImage", savedRel, file.name);
    }
  }, []);

  const handleInsertImageFromMenu = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileInputChange = useCallback(
    (e: ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files;
      if (files && files.length > 0) {
        void handleFiles(files);
      }
      // Reset so picking the same file twice in a row still fires onChange.
      e.target.value = "";
    },
    [handleFiles],
  );

  // Image resolver: map workspace-relative paths to Tauri asset:// URLs.
  const imageResolver = useCallback(
    (url: string): string => {
      if (
        url.startsWith("http://") ||
        url.startsWith("https://") ||
        url.startsWith("data:") ||
        url.startsWith("blob:") ||
        url.startsWith("asset://") ||
        url.startsWith("tauri://")
      ) {
        return url;
      }
      return convertFileSrc(`${wsPath}/${url}`);
    },
    [wsPath],
  );

  // Upload handler: save a single dropped/pasted file to workspace media,
  // returning the rel-path + alt for smartPaste plugin to insert as Markdown.
  const uploadFile = useCallback(async (file: File): Promise<{ url: string; alt?: string }> => {
    const rel = useEditorStore.getState().relPath;
    if (!rel) throw new Error("no relPath");
    const buffer = await file.arrayBuffer();
    const bytes = Array.from(new Uint8Array(buffer));
    const savedRel = await saveMedia(rel, file.name, bytes);
    return { url: savedRel, alt: file.name };
  }, []);

  // Mount the CM6 editor once per Y.Doc (collaboration mode).
  // biome-ignore lint/correctness/useExhaustiveDependencies: ydoc drives the editor lifecycle; resolvedTheme / host capabilities / plugins are read from store snapshot at mount and don't trigger remount
  useEffect(() => {
    const parent = containerRef.current;
    if (!parent) return;

    const initialSettings: EditorSettings = {
      ...DEFAULT_SETTINGS,
      theme: {
        ...DEFAULT_SETTINGS.theme,
        appearance: resolvedTheme === "dark" ? "dark" : "light",
      },
    };

    // Read plugin enablement snapshot at mount. CM6 extensions are frozen at
    // EditorState.create() time, so plugin toggles only take effect on the
    // next editor mount (document switch or app reload).
    const prefs = usePreferencesStore.getState();
    const plugins = buildEditorPlugins(prefs.enabledPlugins, prefs.codeBlockMode);

    const control = createEditor(parent, {
      initialText: "",
      settings: initialSettings,
      collaboration: {
        ydoc,
        fragmentName: "document",
        awareness: provider.awareness,
      },
      host: {
        resolveImage: imageResolver,
        uploadFile,
        openLink: (url) => {
          // Resolve wikilink target / .md relative path → load note
          const internal = resolveInternalLink(url);
          if (internal) {
            useEditorStore.getState().loadDocument(internal.id, internal.title, internal.id);
            return;
          }
          // Fall back to system browser for external URLs
          openUrl(url).catch(() => {
            // URL may be malformed or blocked — silent.
          });
        },
        getSlashItems,
        getWikilinkItems,
      },
      plugins,
      autofocus: true,
      onEvent: (event) => {
        if (event.kind === EditorEventType.Change) {
          useEditorStore.getState().bumpEditorChangeTick();
        } else if (event.kind === EditorEventType.TableContextMenu) {
          setTableMenuState({
            open: true,
            clientX: event.clientX,
            clientY: event.clientY,
            rowIdx: event.rowIdx,
            colIdx: event.colIdx,
            alignment: event.alignment,
            rowCount: event.rowCount,
            colCount: event.colCount,
            actions: event.actions,
          });
        } else if (event.kind === EditorEventType.LinkOpen) {
          // Ctrl/Cmd-click on markdown link / wikilink / image link routes
          // here. First try to resolve as an internal note (wikilink target
          // or .md path); fall back to system browser for external URLs.
          const internal = resolveInternalLink(event.url);
          if (internal) {
            useEditorStore.getState().loadDocument(internal.id, internal.title, internal.id);
          } else {
            openUrl(event.url).catch(() => {
              // URL may be malformed or blocked — silent.
            });
          }
        } else if (event.kind === EditorEventType.SlashTriggerChange) {
          setSlashMatch(event.match.active ? event.match : null);
        } else if (event.kind === EditorEventType.WikilinkTriggerChange) {
          setWikilinkMatch(event.match.active ? event.match : null);
        } else if (event.kind === EditorEventType.SelectionToolbarChange) {
          setSelectionToolbarMatch(event.match.active ? event.match : null);
        }
      },
    });

    // Seed awareness with our identity. y-codemirror.next reads `user.name`
    // and `user.color` to render remote caret labels. Color is derived from
    // peer_id so it stays stable across sessions.
    invoke<{ peer_id: string; device_name: string }>("get_device_info")
      .then((info) => {
        provider.awareness.setLocalStateField("user", {
          name: info.device_name,
          platform: "desktop",
          deviceId: info.peer_id,
          color: colorForDevice(info.peer_id),
        });
      })
      .catch((err) => {
        console.warn("Awareness setLocalStateField skipped:", err);
      });

    controlRef.current = control;
    useEditorStore.getState().setEditorControl(control);
    useEditorStore.getState().setAwareness(provider.awareness);

    return () => {
      controlRef.current = null;
      useEditorStore.getState().setEditorControl(null);
      useEditorStore.getState().setAwareness(null);
      control.destroy();
    };
  }, [ydoc, provider]);

  // Reactively apply theme changes to the live editor.
  useEffect(() => {
    const control = controlRef.current;
    if (!control) return;
    control.updateSettings({
      theme: { appearance: resolvedTheme === "dark" ? "dark" : "light" },
    });
  }, [resolvedTheme]);

  // Track dirty state + debounced char count from Y.Doc updates.
  useEffect(() => {
    let charCountTimer: ReturnType<typeof setTimeout> | null = null;
    const handler = (_update: Uint8Array, origin: unknown) => {
      if (origin !== "remote") {
        markDirtyRef.current();
      }
      if (charCountTimer) clearTimeout(charCountTimer);
      charCountTimer = setTimeout(() => {
        const control = controlRef.current;
        if (control) {
          setCharCountRef.current(control.view.state.doc.length);
        }
      }, 300);
    };
    ydoc.on("update", handler);
    handler(new Uint8Array(), null);
    return () => {
      ydoc.off("update", handler);
      if (charCountTimer) clearTimeout(charCountTimer);
    };
  }, [ydoc]);

  // Flush event from Rust writeback (clears dirty flag).
  useEffect(() => {
    if (!docUuid) return;
    const uuid = docUuid;

    let cancelled = false;
    const unlistenPromise = listen<{ docUuid: string }>("yjs:flushed", (event) => {
      if (!cancelled && event.payload.docUuid === uuid) {
        useEditorStore.getState().markFlushed(new Date());
      }
    });

    return () => {
      cancelled = true;
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [docUuid]);

  // Awareness: remote caret/presence updates from peers.
  useEffect(() => {
    if (!docUuid) return;
    const uuid = docUuid;
    let cancelled = false;
    const unlistenPromise = listen<{ docUuid: string; update: number[] }>(
      "yjs:awareness-update",
      (event) => {
        if (cancelled || event.payload.docUuid !== uuid) return;
        provider.applyRemoteAwarenessUpdate(new Uint8Array(event.payload.update));
      },
    );
    return () => {
      cancelled = true;
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [docUuid, provider]);

  // External .md change silently applied as a yjs update (not dirty path).
  useEffect(() => {
    if (!docUuid) return;
    const uuid = docUuid;

    let cancelled = false;
    const unlistenPromise = listen<{ docUuid: string; update: number[] }>(
      "yjs:external-update",
      (event) => {
        if (!cancelled && event.payload.docUuid === uuid) {
          Y.applyUpdate(ydoc, new Uint8Array(event.payload.update), "remote");
        }
      },
    );

    return () => {
      cancelled = true;
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [docUuid, ydoc]);

  // External change conflict: prompt user before reloading.
  useEffect(() => {
    if (!docUuid) return;
    const uuid = docUuid;

    let cancelled = false;
    const unlistenPromise = listen<{ docUuid: string; relPath: string }>(
      "yjs:external-conflict",
      async (event) => {
        if (cancelled || event.payload.docUuid !== uuid) return;
        const confirmed = await confirm(
          t`"${event.payload.relPath}" 已被外部修改。是否重新加载？当前未保存的编辑将丢失。`,
          { title: t`文件已修改`, kind: "warning" },
        );
        if (confirmed && !cancelled) {
          await reloadYDocConfirmed(event.payload.docUuid);
        }
      },
    );

    return () => {
      cancelled = true;
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [docUuid, t]);

  // Asset refresh: rebuild image widgets when media files are synced from P2P.
  useEffect(() => {
    let cancelled = false;
    const unlistenPromise = listen("yjs:assets-updated", () => {
      if (cancelled) return;
      const control = controlRef.current;
      if (!control) return;
      control.view.dispatch({ effects: refreshBlockImagesEffect.of(null) });
    });

    return () => {
      cancelled = true;
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  // Drag/drop + clipboard paste for image uploads.
  useEffect(() => {
    const parent = containerRef.current;
    if (!parent) return;

    const onDrop = (e: DragEvent) => {
      if (!e.dataTransfer?.files || e.dataTransfer.files.length === 0) return;
      const hasImage = Array.from(e.dataTransfer.files).some((f) => f.type.startsWith("image/"));
      if (!hasImage) return;
      e.preventDefault();
      void handleFiles(e.dataTransfer.files);
    };

    const onPaste = (e: ClipboardEvent) => {
      if (!e.clipboardData?.files || e.clipboardData.files.length === 0) return;
      const hasImage = Array.from(e.clipboardData.files).some((f) => f.type.startsWith("image/"));
      if (!hasImage) return;
      e.preventDefault();
      void handleFiles(e.clipboardData.files);
    };

    parent.addEventListener("drop", onDrop);
    parent.addEventListener("paste", onPaste);
    return () => {
      parent.removeEventListener("drop", onDrop);
      parent.removeEventListener("paste", onPaste);
    };
  }, [handleFiles]);

  return (
    <>
      <EditorContextMenu control={editorControl} onInsertImage={handleInsertImageFromMenu}>
        <div
          ref={containerRef}
          className={`h-full w-full select-text ${
            readableLineLength ? "[&_.cm-scroller]:!px-[max(40px,calc((100%-56rem)/2))]" : ""
          }`}
        />
      </EditorContextMenu>
      <input
        ref={fileInputRef}
        type="file"
        accept="image/png,image/jpeg,image/gif,image/webp,image/svg+xml"
        hidden
        onChange={handleFileInputChange}
      />
      <TableContextMenu state={tableMenuState} onOpenChange={handleTableMenuOpenChange} />
      <SlashCommandPopover match={slashMatch} control={editorControl} />
      <WikilinkPopover match={wikilinkMatch} control={editorControl} />
      <SelectionToolbar match={selectionToolbarMatch} control={editorControl} />
    </>
  );
}
