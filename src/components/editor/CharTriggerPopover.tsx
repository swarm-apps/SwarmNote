import type { EditorControl } from "@swarmnote/editor-core";
import { useEffect, useMemo, useRef } from "react";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

/**
 * Minimum item shape required by the popover. `SlashItem` and `WikilinkItem`
 * both satisfy this (their extra fields like `commandId` / `commit` / `run`
 * are owned by the SDK, not relevant to rendering).
 */
export interface CharTriggerItem {
  id: string;
  title: string;
  description?: string;
  icon?: string;
  section?: string;
}

/**
 * Match shape produced by `slash.*` / `wikilink.*` SDK runtime — keeps this
 * popover decoupled from either specific match type.
 */
export interface CharTriggerMatchLike<TItem extends CharTriggerItem> {
  active: boolean;
  items: TItem[];
  activeIndex: number;
  screenRect?: { x: number; y: number; width: number; height: number };
}

interface CharTriggerPopoverProps<TItem extends CharTriggerItem> {
  match: CharTriggerMatchLike<TItem> | null;
  control: EditorControl | null;
  /**
   * Command id prefix. Keyboard routes ArrowDown/Up/Enter/Escape to
   * `<prefix>.next` / `.prev` / `.confirm` / `.dismiss`; clicks dispatch
   * `<prefix>.confirmAt(index)`.
   */
  commandPrefix: "slash" | "wikilink";
  /** Optional header label rendered above items (e.g. "Link to note"). */
  headerLabel?: string;
  /** Empty-state label when items is empty. */
  emptyLabel: string;
  /** Override side defaults — useful if anchor placement differs. */
  side?: "top" | "bottom";
}

/**
 * Shared floating Radix Popover for char-trigger interactions (slash / wikilink).
 *
 * Subscribes to keyboard events on the editor's contentDOM while open and
 * routes ↑/↓/Enter/Escape to `<commandPrefix>.*` commands. Items are grouped
 * by `section` when any item declares one. Mouse picks go through
 * `<commandPrefix>.confirmAt(index)` to atomically jump-and-commit.
 */
export function CharTriggerPopover<TItem extends CharTriggerItem>({
  match,
  control,
  commandPrefix,
  headerLabel,
  emptyLabel,
  side = "bottom",
}: CharTriggerPopoverProps<TItem>) {
  const open = match?.active ?? false;
  const items = match?.items ?? [];
  const activeIndex = match?.activeIndex ?? 0;
  const screenRect = match?.screenRect;

  const controlRef = useRef(control);
  controlRef.current = control;

  useEffect(() => {
    if (!open || !control) return;
    const contentDom = control.view.contentDOM;
    const handler = (e: KeyboardEvent) => {
      let suffix: string | null = null;
      if (e.key === "ArrowDown") suffix = "next";
      else if (e.key === "ArrowUp") suffix = "prev";
      else if (e.key === "Enter") suffix = "confirm";
      else if (e.key === "Escape") suffix = "dismiss";
      if (!suffix) return;
      e.preventDefault();
      e.stopPropagation();
      controlRef.current?.execCommand(`${commandPrefix}.${suffix}`);
    };
    contentDom.addEventListener("keydown", handler, true);
    return () => {
      contentDom.removeEventListener("keydown", handler, true);
    };
  }, [open, control, commandPrefix]);

  // Group items by section if any item declares one
  const grouped = useMemo(() => {
    const buckets = new Map<string, TItem[]>();
    for (const it of items) {
      const key = it.section ?? "";
      const arr = buckets.get(key) ?? [];
      arr.push(it);
      buckets.set(key, arr);
    }
    return Array.from(buckets.entries());
  }, [items]);

  if (!open || !screenRect) return null;

  return (
    <Popover open={open}>
      <PopoverAnchor asChild>
        <div
          aria-hidden
          style={{
            position: "fixed",
            left: screenRect.x,
            top: screenRect.y,
            width: screenRect.width,
            height: screenRect.height,
            pointerEvents: "none",
          }}
        />
      </PopoverAnchor>
      <PopoverContent
        align="start"
        side={side}
        sideOffset={4}
        className="w-72 p-1"
        onOpenAutoFocus={(e) => e.preventDefault()}
        onCloseAutoFocus={(e) => e.preventDefault()}
      >
        {headerLabel ? (
          <div className="px-2 pt-1.5 pb-0.5 text-xs font-medium text-muted-foreground">
            {headerLabel}
          </div>
        ) : null}
        {items.length === 0 ? (
          <div className="px-2 py-1.5 text-sm text-muted-foreground">{emptyLabel}</div>
        ) : (
          <div className="flex flex-col gap-0.5 max-h-72 overflow-y-auto">
            {grouped.map(([section, sectionItems]) => (
              <Section
                key={section || "_default"}
                label={section}
                items={sectionItems}
                activeIndex={activeIndex}
                allItems={items}
                onPick={(absoluteIndex) => {
                  controlRef.current?.execCommand(`${commandPrefix}.confirmAt`, absoluteIndex);
                }}
              />
            ))}
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}

interface SectionProps<TItem extends CharTriggerItem> {
  label: string;
  items: TItem[];
  activeIndex: number;
  allItems: TItem[];
  onPick: (absoluteIndex: number) => void;
}

function Section<TItem extends CharTriggerItem>({
  label,
  items,
  activeIndex,
  allItems,
  onPick,
}: SectionProps<TItem>) {
  return (
    <>
      {label ? (
        <div className="px-2 pt-1.5 pb-0.5 text-xs font-medium text-muted-foreground">{label}</div>
      ) : null}
      {items.map((item) => {
        const absoluteIndex = allItems.indexOf(item);
        const active = absoluteIndex === activeIndex;
        return (
          <button
            type="button"
            key={item.id}
            data-active={active || undefined}
            // mousedown 而非 click：blur 在 click 前 fire 会 dismiss popover
            onMouseDown={(e) => {
              e.preventDefault();
              onPick(absoluteIndex);
            }}
            className={cn(
              "flex items-start gap-2 rounded-sm px-2 py-1.5 text-sm text-left w-full",
              "cursor-pointer select-none",
              active ? "bg-accent text-accent-foreground" : "hover:bg-muted",
            )}
          >
            {item.icon ? (
              <span className="text-base leading-5 flex-shrink-0" aria-hidden>
                {item.icon}
              </span>
            ) : null}
            <div className="flex flex-col min-w-0">
              <div className="truncate">{item.title}</div>
              {item.description ? (
                <div className="truncate text-xs text-muted-foreground">{item.description}</div>
              ) : null}
            </div>
          </button>
        );
      })}
    </>
  );
}
