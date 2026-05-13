import type { SlashItem, WikilinkItem } from "@swarmnote/editor-core";
import type { FileTreeNode } from "@/commands/fs";
import { useEditorStore } from "@/stores/editorStore";
import { useFileTreeStore } from "@/stores/fileTreeStore";

const MAX_NOTE_JUMP_ITEMS = 8;
const MRU_STORAGE_KEY = "swarmnote.slash.mru";
const MRU_LIMIT = 20;
const MRU_PRIORITY_BASE = 300;

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
 * MRU registry of recently-used slash item ids. Persisted to localStorage so
 * users see their favourites near the top across sessions (Notion-style).
 *
 * The list is most-recent-first; `bumpSlashMru(id)` lifts an id to the head.
 */
function readMru(): string[] {
  try {
    const raw = localStorage.getItem(MRU_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((x): x is string => typeof x === "string") : [];
  } catch {
    return [];
  }
}

function writeMru(ids: string[]): void {
  try {
    localStorage.setItem(MRU_STORAGE_KEY, JSON.stringify(ids.slice(0, MRU_LIMIT)));
  } catch {
    // Quota exceeded or storage unavailable — silently skip
  }
}

export function bumpSlashMru(id: string): void {
  const cur = readMru();
  writeMru([id, ...cur.filter((x) => x !== id)]);
}

/** Build a static block-item catalog (Heading / List / Quote / Divider / Date). */
function basicBlockItems(): SlashItem[] {
  return [
    {
      id: "heading.1",
      title: "Heading 1",
      description: "Top-level section heading",
      icon: "H₁",
      keywords: ["h1", "heading", "标题"],
      section: "Basic",
      run: () => {
        useEditorStore.getState().editorControl?.execCommand("toggleHeading", 1);
      },
    },
    {
      id: "heading.2",
      title: "Heading 2",
      description: "Section heading",
      icon: "H₂",
      keywords: ["h2", "heading", "标题"],
      section: "Basic",
      run: () => {
        useEditorStore.getState().editorControl?.execCommand("toggleHeading", 2);
      },
    },
    {
      id: "heading.3",
      title: "Heading 3",
      description: "Subsection heading",
      icon: "H₃",
      keywords: ["h3", "heading", "标题"],
      section: "Basic",
      run: () => {
        useEditorStore.getState().editorControl?.execCommand("toggleHeading", 3);
      },
    },
    {
      id: "list.bulleted",
      title: "Bulleted list",
      description: "Insert an unordered list",
      icon: "•",
      keywords: ["list", "bullet", "unordered", "无序列表"],
      section: "Basic",
      commandId: "toggleUnorderedList",
    },
    {
      id: "list.numbered",
      title: "Numbered list",
      description: "Insert an ordered list",
      icon: "1.",
      keywords: ["list", "ordered", "numbered", "有序列表"],
      section: "Basic",
      commandId: "toggleOrderedList",
    },
    {
      id: "list.check",
      title: "Check list",
      description: "Insert a todo / checkbox list",
      icon: "☐",
      keywords: ["check", "todo", "task", "任务", "复选"],
      section: "Basic",
      commandId: "toggleCheckList",
    },
    {
      id: "quote",
      title: "Quote",
      description: "Insert a blockquote",
      icon: "❝",
      keywords: ["quote", "blockquote", "引用"],
      section: "Basic",
      commandId: "toggleBlockquote",
    },
    {
      id: "divider",
      title: "Divider",
      description: "Insert a horizontal rule",
      icon: "—",
      keywords: ["divider", "hr", "separator", "分割线"],
      section: "Basic",
      commandId: "insertHorizontalRule",
    },
    {
      id: "date.today",
      title: "Today's date",
      description: "Insert YYYY-MM-DD at cursor",
      icon: "📅",
      keywords: ["date", "today", "日期", "今天"],
      section: "Basic",
      run: ({ view, range }) => {
        const now = new Date();
        const yyyy = now.getFullYear();
        const mm = String(now.getMonth() + 1).padStart(2, "0");
        const dd = String(now.getDate()).padStart(2, "0");
        const insert = `${yyyy}-${mm}-${dd}`;
        view.dispatch({
          changes: { from: range.from, insert },
          selection: { anchor: range.from + insert.length },
        });
      },
    },
  ];
}

/**
 * Host implementation of `EditorHostCapabilities.getSlashItems`.
 *
 * Returns three groups:
 * 1. Basic blocks (Heading / List / Quote / Divider / Date) — via commandId or run
 * 2. Notes (Jump to: <title>) — from `fileTreeStore`, fuzzy on query
 * 3. (Plugin items are merged in by the SDK from `ctx.registerSlashItems`)
 *
 * MRU items get a boosted priority so recently-used items appear near the top.
 */
export async function getSlashItems(query: string, signal: AbortSignal): Promise<SlashItem[]> {
  if (signal.aborted) return [];

  const items: SlashItem[] = [];
  const trimmed = query.trim().toLowerCase();

  // 1. Basic block catalog
  items.push(...basicBlockItems());

  // 2. Note jumps
  const tree = useFileTreeStore.getState().tree;
  const allNotes = flattenNotes(tree);
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

  // 3. Lift MRU items via per-item priority override (SDK reads item.priority)
  const mru = readMru();
  if (mru.length > 0) {
    for (const item of items) {
      const idx = mru.indexOf(item.id);
      if (idx >= 0) {
        item.priority = MRU_PRIORITY_BASE + (MRU_LIMIT - idx);
        // Re-section so the popover renders them in a "Recent" group
        item.section = "Recent";
      }
    }
  }

  if (signal.aborted) return [];
  return items;
}

/**
 * Host implementation of `EditorHostCapabilities.getWikilinkItems`.
 *
 * Returns matching note titles from `fileTreeStore`. Empty query returns
 * the first 8 notes (lets users browse without typing).
 */
export async function getWikilinkItems(
  query: string,
  signal: AbortSignal,
): Promise<WikilinkItem[]> {
  if (signal.aborted) return [];

  const trimmed = query.trim().toLowerCase();
  const tree = useFileTreeStore.getState().tree;
  const allNotes = flattenNotes(tree);
  const matched = trimmed
    ? allNotes.filter((n) => basename(n.id).toLowerCase().includes(trimmed))
    : allNotes.slice(0, MAX_NOTE_JUMP_ITEMS);

  const items: WikilinkItem[] = matched.slice(0, MAX_NOTE_JUMP_ITEMS).map((note) => ({
    id: note.id,
    title: basename(note.id),
    description: note.id,
    icon: "📄",
    commit: "replaceWithLink" as const,
  }));

  if (signal.aborted) return [];
  return items;
}
