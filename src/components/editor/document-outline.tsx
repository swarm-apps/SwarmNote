import { type EditorControl, extractHeadings, type HeadingItem } from "@swarmnote/editor-core";
import { ListTree } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

const OUTLINE_DEBOUNCE_MS = 300;
const SCROLL_THROTTLE_MS = 16;

interface DocumentOutlineProps {
  /** Editor control. Pass `null` while editor is mounting; outline shows empty state. */
  control: EditorControl | null;
  /**
   * Tick value that increments on every editor change. Host typically maintains
   * this via `createEditor({ onEvent: e => e.kind === 'change' && setTick(t+1) })`
   * or any similar mechanism. The outline re-extracts headings (debounced 300ms)
   * whenever this value changes.
   */
  changeTick: number;
  /** Outline navigation height in px (passed to the scrollable container). */
  height: number;
  /** Optional override of the "no editor" placeholder text. */
  emptyEditorLabel?: string;
  /** Optional override of the "no headings" placeholder text. */
  emptyHeadingsLabel?: string;
}

function findActiveHeadingIndex(
  headings: readonly HeadingItem[],
  scrollTop: number,
  offsetToTop: Map<number, number>,
): number {
  if (headings.length === 0) return -1;
  let active = -1;
  for (let i = 0; i < headings.length; i++) {
    const top = offsetToTop.get(headings[i].offset);
    if (top === undefined) continue;
    if (top <= scrollTop + 4) {
      active = i;
    } else {
      break;
    }
  }
  return active === -1 ? 0 : active;
}

/**
 * Document outline panel — scrollable list of editor headings with auto-highlight
 * of the section currently at the top of the viewport. Clicking a heading scrolls
 * the editor to that position and places the cursor at the heading line.
 *
 * Distributed via shadcn registry — consumers run `shadcn add document-outline`
 * and own the source. Useful as a sidebar / second-pane navigation widget.
 */
export function DocumentOutline({
  control,
  changeTick,
  height,
  emptyEditorLabel = "Open a document to see the outline",
  emptyHeadingsLabel = "Add headings to your document to see the outline",
}: DocumentOutlineProps) {
  const [headings, setHeadings] = useState<HeadingItem[]>([]);
  const [activeIndex, setActiveIndex] = useState(0);
  const isScrollingRef = useRef(false);

  // Re-extract headings on content change (debounced).
  // biome-ignore lint/correctness/useExhaustiveDependencies: changeTick is the trigger signal; intentionally unread
  useEffect(() => {
    if (!control) {
      setHeadings([]);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (cancelled) return;
      setHeadings(extractHeadings(control.view.state));
    }, OUTLINE_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [control, changeTick]);

  // Initial extraction (no debounce) when the editor becomes available.
  useEffect(() => {
    if (!control) return;
    setHeadings(extractHeadings(control.view.state));
  }, [control]);

  // Track active heading via scroll position.
  useEffect(() => {
    if (!control || headings.length === 0) {
      setActiveIndex(0);
      return;
    }
    const view = control.view;
    const scroller = view.scrollDOM;

    const offsetToTop = new Map<number, number>();
    for (const h of headings) {
      try {
        const block = view.lineBlockAt(h.offset);
        offsetToTop.set(h.offset, block.top);
      } catch {
        // Heading offset may be stale between parse + event; skip.
      }
    }

    let throttleTimer: number | null = null;
    const onScroll = () => {
      if (isScrollingRef.current) return;
      if (throttleTimer !== null) return;
      throttleTimer = window.setTimeout(() => {
        throttleTimer = null;
        const scrollTop = scroller.scrollTop;
        setActiveIndex(findActiveHeadingIndex(headings, scrollTop, offsetToTop));
      }, SCROLL_THROTTLE_MS);
    };

    scroller.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
    return () => {
      scroller.removeEventListener("scroll", onScroll);
      if (throttleTimer !== null) window.clearTimeout(throttleTimer);
    };
  }, [control, headings]);

  const handleClick = useCallback(
    (index: number) => {
      const heading = headings[index];
      if (!control || !heading) return;

      isScrollingRef.current = true;
      setActiveIndex(index);

      const view = control.view;
      const block = view.lineBlockAt(heading.offset);
      const scroller = view.scrollDOM;
      const scrollTargetOffset = scroller.clientHeight / 4;
      scroller.scrollTo({ top: Math.max(0, block.top - scrollTargetOffset), behavior: "smooth" });

      view.dispatch({ selection: { anchor: heading.offset } });
      view.focus();

      window.setTimeout(() => {
        isScrollingRef.current = false;
      }, 500);
    },
    [control, headings],
  );

  if (!control) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
        <ListTree className="h-8 w-8 opacity-30" />
        <p className="text-xs">{emptyEditorLabel}</p>
      </div>
    );
  }

  if (headings.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
        <ListTree className="h-8 w-8 opacity-30" />
        <p className="px-4 text-center text-xs">{emptyHeadingsLabel}</p>
      </div>
    );
  }

  return (
    <nav className="flex flex-col overflow-y-auto" style={{ maxHeight: height }}>
      {headings.map((h, index) => (
        <button
          key={`${h.offset}-${h.level}-${h.text}`}
          type="button"
          onClick={() => handleClick(index)}
          title={h.text}
          className={cn(
            "flex items-center truncate rounded px-2 text-left text-[13px] leading-7 transition-colors",
            h.level === 1
              ? "pl-2 font-medium"
              : h.level === 2
                ? "pl-5"
                : h.level === 3
                  ? "pl-8"
                  : "pl-11",
            activeIndex === index
              ? "bg-sidebar-accent text-sidebar-accent-foreground"
              : "text-sidebar-foreground hover:bg-sidebar-accent/50",
          )}
        >
          {h.text}
        </button>
      ))}
    </nav>
  );
}
