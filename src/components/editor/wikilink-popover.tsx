import type { EditorControl, WikilinkItem, WikilinkTriggerMatch } from "@swarmnote/editor-core";
import { FileText, type LucideIcon } from "lucide-react";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import { useTriggerKeyboard } from "@/lib/use-trigger-keyboard";
import { cn } from "@/lib/utils";

const DEFAULT_ICON_REGISTRY: Record<string, LucideIcon> = {
  "file-text": FileText,
};

interface WikilinkPopoverProps {
  /**
   * Current wikilink trigger match. Host typically owns this state via
   * `createEditor({ onEvent })` WikilinkTriggerChange listener; pass `null`
   * (or match with `active: false`) to close.
   */
  match: WikilinkTriggerMatch | null;
  /** Editor control, used for keyboard handling and click commit. */
  control: EditorControl | null;
  /** Optional override of the header label. Defaults to "Link to note". */
  headerLabel?: string;
  /** Optional override of the empty-state label. */
  emptyLabel?: string;
  /** Side preference for the popover. Defaults to "bottom". */
  side?: "top" | "bottom";
  /**
   * Optional icon registry override (semantic name → lucide component).
   * Items with an icon string not in the registry fall back to plain text.
   */
  iconRegistry?: Record<string, LucideIcon>;
}

/**
 * Obsidian-style wikilink popover bound to `@swarmnote/editor-core`'s
 * built-in `wikilinkPlugin`. Triggered by `[[` in the editor.
 *
 * Distributed via shadcn registry — consumers run `shadcn add wikilink-popover`
 * and own the source.
 */
export function WikilinkPopover({
  match,
  control,
  headerLabel = "Link to note",
  emptyLabel = "No matching notes",
  side = "bottom",
  iconRegistry,
}: WikilinkPopoverProps) {
  const open = match?.active ?? false;
  const items: WikilinkItem[] = match?.items ?? [];
  const activeIndex = match?.activeIndex ?? 0;
  const screenRect = match?.screenRect;
  const icons = iconRegistry
    ? { ...DEFAULT_ICON_REGISTRY, ...iconRegistry }
    : DEFAULT_ICON_REGISTRY;

  useTriggerKeyboard(open, control, "wikilink");

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
        <div className="px-2 pt-1.5 pb-0.5 text-xs font-medium text-muted-foreground">
          {headerLabel}
        </div>
        {items.length === 0 ? (
          <div className="px-2 py-1.5 text-sm text-muted-foreground" role="status">
            {emptyLabel}
          </div>
        ) : (
          <div
            role="listbox"
            aria-label={headerLabel}
            className="flex flex-col gap-0.5 max-h-72 overflow-y-auto"
          >
            {items.map((item, idx) => {
              const active = idx === activeIndex;
              const Icon = item.icon ? icons[item.icon] : undefined;
              return (
                <button
                  type="button"
                  key={item.id}
                  role="option"
                  aria-selected={active}
                  data-active={active || undefined}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    control?.execCommand("wikilink.confirmAt", idx);
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
                      <div className="truncate text-xs text-muted-foreground">
                        {item.description}
                      </div>
                    ) : null}
                  </div>
                </button>
              );
            })}
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}
