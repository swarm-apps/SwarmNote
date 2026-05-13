import { useLingui } from "@lingui/react/macro";
import type { EditorControl, WikilinkTriggerMatch } from "@swarmnote/editor-core";
import { useEffect, useMemo, useRef } from "react";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

interface WikilinkPopoverProps {
  match: WikilinkTriggerMatch | null;
  control: EditorControl | null;
}

/**
 * Floating Radix Popover for wikilink note picker. Mirror of SlashCommandPopover
 * but subscribes to `WikilinkTriggerChange` and dispatches `wikilink.*` commands.
 */
export function WikilinkPopover({ match, control }: WikilinkPopoverProps) {
  const { t } = useLingui();
  const open = match?.active ?? false;
  const items = useMemo(() => match?.items ?? [], [match]);
  const activeIndex = match?.activeIndex ?? 0;
  const screenRect = match?.screenRect;

  const controlRef = useRef(control);
  controlRef.current = control;

  useEffect(() => {
    if (!open || !control) return;
    const contentDom = control.view.contentDOM;
    const handler = (e: KeyboardEvent) => {
      let cmd: string | null = null;
      if (e.key === "ArrowDown") cmd = "wikilink.next";
      else if (e.key === "ArrowUp") cmd = "wikilink.prev";
      else if (e.key === "Enter") cmd = "wikilink.confirm";
      else if (e.key === "Escape") cmd = "wikilink.dismiss";
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
        <div className="px-2 pt-1.5 pb-0.5 text-xs font-medium text-muted-foreground">
          {t`Link to note`}
        </div>
        {items.length === 0 ? (
          <div className="px-2 py-1.5 text-sm text-muted-foreground">{t`No matching notes`}</div>
        ) : (
          <div className="flex flex-col gap-0.5 max-h-72 overflow-y-auto">
            {items.map((item, idx) => {
              const active = idx === activeIndex;
              return (
                <button
                  type="button"
                  key={item.id}
                  data-active={active || undefined}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    controlRef.current?.execCommand("wikilink.confirmAt", idx);
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
                  ) : (
                    <span className="text-base leading-5 flex-shrink-0" aria-hidden>
                      📄
                    </span>
                  )}
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
