import { useLingui } from "@lingui/react/macro";
import {
  createEditor,
  DEFAULT_SETTINGS,
  type EditorControl,
  EditorEventType,
  type EditorSettings,
  refreshBlockImagesEffect,
} from "@swarmnote/editor";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import * as Y from "yjs";
import { openYDoc, reloadYDocConfirmed, saveMedia } from "@/commands/document";
import { TauriYjsProvider } from "@/lib/TauriYjsProvider";
import { useEditorStore } from "@/stores/editorStore";
import { useUIStore } from "@/stores/uiStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

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

  return <NoteEditorInner ydoc={yjsCtx.ydoc} />;
}

/**
 * Inner component: mounts `@swarmnote/editor` (CM6) bound to the given
 * Y.Doc via `y-codemirror.next`, wires up Tauri event bridges for flush,
 * external updates, external conflict, and asset refresh.
 *
 * The TauriYjsProvider is attached to the Y.Doc in the outer component and
 * is destroyed there on unmount — the inner component doesn't touch it.
 */
function NoteEditorInner({ ydoc }: { ydoc: Y.Doc }) {
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

  // Mount the CM6 editor once per Y.Doc (collaboration mode).
  // biome-ignore lint/correctness/useExhaustiveDependencies: ydoc drives the editor lifecycle; resolvedTheme and imageResolver are applied reactively in sibling effects
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

    const control = createEditor(parent, {
      initialText: "",
      settings: initialSettings,
      collaboration: {
        ydoc,
        fragmentName: "document",
      },
      imageResolver,
      autofocus: true,
      onEvent: (event) => {
        if (event.kind === EditorEventType.Change) {
          useEditorStore.getState().bumpEditorChangeTick();
        }
      },
    });

    controlRef.current = control;
    useEditorStore.getState().setEditorControl(control);

    return () => {
      controlRef.current = null;
      useEditorStore.getState().setEditorControl(null);
      control.destroy();
    };
  }, [ydoc]);

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

    const handleFiles = async (files: FileList | File[]) => {
      const control = controlRef.current;
      if (!control) return;
      const rel = useEditorStore.getState().relPath;
      for (const file of Array.from(files)) {
        if (!file.type.startsWith("image/")) continue;
        const buffer = await file.arrayBuffer();
        const bytes = Array.from(new Uint8Array(buffer));
        const savedRel = await saveMedia(rel, file.name, bytes);
        control.execCommand("insertImage", savedRel, file.name);
      }
    };

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
  }, []);

  return (
    <div
      ref={containerRef}
      className={`h-full w-full ${
        readableLineLength ? "[&_.cm-content]:mx-auto [&_.cm-content]:max-w-4xl" : ""
      }`}
    />
  );
}
