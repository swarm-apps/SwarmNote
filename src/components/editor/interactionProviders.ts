import type { SlashItem } from "@swarmnote/editor-core";
import type { FileTreeNode } from "@/commands/fs";
import { useEditorStore } from "@/stores/editorStore";
import { useFileTreeStore } from "@/stores/fileTreeStore";

const MAX_NOTE_JUMP_ITEMS = 8;

function flattenNotes(nodes: FileTreeNode[]): FileTreeNode[] {
  const flat: FileTreeNode[] = [];
  const walk = (xs: FileTreeNode[]) => {
    for (const n of xs) {
      if (n.children) walk(n.children);
      else if (n.id.endsWith(".md")) flat.push(n);
    }
  };
  walk(nodes);
  return flat;
}

function basename(relPath: string): string {
  const last = relPath.split("/").pop() ?? relPath;
  return last.replace(/\.md$/i, "");
}

/**
 * Host implementation of `EditorHostCapabilities.getSlashItems`.
 *
 * - Empty query: a small set of quick actions (currently jump-to-recent items).
 * - Non-empty query: fuzzy-match note titles from `fileTreeStore` and return
 *   "Jump to: <title>" items.
 *
 * AbortSignal honored by an early `signal.aborted` check before returning.
 */
export async function getSlashItems(query: string, signal: AbortSignal): Promise<SlashItem[]> {
  if (signal.aborted) return [];

  const items: SlashItem[] = [];
  const trimmed = query.trim().toLowerCase();
  const tree = useFileTreeStore.getState().tree;
  const allNotes = flattenNotes(tree);

  // Jump-to-note items
  const matchedNotes = trimmed
    ? allNotes.filter((n) => basename(n.id).toLowerCase().includes(trimmed))
    : allNotes.slice(0, MAX_NOTE_JUMP_ITEMS);

  for (const note of matchedNotes.slice(0, MAX_NOTE_JUMP_ITEMS)) {
    const title = basename(note.id);
    items.push({
      id: `jump:${note.id}`,
      title: `Jump to: ${title}`,
      description: note.id,
      icon: "📄",
      section: "Notes",
      run: () => {
        useEditorStore.getState().loadDocument(note.id, title, note.id);
      },
    });
  }

  if (signal.aborted) return [];
  return items;
}
