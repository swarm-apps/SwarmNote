import { useLingui } from "@lingui/react/macro";
import type { EditorControl, SlashTriggerMatch } from "@swarmnote/editor-core";
import { useEffect, useMemo, useRef } from "react";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

interface SlashCommandPopoverProps {
  /** Current trigger match from `SlashTriggerChange` events. */
  match: SlashTriggerMatch | null;
  /** Editor control instance used to route keyboard events to `slash.*` commands. */
  control: EditorControl | null;
}

/**
 * Renders a floating Radix Popover with slash candidate items.
 *
 * Subscribes to keyboard events on the editor's contentDOM while the trigger
 * is active and routes ↑/↓/Enter/Escape to the SDK's slash.* commands.
 */
export function SlashCommandPopover({ match, control }: SlashCommandPopoverProps) {
  const { t } = useLingui();
  const open = match?.active ?? false;
  const items = useMemo(() => match?.items ?? [], [match]);
  const activeIndex = match?.activeIndex ?? 0;
  const screenRect = match?.screenRect;

  // Capture keyboard events on the editor's contentDOM while open, route to slash.* commands.
  const controlRef = useRef(control);
  controlRef.current = control;

  useEffect(() => {
    if (!open || !control) return;
    const contentDom = control.view.contentDOM;
    const handler = (e: KeyboardEvent) => {
      let cmd: string | null = null;
      if (e.key === "ArrowDown") cmd = "slash.next";
      else if (e.key === "ArrowUp") cmd = "slash.prev";
      else if (e.key === "Enter") cmd = "slash.confirm";
      else if (e.key === "Escape") cmd = "slash.dismiss";
      if (!cmd) return;
      e.preventDefault();
      e.stopPropagation();
      controlRef.current?.execCommand(cmd);
    };
    contentDom.addEventListener("keydown", handler, true);
    return () => {
      contentDom.removeEventListener("keydown", handler, true);
    };
  }, [open, control]);

  // Group items by section if any
  const grouped = useMemo(() => {
    const buckets = new Map<string, typeof items>();
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
        side="bottom"
        sideOffset={4}
        className="w-72 p-1"
        onOpenAutoFocus={(e) => e.preventDefault()}
        onCloseAutoFocus={(e) => e.preventDefault()}
      >
        {items.length === 0 ? (
          <div className="px-2 py-1.5 text-sm text-muted-foreground">{t`No matching commands`}</div>
        ) : (
          <div className="flex flex-col gap-0.5 max-h-72 overflow-y-auto">
            {grouped.map(([section, sectionItems]) => (
              <SlashSection
                key={section || "_default"}
                label={section}
                items={sectionItems}
                activeIndex={activeIndex}
                allItems={items}
                onPick={(absoluteIndex) => {
                  controlRef.current?.execCommand("slash.confirmAt", absoluteIndex);
                }}
              />
            ))}
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}

interface SlashSectionProps {
  label: string;
  items: SlashTriggerMatch["items"];
  activeIndex: number;
  allItems: SlashTriggerMatch["items"];
  onPick: (absoluteIndex: number) => void;
}

function SlashSection({ label, items, activeIndex, allItems, onPick }: SlashSectionProps) {
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
            // mousedown 而非 click：mousedown 在 blur 之前 fire，避免编辑器先失焦
            // 导致 trigger 在 click 到达前已被 dismiss。preventDefault 防失焦。
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
