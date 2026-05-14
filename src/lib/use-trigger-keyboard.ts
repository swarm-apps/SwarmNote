import type { EditorControl } from "@swarmnote/editor-core";
import { useEffect, useRef } from "react";

/**
 * Bind ArrowUp/ArrowDown/Enter/Escape on the editor's contentDOM to
 * `<commandPrefix>.next` / `.prev` / `.confirm` / `.dismiss` commands.
 *
 * Shared by slash-popover and wikilink-popover (any char-trigger style
 * popover). Bound only while `open` is true so editor keyboard shortcuts
 * remain intact when the popover is closed.
 *
 * @param open Whether the popover is currently shown
 * @param control Editor control (from `useEditorControl()` or props)
 * @param commandPrefix `"slash"` for slash, `"wikilink"` for wikilink
 */
export function useTriggerKeyboard(
  open: boolean,
  control: EditorControl | null,
  commandPrefix: "slash" | "wikilink",
): void {
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
}
