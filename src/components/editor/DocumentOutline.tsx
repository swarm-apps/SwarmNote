import { Trans } from "@lingui/react/macro";
import { extractHeadings, type HeadingItem } from "@swarmnote/editor";
import { ListTree } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { cn } from "@/lib/utils";
import { useEditorStore } from "@/stores/editorStore";

const OUTLINE_DEBOUNCE_MS = 300;
const SCROLL_THROTTLE_MS = 16;

interface DocumentOutlineProps {
  height: number;
}

function findActiveHeadingIndex(
  headings: readonly HeadingItem[],
  scrollTop: number,
  offsetToTop: Map<number, number>,
): number {
  if (headings.length === 0) return -1;
  // Find the last heading whose line top is at or above the current scroll.
  let active = -1;
  for (let i = 0; i < headings.length; i++) {
    const top = offsetToTop.get(headings[i].offset);
    if (top === undefined) continue;
    if (top <= scrollTop + 4 /* small tolerance */) {
      active = i;
    } else {
      break;
    }
  }
  return active === -1 ? 0 : active;
}

export function DocumentOutline({ height }: DocumentOutlineProps) {
  const editorControl = useEditorStore((s) => s.editorControl);
  const changeTick = useEditorStore((s) => s.editorChangeTick);

  const [headings, setHeadings] = useState<HeadingItem[]>([]);
  const [activeIndex, setActiveIndex] = useState(0);
  const isScrollingRef = useRef(false);

  // Re-extract headings on content change (debounced).
  // biome-ignore lint/correctness/useExhaustiveDependencies: changeTick is the trigger signal; its value is intentionally unread in the body
  useEffect(() => {
    if (!editorControl) {
      setHeadings([]);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (cancelled) return;
      setHeadings(extractHeadings(editorControl.view.state));
    }, OUTLINE_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [editorControl, changeTick]);

  // Initial extraction (no debounce) when the editor becomes available.
  useEffect(() => {
    if (!editorControl) return;
    setHeadings(extractHeadings(editorControl.view.state));
  }, [editorControl]);

  // Track active heading via scroll position.
  useEffect(() => {
    if (!editorControl || headings.length === 0) {
      setActiveIndex(0);
      return;
    }
    const view = editorControl.view;
    const scroller = view.scrollDOM;

    // Pre-compute each heading's line-top in absolute document coordinates.
    // Must recompute whenever headings change (new/removed lines shift positions).
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
  }, [editorControl, headings]);

  const handleClick = useCallback(
    (index: number) => {
      const control = editorControl;
      const heading = headings[index];
      if (!control || !heading) return;

      isScrollingRef.current = true;
      setActiveIndex(index);

      const view = control.view;
      const block = view.lineBlockAt(heading.offset);
      // Scroll such that the heading sits roughly 1/4 from the top of the viewport.
      const scroller = view.scrollDOM;
      const scrollTargetOffset = scroller.clientHeight / 4;
      scroller.scrollTo({ top: Math.max(0, block.top - scrollTargetOffset), behavior: "smooth" });

      view.dispatch({
        selection: { anchor: heading.offset },
      });
      view.focus();

      window.setTimeout(() => {
        isScrollingRef.current = false;
      }, 500);
    },
    [editorControl, headings],
  );

  if (!editorControl) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
        <ListTree className="h-8 w-8 opacity-30" />
        <p className="text-xs">
          <Trans>打开文档以查看大纲</Trans>
        </p>
      </div>
    );
  }

  if (headings.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
        <ListTree className="h-8 w-8 opacity-30" />
        <p className="px-4 text-center text-xs">
          <Trans>在文档中添加标题即可看到大纲导航</Trans>
        </p>
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
