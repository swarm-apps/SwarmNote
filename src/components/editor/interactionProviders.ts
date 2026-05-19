import type { SlashItem, WikilinkItem } from "@swarmnote/editor-core";
import type { FileTreeNode_Serialize as FileTreeNode } from "@/lib/bindings";
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

/** Snapshot notes from fileTreeStore matching `query` (empty → first N). */
function matchNotes(query: string): FileTreeNode[] {
  const trimmed = query.trim().toLowerCase();
  const notes = flattenNotes(useFileTreeStore.getState().tree);
  if (!trimmed) return notes.slice(0, MAX_NOTE_JUMP_ITEMS);
  return notes
    .filter((n) => basename(n.id).toLowerCase().includes(trimmed))
    .slice(0, MAX_NOTE_JUMP_ITEMS);
}

// ---------------------------------------------------------------------------
// MRU: persist recently-confirmed slash item ids to localStorage so favourites
// surface at the top across sessions (Notion-style).
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Basic block catalog — module-level static data (no captured state, no
// closures). Items use `commandId` + `commandArgs` exclusively so the same
// shape works on both desktop and RN (RN's Comlink can't serialize `run`
// closures). Only "date.today" uses `run` because it computes a value.
// ---------------------------------------------------------------------------

const HEADING_LEVELS: ReadonlyArray<{ level: 1 | 2 | 3; icon: string; description: string }> = [
  { level: 1, icon: "heading-1", description: "Top-level section heading" },
  { level: 2, icon: "heading-2", description: "Section heading" },
  { level: 3, icon: "heading-3", description: "Subsection heading" },
];

const BASIC_BLOCK_ITEMS: readonly SlashItem[] = [
  ...HEADING_LEVELS.map(
    ({ level, icon, description }): SlashItem => ({
      id: `heading.${level}`,
      title: `Heading ${level}`,
      description,
      icon,
      keywords: [`h${level}`, "heading", "标题"],
      section: "Basic",
      commandId: "toggleHeading",
      commandArgs: [level],
    }),
  ),
  {
    id: "list.bulleted",
    title: "Bulleted list",
    description: "Insert an unordered list",
    icon: "list",
    keywords: ["list", "bullet", "unordered", "无序列表"],
    section: "Basic",
    commandId: "toggleUnorderedList",
  },
  {
    id: "list.numbered",
    title: "Numbered list",
    description: "Insert an ordered list",
    icon: "list-ordered",
    keywords: ["list", "ordered", "numbered", "有序列表"],
    section: "Basic",
    commandId: "toggleOrderedList",
  },
  {
    id: "list.check",
    title: "Check list",
    description: "Insert a todo / checkbox list",
    icon: "list-todo",
    keywords: ["check", "todo", "task", "任务", "复选"],
    section: "Basic",
    commandId: "toggleCheckList",
  },
  {
    id: "quote",
    title: "Quote",
    description: "Insert a blockquote",
    icon: "quote",
    keywords: ["quote", "blockquote", "引用"],
    section: "Basic",
    commandId: "toggleBlockquote",
  },
  {
    id: "divider",
    title: "Divider",
    description: "Insert a horizontal rule",
    icon: "minus",
    keywords: ["divider", "hr", "separator", "分割线"],
    section: "Basic",
    commandId: "insertHorizontalRule",
  },
  {
    id: "date.today",
    title: "Today's date",
    description: "Insert YYYY-MM-DD at cursor",
    icon: "calendar",
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

// ---------------------------------------------------------------------------
// Host providers consumed by editor SDK
// ---------------------------------------------------------------------------

/**
 * Host `getSlashItems`. Returns:
 * 1. Basic block catalog (Heading / List / Quote / Divider / Date)
 * 2. Jump-to-note items matching query (or first 8 when empty)
 *
 * MRU items get a boosted priority + "Recent" section so favourites surface
 * at the top. Plugin items are merged in by the SDK from `ctx.registerSlashItems`.
 */
export async function getSlashItems(query: string, signal: AbortSignal): Promise<SlashItem[]> {
  if (signal.aborted) return [];

  const items: SlashItem[] = [...BASIC_BLOCK_ITEMS];

  for (const note of matchNotes(query)) {
    const title = basename(note.id);
    items.push({
      id: `jump:${note.id}`,
      title: `Jump to: ${title}`,
      description: note.id,
      icon: "file-text",
      section: "Notes",
      run: () => {
        useEditorStore.getState().loadDocument(note.id, title, note.id);
      },
    });
  }

  const mru = readMru();
  if (mru.length > 0) {
    for (const item of items) {
      const idx = mru.indexOf(item.id);
      if (idx >= 0) {
        item.priority = MRU_PRIORITY_BASE + (MRU_LIMIT - idx);
        item.section = "Recent";
      }
    }
  }

  if (signal.aborted) return [];
  return items;
}

/** Host `getWikilinkItems`. Returns matching note titles for `[[query` trigger. */
export async function getWikilinkItems(
  query: string,
  signal: AbortSignal,
): Promise<WikilinkItem[]> {
  if (signal.aborted) return [];

  const items: WikilinkItem[] = matchNotes(query).map((note) => ({
    id: note.id,
    title: basename(note.id),
    description: note.id,
    icon: "file-text",
    commit: "replaceWithLink",
  }));

  if (signal.aborted) return [];
  return items;
}

/**
 * Resolve a `LinkOpen` event's url to an internal note. Returns null when the
 * url should be opened as an external URL by the host.
 *
 * Match order:
 * 1. External scheme (`xxx://` / `mailto:` / `tel:` ...) → null
 * 2. `.md` path → exact match
 * 3. Wikilink target → basename case-insensitive, then fuzzy contains
 */
export function resolveInternalLink(url: string): { id: string; title: string } | null {
  if (!url) return null;
  if (/^[a-z][a-z0-9+.-]*:/i.test(url)) return null;

  const normalized = url.startsWith("./") ? url.slice(2) : url;
  const lowered = normalized.toLowerCase();
  const notes = flattenNotes(useFileTreeStore.getState().tree);

  if (lowered.endsWith(".md")) {
    const hit = notes.find((n) => n.id === normalized);
    if (hit) return { id: hit.id, title: basename(hit.id) };
  }

  const exactTitle = notes.find((n) => basename(n.id).toLowerCase() === lowered);
  if (exactTitle) return { id: exactTitle.id, title: basename(exactTitle.id) };

  const fuzzy = notes.find((n) => basename(n.id).toLowerCase().includes(lowered));
  if (fuzzy) return { id: fuzzy.id, title: basename(fuzzy.id) };

  return null;
}
