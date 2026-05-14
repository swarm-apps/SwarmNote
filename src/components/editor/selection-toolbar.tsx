import type { EditorControl, SelectionToolbarMatch } from "@swarmnote/editor-core";
import { Bold, Code, Italic, Link as LinkIcon, type LucideIcon, Strikethrough } from "lucide-react";
import { useMemo, useRef } from "react";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

interface SelectionToolbarProps {
  /**
   * Current selection toolbar match. Host typically owns this state via
   * `createEditor({ onEvent })` SelectionToolbarChange listener; pass `null`
   * (or match with `active: false`) to hide.
   */
  match: SelectionToolbarMatch | null;
  /** Editor control, used for command dispatch on button click. */
  control: EditorControl | null;
  /**
   * Optional icon registry override. Defaults to lucide-react bold / italic /
   * strikethrough / code / link icons. Add custom keys (matching `action.icon`
   * strings) to render additional plugin-contributed actions.
   */
  iconRegistry?: Record<string, LucideIcon>;
}

const DEFAULT_ICON_REGISTRY: Record<string, LucideIcon> = {
  bold: Bold,
  italic: Italic,
  strikethrough: Strikethrough,
  code: Code,
  link: LinkIcon,
};

/**
 * Floating toolbar above the current text selection.
 *
 * Distributed via shadcn registry — consumers run `shadcn add selection-toolbar`
 * and own the source. Renders each action as a button; click dispatches
 * `action.commandId` via `control.execCommand`. `onMouseDown` + `preventDefault`
 * prevents the editor's selection from being lost when the button is pressed.
 *
 * Plugin-contributed actions can declare custom icon strings; extend
 * `iconRegistry` prop to render them.
 */
export function SelectionToolbar({
  match,
  control,
  iconRegistry = DEFAULT_ICON_REGISTRY,
}: SelectionToolbarProps) {
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
        role="toolbar"
        aria-label="Selection formatting"
      >
        {actions.map((action) => {
          const Icon = iconRegistry[action.icon];
          return (
            <button
              type="button"
              key={action.id}
              title={action.title}
              aria-label={action.title}
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
