import type { EditorControl, SlashItem, SlashTriggerMatch } from "@swarmnote/editor-core";
import {
  CalendarDays,
  CircleAlert,
  Code,
  FileText,
  Heading1,
  Heading2,
  Heading3,
  Image as ImageIcon,
  Lightbulb,
  List,
  ListOrdered,
  ListTodo,
  type LucideIcon,
  Minus,
  OctagonAlert,
  Quote,
  Sigma,
  StickyNote,
  Table as TableIcon,
  TriangleAlert,
} from "lucide-react";
import { useMemo } from "react";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import { useTriggerKeyboard } from "@/lib/use-trigger-keyboard";
import { cn } from "@/lib/utils";

/**
 * Built-in icon registry mapping `SlashItem.icon` semantic name → lucide
 * component. Extend via `iconRegistry` prop to add custom icons; items with
 * an unknown icon fall back to plain text rendering.
 */
const DEFAULT_ICON_REGISTRY: Record<string, LucideIcon> = {
  // basic blocks
  "heading-1": Heading1,
  "heading-2": Heading2,
  "heading-3": Heading3,
  list: List,
  "list-ordered": ListOrdered,
  "list-todo": ListTodo,
  quote: Quote,
  minus: Minus,
  "file-text": FileText,
  calendar: CalendarDays,
  // built-in plugins (math / table / codeBlock / blockImage)
  table: TableIcon,
  sigma: Sigma,
  "square-code": Code,
  image: ImageIcon,
  // admonition variants
  "sticky-note": StickyNote,
  lightbulb: Lightbulb,
  "triangle-alert": TriangleAlert,
  "circle-alert": CircleAlert,
  "octagon-alert": OctagonAlert,
};

interface SlashPopoverProps {
  /**
   * Current slash trigger match. Host typically owns this state via
   * `createEditor({ onEvent })` SlashTriggerChange listener; pass `null`
   * (or match with `active: false`) to close.
   */
  match: SlashTriggerMatch | null;
  /** Editor control, used for keyboard handling and click commit. */
  control: EditorControl | null;
  /** Optional override of the empty-state label. */
  emptyLabel?: string;
  /** Side preference for the popover. Defaults to "bottom". */
  side?: "top" | "bottom";
  /**
   * Optional icon registry override. Extends or replaces the built-in
   * lucide mapping. Item with an icon string not in the registry falls
   * back to plain text (legacy emoji-style icon still renders).
   */
  iconRegistry?: Record<string, LucideIcon>;
}

/**
 * Notion-style slash command popover bound to `@swarmnote/editor-core`'s
 * built-in `slashCommandPlugin`.
 *
 * Distributed via shadcn registry — consumers run `shadcn add slash-popover`
 * and own the source. Renders a floating list anchored to the editor's
 * caret rect; keyboard ↑/↓/Enter/Escape dispatch into `slash.*` commands.
 *
 * Customization typically goes by editing this file in your `components/`
 * directory after `shadcn add`. For deeper changes (item shape, command
 * names) see `@swarmnote/editor-core` plugin docs.
 */
export function SlashPopover({
  match,
  control,
  emptyLabel = "No matching commands",
  side = "bottom",
  iconRegistry,
}: SlashPopoverProps) {
  const open = match?.active ?? false;
  const items: SlashItem[] = match?.items ?? [];
  const activeIndex = match?.activeIndex ?? 0;
  const screenRect = match?.screenRect;
  const icons = iconRegistry
    ? { ...DEFAULT_ICON_REGISTRY, ...iconRegistry }
    : DEFAULT_ICON_REGISTRY;

  useTriggerKeyboard(open, control, "slash");

  // Group items by `section` (Notion-style category headers). Items with no
  // section share the implicit empty bucket rendered first.
  const grouped = useMemo(() => {
    const buckets = new Map<string, SlashItem[]>();
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
        {items.length === 0 ? (
          <div className="px-2 py-1.5 text-sm text-muted-foreground" role="status">
            {emptyLabel}
          </div>
        ) : (
          <div
            role="listbox"
            aria-label="Slash command suggestions"
            className="flex flex-col gap-0.5 max-h-72 overflow-y-auto"
          >
            {grouped.map(([section, sectionItems]) => (
              <Section
                key={section || "_default"}
                label={section}
                items={sectionItems}
                allItems={items}
                activeIndex={activeIndex}
                icons={icons}
                onPick={(absoluteIndex) => {
                  control?.execCommand("slash.confirmAt", absoluteIndex);
                }}
              />
            ))}
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}

interface SectionProps {
  label: string;
  items: SlashItem[];
  allItems: SlashItem[];
  activeIndex: number;
  icons: Record<string, LucideIcon>;
  onPick: (absoluteIndex: number) => void;
}

function Section({ label, items, allItems, activeIndex, icons, onPick }: SectionProps) {
  return (
    <>
      {label ? (
        <div className="px-2 pt-1.5 pb-0.5 text-xs font-medium text-muted-foreground">{label}</div>
      ) : null}
      {items.map((item) => {
        const absoluteIndex = allItems.indexOf(item);
        const active = absoluteIndex === activeIndex;
        const Icon = item.icon ? icons[item.icon] : undefined;
        return (
          <button
            type="button"
            key={item.id}
            role="option"
            aria-selected={active}
            data-active={active || undefined}
            // mousedown fires before blur — using onClick would lose the popover
            // to a flicker. Critical for picker-style UIs.
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
            {Icon ? (
              <Icon className="h-4 w-4 flex-shrink-0 mt-0.5" aria-hidden />
            ) : item.icon ? (
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
