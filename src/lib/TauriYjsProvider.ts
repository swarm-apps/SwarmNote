import { invoke } from "@tauri-apps/api/core";
import { Awareness, applyAwarenessUpdate, encodeAwarenessUpdate } from "y-protocols/awareness";
import type * as Y from "yjs";
import { applyYDocUpdate, closeYDoc } from "@/commands/document";

const REMOTE_AWARENESS_ORIGIN = "remote-awareness";

/**
 * Custom yjs provider that bridges BlockNote's collaboration layer with the
 * Tauri Rust backend via IPC. Uses the stable database UUID (not relPath)
 * to identify documents, so renames don't break the connection.
 *
 * Also owns the `Awareness` instance for caret/presence sync. Awareness
 * updates are forwarded to the Rust core via `broadcast_awareness` IPC and
 * never persisted — they ride a parallel GossipSub topic.
 */
export class TauriYjsProvider {
  public awareness: Awareness;
  public doc: Y.Doc;
  private _docUuid: string;
  private _destroying = false;

  constructor(doc: Y.Doc, docUuid: string) {
    this.doc = doc;
    this._docUuid = docUuid;
    this.awareness = new Awareness(doc);

    doc.on("update", this._onDocUpdate);
    this.awareness.on("update", this._onAwarenessUpdate);
  }

  private _onDocUpdate = (update: Uint8Array, origin: unknown) => {
    if (this._destroying) return;
    if (origin === "remote") return;

    applyYDocUpdate(this._docUuid, Array.from(update)).catch((err) => {
      console.error("Failed to send yjs update to backend:", err);
    });
  };

  private _onAwarenessUpdate = (
    changed: { added: number[]; updated: number[]; removed: number[] },
    origin: unknown,
  ) => {
    if (this._destroying) return;
    if (origin === REMOTE_AWARENESS_ORIGIN) return;
    const changedClients = [...changed.added, ...changed.updated, ...changed.removed];
    if (changedClients.length === 0) return;
    const payload = encodeAwarenessUpdate(this.awareness, changedClients);
    invoke("broadcast_awareness", {
      docUuid: this._docUuid,
      update: Array.from(payload),
    }).catch((err) => {
      console.error("Failed to broadcast awareness:", err);
    });
  };

  /** Apply a remote awareness update received via the `yjs:awareness-update`
   *  Tauri event. Uses REMOTE_AWARENESS_ORIGIN so our local listener won't
   *  re-broadcast it. */
  applyRemoteAwarenessUpdate(update: Uint8Array): void {
    if (this._destroying) return;
    applyAwarenessUpdate(this.awareness, update, REMOTE_AWARENESS_ORIGIN);
  }

  destroy() {
    // setLocalState(null) must precede off('update') so the synthetic
    // "removed" event reaches the listener and triggers the broadcast —
    // see dev-notes/knowledge/editor.md "destroy 顺序敏感".
    this.awareness.setLocalState(null);

    this._destroying = true;
    this.doc.off("update", this._onDocUpdate);
    this.awareness.off("update", this._onAwarenessUpdate);
    this.awareness.destroy();

    closeYDoc(this._docUuid).catch((err) => {
      console.error("Failed to close ydoc:", err);
    });
  }
}
