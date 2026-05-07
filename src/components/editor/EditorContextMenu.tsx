import { useLingui } from "@lingui/react/macro";
import {
  DEFAULT_SELECTION_FORMATTING,
  type EditorControl,
  type SelectionFormatting,
} from "@swarmnote/editor";
import {
  Bold,
  Code,
  Code2,
  Copy,
  Highlighter,
  Image as ImageIcon,
  Italic,
  Link as LinkIcon,
  Minus,
  MousePointerSquareDashed,
  Pilcrow,
  PlusSquare,
  Quote,
  Scissors,
  Strikethrough,
  Table as TableIcon,
  Type,
  WrapText,
} from "lucide-react";
import { type ReactNode, useCallback, useState } from "react";
import {
  ContextMenu,
  ContextMenuCheckboxItem,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuRadioGroup,
  ContextMenuRadioItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { modKey } from "@/lib/utils";
import { useUIStore } from "@/stores/uiStore";

interface EditorContextMenuProps {
  children: ReactNode;
  control: EditorControl | null;
  onInsertImage: () => void | Promise<void>;
}

/**
 * Right-click context menu for the desktop CodeMirror editor. Modeled after
 * Obsidian: full menu is always rendered, selection-dependent items (Cut,
 * Copy) are disabled when there's no selection. The formatting snapshot for
 * active-state checkmarks is frozen at menu open time via
 * `getSelectionFormatting()` — items don't re-render while the menu is mounted.
 */
export function EditorContextMenu({ children, control, onInsertImage }: EditorContextMenuProps) {
  const { t } = useLingui();
  const readableLineLength = useUIStore((s) => s.readableLineLength);
  const setReadableLineLength = useUIStore((s) => s.setReadableLineLength);

  const [formatting, setFormatting] = useState<SelectionFormatting>(DEFAULT_SELECTION_FORMATTING);
  const [hasSelection, setHasSelection] = useState(false);

  const handleOpenChange = useCallback(
    (open: boolean) => {
      if (!open || !control) return;
      setFormatting(control.getSelectionFormatting());
      setHasSelection(!control.view.state.selection.main.empty);
    },
    [control],
  );

  const exec = useCallback(
    (name: string, ...args: unknown[]) => {
      control?.execCommand(name, ...args);
    },
    [control],
  );

  const copySelection = useCallback(() => {
    if (!control) return;
    const { from, to } = control.view.state.selection.main;
    if (from === to) return;
    const text = control.view.state.sliceDoc(from, to);
    navigator.clipboard.writeText(text).catch((err) => console.warn("clipboard write failed", err));
  }, [control]);

  const cutSelection = useCallback(() => {
    if (!control) return;
    const { from, to } = control.view.state.selection.main;
    if (from === to) return;
    const text = control.view.state.sliceDoc(from, to);
    navigator.clipboard.writeText(text).catch((err) => console.warn("clipboard write failed", err));
    control.view.dispatch({
      changes: { from, to, insert: "" },
      selection: { anchor: from },
    });
    control.view.focus();
  }, [control]);

  const handleHeading = useCallback(
    (newLevel: number) => {
      if (!control) return;
      const current = formatting.heading;
      if (newLevel === current) return;
      if (newLevel === 0) {
        if (current >= 1) {
          control.execCommand("toggleHeading", current);
        }
        return;
      }
      control.execCommand("toggleHeading", newLevel);
    },
    [control, formatting.heading],
  );

  const handleList = useCallback(
    (type: string) => {
      const cmd =
        type === "unordered"
          ? "toggleUnorderedList"
          : type === "ordered"
            ? "toggleOrderedList"
            : type === "check"
              ? "toggleCheckList"
              : null;
      if (cmd) exec(cmd);
    },
    [exec],
  );

  const headingValue = String(formatting.heading);
  const listValue = formatting.listType ?? "";

  return (
    <ContextMenu onOpenChange={handleOpenChange}>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      {control == null ? null : (
        <ContextMenuContent className="w-56">
          <ContextMenuItem onClick={() => exec("insertLink")}>
            <LinkIcon />
            {t`插入链接`}
            <ContextMenuShortcut>{`${modKey}K`}</ContextMenuShortcut>
          </ContextMenuItem>
          <ContextMenuItem onClick={() => void onInsertImage()}>
            <ImageIcon />
            {t`插入图片`}
          </ContextMenuItem>
          <ContextMenuSub>
            <ContextMenuSubTrigger>
              <PlusSquare />
              {t`插入`}
            </ContextMenuSubTrigger>
            <ContextMenuSubContent>
              <ContextMenuItem onClick={() => exec("insertCodeBlock")}>
                <Code2 />
                {t`插入代码块`}
              </ContextMenuItem>
              <ContextMenuItem onClick={() => exec("insertTable")}>
                <TableIcon />
                {t`插入表格`}
              </ContextMenuItem>
              <ContextMenuItem onClick={() => exec("insertHorizontalRule")}>
                <Minus />
                {t`插入分割线`}
              </ContextMenuItem>
            </ContextMenuSubContent>
          </ContextMenuSub>
          <ContextMenuSeparator />

          <ContextMenuSub>
            <ContextMenuSubTrigger>
              <Type />
              {t`文本格式`}
            </ContextMenuSubTrigger>
            <ContextMenuSubContent>
              <ContextMenuCheckboxItem
                checked={formatting.bold}
                onCheckedChange={() => exec("toggleBold")}
              >
                <Bold />
                {t`加粗`}
                <ContextMenuShortcut>{`${modKey}B`}</ContextMenuShortcut>
              </ContextMenuCheckboxItem>
              <ContextMenuCheckboxItem
                checked={formatting.italic}
                onCheckedChange={() => exec("toggleItalic")}
              >
                <Italic />
                {t`斜体`}
                <ContextMenuShortcut>{`${modKey}I`}</ContextMenuShortcut>
              </ContextMenuCheckboxItem>
              <ContextMenuCheckboxItem
                checked={formatting.strikethrough}
                onCheckedChange={() => exec("toggleStrike")}
              >
                <Strikethrough />
                {t`删除线`}
                <ContextMenuShortcut>{`${modKey}⇧X`}</ContextMenuShortcut>
              </ContextMenuCheckboxItem>
              <ContextMenuCheckboxItem
                checked={formatting.highlight}
                onCheckedChange={() => exec("toggleHighlight")}
              >
                <Highlighter />
                {t`高亮`}
                <ContextMenuShortcut>{`${modKey}⇧=`}</ContextMenuShortcut>
              </ContextMenuCheckboxItem>
              <ContextMenuSeparator />
              <ContextMenuCheckboxItem
                checked={formatting.code}
                onCheckedChange={() => exec("toggleCode")}
              >
                <Code />
                {t`行内代码`}
                <ContextMenuShortcut>{`${modKey}E`}</ContextMenuShortcut>
              </ContextMenuCheckboxItem>
            </ContextMenuSubContent>
          </ContextMenuSub>

          <ContextMenuSub>
            <ContextMenuSubTrigger>
              <Pilcrow />
              {t`段落设置`}
            </ContextMenuSubTrigger>
            <ContextMenuSubContent>
              <ContextMenuRadioGroup value={listValue} onValueChange={handleList}>
                <ContextMenuRadioItem value="unordered">{t`无序列表`}</ContextMenuRadioItem>
                <ContextMenuRadioItem value="ordered">{t`有序列表`}</ContextMenuRadioItem>
                <ContextMenuRadioItem value="check">{t`任务列表`}</ContextMenuRadioItem>
              </ContextMenuRadioGroup>
              <ContextMenuSeparator />
              <ContextMenuRadioGroup
                value={headingValue}
                onValueChange={(v) => handleHeading(Number.parseInt(v, 10))}
              >
                <ContextMenuRadioItem value="1">{t`标题 1`}</ContextMenuRadioItem>
                <ContextMenuRadioItem value="2">{t`标题 2`}</ContextMenuRadioItem>
                <ContextMenuRadioItem value="3">{t`标题 3`}</ContextMenuRadioItem>
                <ContextMenuRadioItem value="4">{t`标题 4`}</ContextMenuRadioItem>
                <ContextMenuRadioItem value="5">{t`标题 5`}</ContextMenuRadioItem>
                <ContextMenuRadioItem value="6">{t`标题 6`}</ContextMenuRadioItem>
                <ContextMenuRadioItem value="0">{t`正文`}</ContextMenuRadioItem>
              </ContextMenuRadioGroup>
              <ContextMenuSeparator />
              <ContextMenuCheckboxItem
                checked={formatting.inBlockquote}
                onCheckedChange={() => exec("toggleBlockquote")}
              >
                <Quote />
                {t`引用块`}
                <ContextMenuShortcut>{`${modKey}⇧Q`}</ContextMenuShortcut>
              </ContextMenuCheckboxItem>
            </ContextMenuSubContent>
          </ContextMenuSub>
          <ContextMenuSeparator />

          <ContextMenuItem onClick={cutSelection} disabled={!hasSelection}>
            <Scissors />
            {t`剪切`}
            <ContextMenuShortcut>{`${modKey}X`}</ContextMenuShortcut>
          </ContextMenuItem>
          <ContextMenuItem onClick={copySelection} disabled={!hasSelection}>
            <Copy />
            {t`复制`}
            <ContextMenuShortcut>{`${modKey}C`}</ContextMenuShortcut>
          </ContextMenuItem>
          <ContextMenuItem onClick={() => exec("selectAll")}>
            <MousePointerSquareDashed />
            {t`全选`}
            <ContextMenuShortcut>{`${modKey}A`}</ContextMenuShortcut>
          </ContextMenuItem>
          <ContextMenuSeparator />

          <ContextMenuLabel>{t`视图`}</ContextMenuLabel>
          <ContextMenuCheckboxItem
            checked={readableLineLength}
            onCheckedChange={(checked) => setReadableLineLength(checked === true)}
          >
            <WrapText />
            {t`可读行宽`}
          </ContextMenuCheckboxItem>
        </ContextMenuContent>
      )}
    </ContextMenu>
  );
}
