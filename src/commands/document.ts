import { invoke } from "@tauri-apps/api/core";

export interface DocumentModel {
  id: string;
  workspace_id: string;
  folder_id: string | null;
  title: string;
  rel_path: string;
  file_hash: number[] | null;
  yjs_state: number[] | null;
  state_vector: number[] | null;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface UpsertDocumentInput {
  id?: string;
  workspace_id: string;
  folder_id?: string | null;
  title: string;
  rel_path: string;
  file_hash?: string | null;
}

export async function getDocuments(workspaceId: string): Promise<DocumentModel[]> {
  return invoke<DocumentModel[]>("db_get_documents", { workspaceId });
}

export async function upsertDocument(input: UpsertDocumentInput): Promise<DocumentModel> {
  return invoke<DocumentModel>("db_upsert_document", { input });
}

export async function deleteDocumentByRelPath(relPath: string): Promise<void> {
  return invoke("delete_document_by_rel_path", { relPath });
}

export async function deleteDocumentsByPrefix(prefix: string): Promise<number> {
  return invoke<number>("delete_documents_by_prefix", { prefix });
}

export async function renameDocument(
  oldRelPath: string,
  newRelPath: string,
  newTitle: string,
): Promise<void> {
  return invoke("rename_document", {
    input: { old_rel_path: oldRelPath, new_rel_path: newRelPath, new_title: newTitle },
  });
}

export interface MoveDocumentResult {
  new_rel_path: string;
  is_dir: boolean;
}

/**
 * Atomically move a document or folder on disk and update the DB + open Y.Docs.
 * `toRelPath` must be the full target path (including filename/dirname), not
 * the destination parent directory. Rejects folder-into-self and existing
 * targets.
 */
export async function moveDocument(
  fromRelPath: string,
  toRelPath: string,
): Promise<MoveDocumentResult> {
  return invoke<MoveDocumentResult>("move_document", {
    input: { from_rel_path: fromRelPath, to_rel_path: toRelPath },
  });
}

export async function saveMedia(
  relPath: string,
  fileName: string,
  data: number[],
): Promise<string> {
  return invoke<string>("save_media", { relPath, fileName, data });
}

// ── Y.Doc commands ──

export interface OpenDocResult {
  doc_uuid: string;
  yjs_state: number[];
}

export async function openYDoc(relPath: string, workspaceId: string): Promise<OpenDocResult> {
  return invoke<OpenDocResult>("open_ydoc", { relPath, workspaceId });
}

export async function applyYDocUpdate(docUuid: string, update: number[]): Promise<void> {
  return invoke("apply_ydoc_update", { docUuid, update });
}

export async function closeYDoc(docUuid: string): Promise<void> {
  return invoke("close_ydoc", { docUuid });
}

export async function renameYDoc(docUuid: string, newRelPath: string): Promise<void> {
  return invoke("rename_ydoc", { docUuid, newRelPath });
}

export async function reloadYDocConfirmed(docUuid: string): Promise<void> {
  return invoke("reload_ydoc_confirmed", { docUuid });
}
