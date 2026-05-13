import type { EditorControl, SelectionToolbarMatch } from "@swarmnote/editor-core";
import { Bold, Code, Italic, Link as LinkIcon, type LucideIcon, Strikethrough } from "lucide-react";
import { useMemo, useRef } from "react";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

interface SelectionToolbarProps {
  match: SelectionToolbarMatch | null;
  control: EditorControl | null;
}

const ICON_REGISTRY: Record<string, LucideIcon> = {
  bold: Bold,
  italic: Italic,
  strikethrough: Strikethrough,
  code: Code,
  link: LinkIcon,
};

/**
 * Floating toolbar above the current text selection. Subscribes to
 * `SelectionToolbarChange` and renders the merged action buttons; each
 * button dispatches `action.commandId` via `editorControl.execCommand`.
 *
 * Uses onMouseDown + preventDefault so the editor selection isn't lost
 * when the button is pressed.
 */
export function SelectionToolbar({ match, control }: SelectionToolbarProps) {
  const open = match?.active ?? false;
  const actions = useMemo(() => match?.actions ?? [], [match]);
  const screenRect = match?.screenRect;

  const controlRef = useRef(control);
  controlRef.current = control;

  if (!open || !screenRect || actions.length === 0) return null;

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
        align="center"
        side="top"
        sideOffset={6}
        className="flex flex-row items-center gap-0.5 p-1 w-auto"
        onOpenAutoFocus={(e) => e.preventDefault()}
        onCloseAutoFocus={(e) => e.preventDefault()}
      >
        {actions.map((action) => {
          const Icon = ICON_REGISTRY[action.icon];
          return (
            <button
              type="button"
              key={action.id}
              title={action.title}
              onMouseDown={(e) => {
                e.preventDefault();
                controlRef.current?.execCommand(action.commandId);
              }}
              className={cn(
                "inline-flex h-8 w-8 items-center justify-center rounded-sm",
                "cursor-pointer select-none text-sm",
                "hover:bg-muted",
              )}
            >
              {Icon ? <Icon className="h-4 w-4" /> : <span aria-hidden>{action.icon}</span>}
            </button>
          );
        })}
      </PopoverContent>
    </Popover>
  );
}
