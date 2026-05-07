import { useLingui } from "@lingui/react/macro";
import type { TableAlignment, TableContextMenuActions } from "@swarmnote/editor";
import {
  AlignCenter,
  AlignJustify,
  AlignLeft,
  AlignRight,
  ArrowRightFromLine,
  Check,
  Copy,
  Plus,
  Trash2,
} from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

export interface TableContextMenuState {
  open: boolean;
  clientX: number;
  clientY: number;
  /** -1 if right-click target is the header row, otherwise tbody row index. */
  rowIdx: number;
  colIdx: number;
  alignment: TableAlignment;
  rowCount: number;
  colCount: number;
  actions: TableContextMenuActions | null;
}

export const initialTableContextMenuState: TableContextMenuState = {
  open: false,
  clientX: 0,
  clientY: 0,
  rowIdx: -1,
  colIdx: 0,
  alignment: null,
  rowCount: 0,
  colCount: 0,
  actions: null,
};

interface TableContextMenuProps {
  state: TableContextMenuState;
  onOpenChange: (open: boolean) => void;
}

/**
 * Right-click context menu for the GFM table widget. The widget itself never
 * paints menu DOM — it raises an `EditorTableContextMenu` event with a bag of
 * imperative `actions`, and this component renders a shadcn DropdownMenu at
 * the click coordinates. Two submenus (Row / Column) plus three table-level
 * actions cover Obsidian's full surface (sort + cross-block move are v2).
 */
export function TableContextMenu({ state, onOpenChange }: TableContextMenuProps) {
  const { t } = useLingui();
  const { open, clientX, clientY, rowIdx, colIdx, alignment, colCount, actions } = state;

  if (!actions) return null;

  const isHeader = rowIdx === -1;
  const canDeleteCol = colCount > 1;

  const run = (fn: () => void) => () => {
    fn();
    onOpenChange(false);
  };

  return (
    <DropdownMenu open={open} onOpenChange={onOpenChange} modal={false}>
      {/* Invisible 1×1 trigger anchored at the right-click coordinates so the
          DropdownMenu opens exactly where the user clicked. */}
      <DropdownMenuTrigger asChild>
        <div
          aria-hidden
          style={{
            position: "fixed",
            left: clientX,
            top: clientY,
            width: 1,
            height: 1,
            pointerEvents: "none",
          }}
        />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-52">
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>{t`行`}</DropdownMenuSubTrigger>
          <DropdownMenuSubContent>
            {isHeader ? (
              <DropdownMenuItem onClick={run(() => actions.addRowAt(0, "above"))}>
                <Plus />
                {t`在表头下方新增行`}
              </DropdownMenuItem>
            ) : (
              <>
                <DropdownMenuItem onClick={run(() => actions.addRowAt(rowIdx, "above"))}>
                  <Plus />
                  {t`在上方新增行`}
                </DropdownMenuItem>
                <DropdownMenuItem onClick={run(() => actions.addRowAt(rowIdx, "below"))}>
                  <Plus />
                  {t`在下方新增行`}
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  variant="destructive"
                  onClick={run(() => actions.deleteRow(rowIdx))}
                >
                  <Trash2 />
                  {t`删除行`}
                </DropdownMenuItem>
              </>
            )}
          </DropdownMenuSubContent>
        </DropdownMenuSub>

        <DropdownMenuSub>
          <DropdownMenuSubTrigger>{t`列`}</DropdownMenuSubTrigger>
          <DropdownMenuSubContent>
            <DropdownMenuItem onClick={run(() => actions.addColumnAt(colIdx, "left"))}>
              <Plus />
              {t`在左侧新增列`}
            </DropdownMenuItem>
            <DropdownMenuItem onClick={run(() => actions.addColumnAt(colIdx, "right"))}>
              <Plus />
              {t`在右侧新增列`}
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <AlignmentItem
              icon={<AlignLeft />}
              label={t`左对齐`}
              active={alignment === "left"}
              onClick={run(() => actions.setAlignment(colIdx, "left"))}
            />
            <AlignmentItem
              icon={<AlignCenter />}
              label={t`居中对齐`}
              active={alignment === "center"}
              onClick={run(() => actions.setAlignment(colIdx, "center"))}
            />
            <AlignmentItem
              icon={<AlignRight />}
              label={t`右对齐`}
              active={alignment === "right"}
              onClick={run(() => actions.setAlignment(colIdx, "right"))}
            />
            <AlignmentItem
              icon={<AlignJustify />}
              label={t`默认对齐`}
              active={alignment === null}
              onClick={run(() => actions.setAlignment(colIdx, null))}
            />
            {canDeleteCol && (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  variant="destructive"
                  onClick={run(() => actions.deleteColumn(colIdx))}
                >
                  <Trash2 />
                  {t`删除列`}
                </DropdownMenuItem>
              </>
            )}
          </DropdownMenuSubContent>
        </DropdownMenuSub>

        <DropdownMenuSeparator />

        <DropdownMenuItem onClick={run(actions.toggleSource)}>
          <ArrowRightFromLine />
          {t`切换源码`}
        </DropdownMenuItem>
        <DropdownMenuItem onClick={run(actions.copyMarkdown)}>
          <Copy />
          {t`复制为 Markdown`}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem variant="destructive" onClick={run(actions.deleteTable)}>
          <Trash2 />
          {t`删除表格`}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

interface AlignmentItemProps {
  icon: React.ReactNode;
  label: string;
  active: boolean;
  onClick: () => void;
}

function AlignmentItem({ icon, label, active, onClick }: AlignmentItemProps) {
  return (
    <DropdownMenuItem onClick={onClick}>
      {icon}
      <span className="flex-1">{label}</span>
      {active ? <Check className="size-4" /> : null}
    </DropdownMenuItem>
  );
}
